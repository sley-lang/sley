//! RW-075 executable-closure and self-host boundary repair fixtures.
//!
//! These fixtures use only the public `sley-vm` surface plus public
//! `sley-ssmc` value types — the same boundary a later seed-absent
//! toolchain sees. They prove, through the repaired
//! `execute_approved_package` path:
//!
//! * AR-01: a package with forged/missing/mismatched layouts, constants,
//!   field layouts, dependency closure, or root/inventory cannot execute;
//!   native `TypeEnvironment` semantic preparation is not required (the
//!   package path hydrates structurally via
//!   `hydrate_verified_definitions`, never `TypeEnvironment::new`);
//! * AR-03: the complete closure (image, constants, layouts, exact import
//!   rows, profile, epoch, VM/ABI versions, limits, entry, package digest,
//!   admission receipt) is bound; correct-image/wrong-closure,
//!   wrong-receipt, stale-package, mismatched-cache, cross-package
//!   observation, and broader-profile substitutions all refuse;
//! * AR-04: anti-shortcut probes showing the host cannot synthesize
//!   semantic answers (checker/lowering/candidate/dependency results,
//!   semantic digests, or high-level environments);
//! * AR-05: branching work-queue, real image emission, and mixed
//!   compiler-like workloads within declared bounds.
//!
//! No toolchain graph, no C1, and no SH1/SH2 claim in this file.

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BuiltinFailureKind, ConstData, ConstValue, ConstantDefinition,
    ContractDefinition, FunctionGraph, GlobalValueDefinition, Immediate, IntegerWidth, MemberId,
    Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability, RecordField,
    ReturnTerminator, Terminator, TypeDefForm, TypeDefinition, TypeExpr, ValueRef, Visibility,
};
use sley_vm::bootstrap::{BootstrapProfileInput, judge_bootstrap_profile};
use sley_vm::{
    ApprovedExecutionPackage, CacheProfile, ExecutionLimits, ExecutionPackage, ExecutionRequest,
    ExecutionTermination, LoweringInput, PackageError, admit_package, approve_package,
    execute_approved_package, lower_function, package_digests,
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

fn u64_value(value: u64) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::UInt(IntegerWidth::from_bits(64)),
        data: ConstData::UInt(u128::from(value)),
    }
}

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}

/// One hand-built single-block program plus every inventory the package
/// carries. Entity IDs are fixed so approved bindings are deterministic.
struct Program {
    types: TypeEnvironment,
    definitions: Vec<TypeDefinition>,
    entry: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    globals: Vec<GlobalValueDefinition>,
    contracts: Vec<ContractDefinition>,
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
            globals: &self.globals,
            functions: &[],
            contracts: &self.contracts,
            adapters: &self.adapters,
        }
    }

    fn package(&self, limits: ExecutionLimits) -> ExecutionPackage {
        let lowered = lower_function(self.lowering_input()).expect("fixture lowers");
        let gate = self.gate_report(&lowered.bytes);
        self.package_with_counts(
            limits,
            gate.operation_count(),
            gate.bridge_uses(),
            gate.closure_fingerprints().to_vec(),
        )
    }

    /// Builds a package with caller-supplied gate evidence (the
    /// fabrication path: an attacker can write any evidence into package
    /// bytes, but approval cross-checks it against the sealed gate
    /// report and the actual image bytes).
    fn package_with_counts(
        &self,
        limits: ExecutionLimits,
        gate_operation_count: u32,
        gate_bridge_uses: u32,
        gate_closure_fingerprints: Vec<sley_id::SemanticFingerprint>,
    ) -> ExecutionPackage {
        let lowered = lower_function(self.lowering_input()).expect("fixture lowers");
        ExecutionPackage {
            image_bytes: lowered.bytes.clone(),
            constants: self.constants.clone(),
            type_definitions: self.definitions.clone(),
            imports: self.adapters.clone(),
            globals: self.globals.clone(),
            contracts: self.contracts.clone(),
            entry: self.entry.entity_id,
            schema_epoch: epoch(),
            state_root: root(),
            profile: CacheProfile::EXTENDED_V1,
            admitted_limits: limits,
            gate_operation_count,
            gate_bridge_uses,
            gate_closure_fingerprints,
        }
    }

    fn approved(&self, limits: ExecutionLimits) -> (ExecutionPackage, ApprovedExecutionPackage) {
        let package = self.package(limits);
        let digests = package_digests(&package).expect("fixture digests");
        let receipt = admit_package(digests.package_digest);
        // The same image bytes the package was built from: the approval
        // reuses the image-bound report, exactly as the authority does.
        let gate = self.gate_report(&package.image_bytes);
        let approved =
            approve_package(&package, &digests, receipt, &gate).expect("fixture approves");
        (package, approved)
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
        })
        .expect("fixture closure is gate-admitted")
    }
}

/// Boolean conjunction: `(Bool, Bool) -> Bool`, no imports, no constants,
/// no definitions. The minimal load-bearing image for binding tests.
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
        entry: graph.clone(),
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
        globals: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

fn record_definition(definition: EntityId, field_type: TypeExpr) -> TypeDefinition {
    TypeDefinition {
        entity_id: definition,
        type_parameters: Vec::new(),
        form: TypeDefForm::Record(vec![RecordField {
            member_id: MemberId::from_bytes([0xA1; 32]),
            value_type: field_type,
            visibility: Visibility::Private,
        }]),
        invariants: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn named(definition: EntityId) -> TypeExpr {
    TypeExpr::Named(sley_ssmc::NamedType {
        definition,
        arguments: Vec::new(),
    })
}

/// Record construction: `(Bool) -> Record{Bool}`, one definition.
/// Layouts are load-bearing here: the image's register types name the
/// definition, and execution resolves its fields structurally.
fn record_program(field_type: TypeExpr) -> Program {
    let definition = id(60);
    let definitions = vec![record_definition(definition, field_type.clone())];
    let function = id(1);
    let block = id(2);
    let parameter = id(10);
    let operation = id(100);
    let result = named(definition);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![parameter],
        result_type: result.clone(),
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    Program {
        types: TypeEnvironment::new(definitions.clone()).unwrap(),
        definitions,
        entry: graph,
        parameters: vec![Parameter {
            entity_id: parameter,
            owner: function,
            role: ParameterRole::Function,
            ordinal: 0,
            value_type: field_type,
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
            opcode: Opcode::RecordNew,
            operands: vec![ValueRef::Parameter(parameter)],
            result_types: vec![result],
            immediate: Immediate::Entity(definition),
        }],
        constants: Vec::new(),
        globals: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

/// Constant reference: `() -> Bool` via `constant_ref`. Constants are
/// load-bearing: the image names the constant, the package binds its value.
fn constant_program(value: bool) -> Program {
    let constant = id(200);
    let constants = vec![ConstantDefinition {
        entity_id: constant,
        value: bool_value(value),
    }];
    let function = id(3);
    let block = id(4);
    let operation = id(103);
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
        globals: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

fn request(inputs: Vec<ConstValue>, limits: ExecutionLimits) -> ExecutionRequest {
    ExecutionRequest { inputs, limits }
}

fn success_bool(termination: &ExecutionTermination) -> bool {
    match termination {
        ExecutionTermination::Success(value) => match &value.data {
            ConstData::Bool(value) => *value,
            other => panic!("expected a Bool result, got {other:?}"),
        },
        other => panic!("expected success, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// AR-01: complete executable closure — no native semantic preparation.
// ---------------------------------------------------------------------------

#[test]
fn package_path_executes_without_type_environment_new() {
    let program = bool_and_program();
    let limits = generous_limits();
    let (package, approved) = program.approved(limits);
    let outcome = execute_approved_package(
        &package,
        &approved,
        request(vec![bool_value(true), bool_value(false)], limits),
    )
    .expect("valid package executes");
    assert!(
        !success_bool(&outcome.termination),
        "true AND false is false"
    );
    let outcome = execute_approved_package(
        &package,
        &approved,
        request(vec![bool_value(true), bool_value(true)], limits),
    )
    .expect("valid package executes");
    assert!(success_bool(&outcome.termination), "true AND true is true");
}

#[test]
fn forged_layout_cannot_execute() {
    let program = record_program(TypeExpr::Bool);
    let limits = generous_limits();
    let (_, approved) = program.approved(limits);
    let forged = record_program(TypeExpr::UInt(IntegerWidth::from_bits(8)));
    let forged_package = forged.package(limits);
    assert!(
        matches!(
            execute_approved_package(
                &forged_package,
                &approved,
                request(vec![bool_value(true)], limits),
            )
            .unwrap_err(),
            sley_vm::PackageExecutionError::Package(_)
        ),
        "same image shape with a forged field layout must refuse by binding"
    );
}

#[test]
fn missing_layout_cannot_execute() {
    let program = record_program(TypeExpr::Bool);
    let limits = generous_limits();
    let (_, approved) = program.approved(limits);
    let mut missing = program.package(limits);
    missing.type_definitions.clear();
    assert!(
        matches!(
            execute_approved_package(&missing, &approved, request(vec![bool_value(true)], limits),)
                .unwrap_err(),
            sley_vm::PackageExecutionError::Package(_)
        ),
        "dropped layouts must refuse by binding"
    );
}

#[test]
fn mismatched_constants_cannot_execute() {
    let program = constant_program(true);
    let limits = generous_limits();
    let (_, approved) = program.approved(limits);
    let wrong = constant_program(false);
    let wrong_package = wrong.package(limits);
    assert!(
        matches!(
            execute_approved_package(&wrong_package, &approved, request(Vec::new(), limits),)
                .unwrap_err(),
            sley_vm::PackageExecutionError::Package(_)
        ),
        "same image with a different constant value must refuse"
    );
}

#[test]
fn wrong_field_layout_cannot_execute() {
    let program = record_program(TypeExpr::Bool);
    let limits = generous_limits();
    let (package, approved) = program.approved(limits);
    let outcome =
        execute_approved_package(&package, &approved, request(vec![bool_value(true)], limits))
            .expect("correct field layout executes");
    match outcome.termination {
        ExecutionTermination::Success(_) => {}
        other => panic!("expected record success, got {other:?}"),
    }
    let wrong_field = record_program(u64_type());
    let wrong_package = wrong_field.package(limits);
    execute_approved_package(
        &wrong_package,
        &approved,
        request(vec![u64_value(7)], limits),
    )
    .expect_err("u64 field layout against a Bool approval must refuse");
}

#[test]
fn wrong_dependency_closure_cannot_execute() {
    let program = bool_and_program();
    let limits = generous_limits();
    let (_, approved) = program.approved(limits);
    let mut extra = program.package(limits);
    extra.globals.push(GlobalValueDefinition {
        entity_id: id(90),
        value_type: TypeExpr::Bool,
        initializer: id(91),
        visibility: Visibility::Private,
    });
    assert!(
        matches!(
            execute_approved_package(
                &extra,
                &approved,
                request(vec![bool_value(true), bool_value(true)], limits),
            )
            .unwrap_err(),
            sley_vm::PackageExecutionError::Package(_)
        ),
        "an added global changes the dependency digest and must refuse"
    );
}

#[test]
fn mismatched_root_inventory_cannot_execute() {
    let program = bool_and_program();
    let limits = generous_limits();
    let (_, approved) = program.approved(limits);
    let mut wrong_root = program.package(limits);
    wrong_root.state_root = StateRoot::from_bytes([0xAA; 32]);
    assert!(
        matches!(
            execute_approved_package(
                &wrong_root,
                &approved,
                request(vec![bool_value(true), bool_value(true)], limits),
            )
            .unwrap_err(),
            sley_vm::PackageExecutionError::Package(_)
        ),
        "a different state root is a different closure and must refuse"
    );
    let mut wrong_epoch = program.package(limits);
    wrong_epoch.schema_epoch = SchemaEpochId::from_bytes([0xBB; 32]);
    execute_approved_package(
        &wrong_epoch,
        &approved,
        request(vec![bool_value(true), bool_value(true)], limits),
    )
    .expect_err("a different epoch must refuse");
}

#[test]
fn malformed_structural_metadata_cannot_execute() {
    let program = bool_and_program();
    let limits = generous_limits();
    let (mut package, approved) = program.approved(limits);
    package.image_bytes.truncate(package.image_bytes.len() / 2);
    execute_approved_package(
        &package,
        &approved,
        request(vec![bool_value(true), bool_value(true)], limits),
    )
    .expect_err("truncated image bytes must refuse before execution");
}

// ---------------------------------------------------------------------------
// AR-03: bind the whole execution package.
// ---------------------------------------------------------------------------

#[test]
fn correct_image_wrong_constants_refuses() {
    let program = constant_program(true);
    let limits = generous_limits();
    let (package, approved) = program.approved(limits);
    let outcome = execute_approved_package(&package, &approved, request(Vec::new(), limits))
        .expect("correct closure executes");
    assert!(success_bool(&outcome.termination));
    mismatched_constants_cannot_execute();
}

#[test]
fn correct_image_wrong_layouts_refuses() {
    forged_layout_cannot_execute();
    missing_layout_cannot_execute();
}

#[test]
fn correct_image_wrong_import_row_refuses() {
    use sley_vm::host_abi::{BRIDGE_ABI_VERSION, bridge_identity};
    let program = bool_and_program();
    let limits = generous_limits();
    let (_, approved) = program.approved(limits);
    assert!(approved.imports.is_empty());
    let identity = EntityId::from_bytes(bridge_identity(*b"B2V1"));
    let row = AdapterImport {
        entity_id: identity,
        adapter_id: *identity.as_bytes(),
        abi_version: BRIDGE_ABI_VERSION,
        request_type: TypeExpr::Bytes,
        response_type: TypeExpr::Vector(Box::new(TypeExpr::UInt(IntegerWidth::from_bits(8)))),
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    };
    let mut wrong_imports = program.package(limits);
    wrong_imports.imports.push(row.clone());
    assert!(
        matches!(
            execute_approved_package(
                &wrong_imports,
                &approved,
                request(vec![bool_value(true), bool_value(true)], limits),
            )
            .unwrap_err(),
            sley_vm::PackageExecutionError::Package(_)
        ),
        "an added import row changes the manifest digest and must refuse"
    );
    let mut wrong_schema = program.package(limits);
    let mut tampered = row.clone();
    tampered.response_type = TypeExpr::Bytes;
    wrong_schema.imports.push(tampered);
    execute_approved_package(
        &wrong_schema,
        &approved,
        request(vec![bool_value(true), bool_value(true)], limits),
    )
    .expect_err("same import ID with a different schema must refuse");
}

#[test]
fn correct_image_wrong_profile_refuses() {
    let program = bool_and_program();
    let limits = generous_limits();
    let (mut package, approved) = program.approved(limits);
    package.profile = CacheProfile::RESTRICTED_V1;
    assert!(
        matches!(
            execute_approved_package(
                &package,
                &approved,
                request(vec![bool_value(true), bool_value(true)], limits),
            )
            .unwrap_err(),
            sley_vm::PackageExecutionError::Package(_)
        ),
        "a re-profiled package is a different closure"
    );
}

#[test]
fn wrong_limits_refuse() {
    let program = bool_and_program();
    let limits = generous_limits();
    let (package, approved) = program.approved(limits);
    let mut other_limits = limits;
    other_limits.max_fuel += 1;
    assert!(
        matches!(
            execute_approved_package(
                &package,
                &approved,
                request(vec![bool_value(true), bool_value(true)], other_limits),
            )
            .unwrap_err(),
            sley_vm::PackageExecutionError::Package(_)
        ),
        "budgets are bound: a different limit set must refuse"
    );
}

#[test]
fn receipt_for_another_package_refuses() {
    let first = bool_and_program();
    let second = constant_program(true);
    let limits = generous_limits();
    let (first_package, _) = first.approved(limits);
    let (_, second_approved) = second.approved(limits);
    assert_eq!(
        execute_approved_package(
            &first_package,
            &second_approved,
            request(vec![bool_value(true), bool_value(true)], limits),
        )
        .unwrap_err(),
        sley_vm::PackageExecutionError::Package(PackageError::ReceiptMismatch),
        "a receipt for another package must refuse"
    );
}

#[test]
fn forged_root_identity_refuses() {
    mismatched_root_inventory_cannot_execute();
}

#[test]
fn stale_package_refuses() {
    let first = constant_program(true);
    let limits = generous_limits();
    let (_, stale_approved) = first.approved(limits);
    let second = constant_program(false);
    let (second_package, _) = second.approved(limits);
    execute_approved_package(
        &second_package,
        &stale_approved,
        request(Vec::new(), limits),
    )
    .expect_err("new bytes under an old approval must refuse");
}

#[test]
fn cache_key_with_mismatched_closure_refuses() {
    let program = bool_and_program();
    let limits = generous_limits();
    let (mut package, approved) = program.approved(limits);
    package.entry = id(77);
    execute_approved_package(
        &package,
        &approved,
        request(vec![bool_value(true), bool_value(true)], limits),
    )
    .expect_err("a different entry is a different cache identity");
}

#[test]
fn observations_bind_the_complete_package_identity() {
    let first = bool_and_program();
    let second = constant_program(true);
    let limits = generous_limits();
    let (first_package, first_approved) = first.approved(limits);
    let (second_package, second_approved) = second.approved(limits);
    let first_outcome = execute_approved_package(
        &first_package,
        &first_approved,
        request(vec![bool_value(true), bool_value(true)], limits),
    )
    .expect("first package executes");
    let second_outcome = execute_approved_package(
        &second_package,
        &second_approved,
        request(Vec::new(), limits),
    )
    .expect("second package executes");
    assert_ne!(
        first_outcome.observation_id, second_outcome.observation_id,
        "two distinct packages must not produce interchangeable observations"
    );
    let repeat = execute_approved_package(
        &first_package,
        &first_approved,
        request(vec![bool_value(true), bool_value(true)], limits),
    )
    .expect("deterministic repeat executes");
    assert_eq!(
        first_outcome.observation_id, repeat.observation_id,
        "the same package with the same inputs is deterministic"
    );
}

#[test]
fn broader_profile_operation_cannot_ride_a_bootstrap_receipt() {
    let float_program = float_add_program();
    let limits = generous_limits();
    let float_package = float_program.package_with_counts(limits, 1, 0, Vec::new());
    let bootstrap = bool_and_program();
    let (_, bootstrap_approved) = bootstrap.approved(limits);
    execute_approved_package(
        &float_package,
        &bootstrap_approved,
        request(
            vec![
                ConstValue {
                    value_type: TypeExpr::F32,
                    data: ConstData::F32Bits(0),
                },
                ConstValue {
                    value_type: TypeExpr::F32,
                    data: ConstData::F32Bits(0),
                },
            ],
            limits,
        ),
    )
    .expect_err("a broader-profile image under a bootstrap receipt must refuse");
}

/// `(F32, F32) -> F32` via `FloatAdd`: valid under `EXTENDED_V1` lowering
/// but outside `BOOTSTRAP_PROFILE_1`. Used only to prove out-of-profile
/// images cannot ride a bootstrap approval.
fn float_add_program() -> Program {
    let function = id(7);
    let block = id(8);
    let left = id(70);
    let right = id(71);
    let operation = id(170);
    let graph = FunctionGraph {
        entity_id: function,
        type_parameters: Vec::new(),
        parameters: vec![left, right],
        result_type: TypeExpr::F32,
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
                value_type: TypeExpr::F32,
            },
            Parameter {
                entity_id: right,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 1,
                value_type: TypeExpr::F32,
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
            opcode: Opcode::FloatAdd,
            operands: vec![ValueRef::Parameter(left), ValueRef::Parameter(right)],
            result_types: vec![TypeExpr::F32],
            immediate: Immediate::None,
        }],
        constants: Vec::new(),
        globals: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Nabu round-1 repairs: adjudicator-owned admission, envelope ceilings.
// ---------------------------------------------------------------------------

#[test]
fn same_package_out_of_bootstrap_approval_refuses() {
    let float_program = float_add_program();
    let gate_result = judge_bootstrap_profile(&BootstrapProfileInput {
        types: &float_program.types,
        schema_epoch: epoch(),
        entry: &float_program.entry,
        presented_image_bytes: &[],
        functions: std::slice::from_ref(&float_program.entry),
        parameters: &float_program.parameters,
        blocks: &float_program.blocks,
        operations: &float_program.operations,
        adapters: &float_program.adapters,
        constants: &float_program.constants,
    });
    assert!(
        gate_result.is_err(),
        "the admission authority refuses the broader-profile closure: no gate report exists for it"
    );
    let limits = generous_limits();
    // The fabrication path: package bytes (including counts) are writable
    // by anyone, but no gate report exists for this closure, so approval
    // must refuse against any genuine report.
    let float_package = float_program.package_with_counts(limits, 1, 0, Vec::new());
    let float_digests = package_digests(&float_package).expect("float package digests");
    let float_receipt = admit_package(float_digests.package_digest);
    let bootstrap = bool_and_program();
    let bootstrap_gate = bootstrap.gate_report(&[]);
    assert_eq!(
        approve_package(
            &float_package,
            &float_digests,
            float_receipt,
            &bootstrap_gate,
        )
        .unwrap_err(),
        PackageError::BindingMismatch,
        "a receipt plus another closure's gate report cannot approve an out-of-bootstrap package"
    );
}

#[test]
fn cross_closure_gate_report_refuses() {
    let program = bool_and_program();
    let other = constant_program(true);
    let limits = generous_limits();
    let package = program.package(limits);
    let digests = package_digests(&package).expect("package digests");
    let receipt = admit_package(digests.package_digest);
    let other_gate = other.gate_report(&[]);
    assert_eq!(
        approve_package(&package, &digests, receipt, &other_gate).unwrap_err(),
        PackageError::BindingMismatch,
        "a gate report for another closure cannot approve this package"
    );
}

#[test]
fn oversized_image_refuses() {
    let program = bool_and_program();
    let limits = generous_limits();
    let mut package = program.package(limits);
    package.image_bytes = vec![0u8; sley_vm::host_abi::IMAGE_MAX_BYTES + 1];
    assert_eq!(
        package_digests(&package).unwrap_err(),
        PackageError::Oversized,
        "image bytes past the image ceiling refuse before any hashing"
    );
}

#[test]
fn total_envelope_ceiling_enforced() {
    let program = bool_and_program();
    let limits = generous_limits();
    let mut package = program.package(limits);
    package.image_bytes = vec![0u8; sley_vm::exec_package::EXEC_PACKAGE_MAX_BYTES];
    assert_eq!(
        package_digests(&package).unwrap_err(),
        PackageError::Oversized,
        "an image within its own ceiling but over the total envelope with sections refuses"
    );
}

#[test]
fn dependency_count_ceiling_enforced() {
    let program = bool_and_program();
    let limits = generous_limits();
    let mut package = program.package(limits);
    package.globals = (0..=sley_vm::exec_package::EXEC_PACKAGE_MAX_GLOBALS)
        .map(|index| {
            let counter = u64::try_from(index).unwrap_or(u64::MAX).to_be_bytes();
            let mut identity = [0xEE_u8; 32];
            identity[..8].copy_from_slice(&counter);
            identity[8..16].copy_from_slice(&counter);
            identity[16..24].copy_from_slice(&counter);
            identity[24..32].copy_from_slice(&counter);
            GlobalValueDefinition {
                entity_id: EntityId::from_bytes(identity),
                value_type: TypeExpr::Bool,
                initializer: id(91),
                visibility: Visibility::Private,
            }
        })
        .collect();
    assert_eq!(
        package_digests(&package).unwrap_err(),
        PackageError::Oversized,
        "a dependency inventory past its count ceiling refuses"
    );
}

/// `(Bool, Bool) -> Bool` computing `AND(NOT left, right)`: same entry
/// identity and same (empty) import set as `bool_and_program`, but two
/// judged operations instead of one.
fn two_op_bool_program() -> Program {
    let function = id(1);
    let block = id(2);
    let left = id(10);
    let right = id(11);
    let not_operation = id(100);
    let and_operation = id(101);
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
                operands: vec![ValueRef::Parameter(left)],
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
                    ValueRef::Parameter(right),
                ],
                result_types: vec![TypeExpr::Bool],
                immediate: Immediate::None,
            },
        ],
        constants: Vec::new(),
        globals: Vec::new(),
        contracts: Vec::new(),
        adapters: Vec::new(),
    }
}

#[test]
fn same_entry_imports_different_counts_refuses() {
    let program = bool_and_program();
    let other = two_op_bool_program();
    let limits = generous_limits();
    let package = program.package(limits);
    let digests = package_digests(&package).expect("package digests");
    let receipt = admit_package(digests.package_digest);
    let other_gate = other.gate_report(&[]);
    assert_eq!(other_gate.operation_count(), 2);
    assert_eq!(
        approve_package(&package, &digests, receipt, &other_gate).unwrap_err(),
        PackageError::BindingMismatch,
        "entry and imports match, but the quantitative gate claims differ"
    );
}

#[test]
fn dependency_byte_ceiling_enforced_incrementally() {
    use sley_ssmc::{ContractBinding, ContractDefinition, ContractKind, ContractSource};
    let program = bool_and_program();
    let limits = generous_limits();
    let mut package = program.package(limits);
    package.contracts = (0..100_000u32)
        .map(|index| {
            let counter = index.to_be_bytes();
            let mut identity = [0xC0_u8; 32];
            identity[28..32].copy_from_slice(&counter);
            ContractDefinition {
                entity_id: EntityId::from_bytes(identity),
                target: id(1),
                contract_kind: ContractKind::Precondition,
                predicate: id(6),
                bindings: vec![ContractBinding {
                    predicate_parameter: 0,
                    source: ContractSource::Result,
                }],
                resource_limits: None,
            }
        })
        .collect();
    assert_eq!(
        package_digests(&package).unwrap_err(),
        PackageError::Oversized,
        "a dependency section past its byte ceiling refuses during encoding"
    );
}

/// `(Bool, Bool) -> Bool` computing `OR`: same entry identity, same
/// (empty) import set, and same gate counts (1 operation, 0 bridge uses)
/// as `bool_and_program`, but different judged operations — hence a
/// different closure fingerprint. The reviewer's equal-count replay case.
fn bool_or_program() -> Program {
    let mut program = bool_and_program();
    program.operations[0].opcode = Opcode::BoolOr;
    program
}

#[test]
fn equal_count_replay_with_different_closure_refuses() {
    let program = bool_and_program();
    let other = bool_or_program();
    let limits = generous_limits();
    let package = program.package(limits);
    let digests = package_digests(&package).expect("package digests");
    let receipt = admit_package(digests.package_digest);
    let other_gate = other.gate_report(&[]);
    assert_eq!(other_gate.operation_count(), 1);
    assert_eq!(other_gate.bridge_uses(), 0);
    assert_ne!(
        other_gate.closure_fingerprints(),
        program.gate_report(&[]).closure_fingerprints(),
        "same summaries, different judged operations: fingerprints differ"
    );
    assert_eq!(
        approve_package(&package, &digests, receipt, &other_gate).unwrap_err(),
        PackageError::BindingMismatch,
        "a genuine report for a different closure with equal counts cannot approve"
    );
}

#[test]
fn adversarial_copied_evidence_against_genuine_report_refuses() {
    let and_program = bool_and_program();
    let or_program = bool_or_program();
    let limits = generous_limits();
    // Genuine authority evidence for the AND closure, bound to the AND
    // image bytes presented at admission.
    let and_lowered = lower_function(and_program.lowering_input()).expect("and lowers");
    let and_gate = and_program.gate_report(&and_lowered.bytes);
    // The adversary builds an OR package but copies the AND closure's
    // counts and fingerprints into the package bytes, mints its own
    // receipt, and presents the genuine AND report. The sealed report's
    // committed image digest (AND bytes) does not match the package's
    // actual OR bytes, so approval refuses.
    let adversarial = or_program.package_with_counts(
        limits,
        and_gate.operation_count(),
        and_gate.bridge_uses(),
        and_gate.closure_fingerprints().to_vec(),
    );
    let adversarial_digests = package_digests(&adversarial).expect("adversarial digests");
    let adversarial_receipt = admit_package(adversarial_digests.package_digest);
    assert_eq!(
        approve_package(
            &adversarial,
            &adversarial_digests,
            adversarial_receipt,
            &and_gate,
        )
        .unwrap_err(),
        PackageError::BindingMismatch,
        "copied evidence cannot replay a genuine report against different executable bytes"
    );
}

/// `(Bool, Bool) -> Bool` computing `AND` with swapped operands: the same
/// opcode tag as `bool_and_program`, but different executable bytes
/// (operand order), hence a different image digest. The reviewer's
/// same-opcode rewiring case.
fn bool_and_swapped_program() -> Program {
    let mut program = bool_and_program();
    let left = program.parameters[0].entity_id;
    let right = program.parameters[1].entity_id;
    program.operations[0].operands = vec![ValueRef::Parameter(right), ValueRef::Parameter(left)];
    program
}

#[test]
fn same_opcode_rewired_image_against_genuine_report_refuses() {
    let and_program = bool_and_program();
    let swapped_program = bool_and_swapped_program();
    let limits = generous_limits();
    let and_lowered = lower_function(and_program.lowering_input()).expect("and lowers");
    let and_gate = and_program.gate_report(&and_lowered.bytes);
    let swapped_lowered = lower_function(swapped_program.lowering_input()).expect("swapped lowers");
    assert_ne!(
        swapped_lowered.bytes, and_lowered.bytes,
        "rewired operands change the executable bytes under identical tags"
    );
    // Same opcode multiset, different bytes: copied evidence plus the
    // genuine report still refuses on the committed image digest.
    let adversarial = swapped_program.package_with_counts(
        limits,
        and_gate.operation_count(),
        and_gate.bridge_uses(),
        and_gate.closure_fingerprints().to_vec(),
    );
    let adversarial_digests = package_digests(&adversarial).expect("adversarial digests");
    let adversarial_receipt = admit_package(adversarial_digests.package_digest);
    assert_eq!(
        approve_package(
            &adversarial,
            &adversarial_digests,
            adversarial_receipt,
            &and_gate,
        )
        .unwrap_err(),
        PackageError::BindingMismatch,
        "same-opcode rewiring changes image bytes and refuses on the committed digest"
    );
}

/// Models the admission authority's builder-faithfulness check (production:
/// the Sley build driver per the RW-080 contract; RW-075: this helper):
/// the authority re-lowers the judged graphs with the reference lowerer
/// and compares bytes before minting any receipt. A tampered image never
/// obtains a receipt, so no approval is constructible for it.
fn authority_admit(
    program: &Program,
    package: &ExecutionPackage,
) -> Result<sley_vm::AdmissionReceipt, PackageError> {
    let relowered = lower_function(program.lowering_input()).expect("authority re-lowers");
    if relowered.bytes != package.image_bytes {
        return Err(PackageError::BindingMismatch);
    }
    let digests = package_digests(package)?;
    Ok(admit_package(digests.package_digest))
}

#[test]
fn authority_refuses_tampered_image_before_any_receipt() {
    let program = bool_and_program();
    let limits = generous_limits();
    let (package, _) = program.approved(limits);
    authority_admit(&program, &package).expect("faithful builder image admits");
    let mut tampered = package.clone();
    let last = tampered.image_bytes.len() - 1;
    tampered.image_bytes[last] ^= 0x01;
    assert_eq!(
        authority_admit(&program, &tampered).unwrap_err(),
        PackageError::BindingMismatch,
        "reference re-lowering comparison refuses the tampered image: no receipt exists for it"
    );
}
