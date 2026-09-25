//! Package-path integer input width (2.0.1 hardening of the #7 follow-up).
//!
//! `execute_approved_package` and `execute_approved_package_v2` judge
//! runtime inputs structurally, not with `check_constant`, and the codec
//! carries integer widths without comparing data against them. These
//! fixtures use only the public `sley-vm` surface to pin that:
//!
//! * an integer input whose data lies outside its declared width, or
//!   whose data does not match its declared signedness, is refused before
//!   execution with `VM_EXEC_INPUT_NOT_CANONICAL`, at the top level and
//!   nested inside a container, on both package versions;
//! * the S20-270 lowering path still refuses the same input with its
//!   earlier `TYPE_CONST_RANGE` judgment;
//! * in-width inputs execute with unchanged answers, including each
//!   width's boundary values.

use sley_check::{TypeEnvironment, TypeErrorCode};
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    Block, BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue, FunctionGraph,
    Immediate, IntegerWidth, Opcode, Operation, OperationResultRef, Parameter, ParameterRole,
    Reachability, ResultConst, ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
};
use sley_vm::bootstrap::{BootstrapProfileInput, BootstrapProfileVersion, judge_bootstrap_profile};
use sley_vm::{
    ApprovedExecutionPackage, CacheProfile, ExecutionError, ExecutionErrorCode, ExecutionLimits,
    ExecutionOutcome, ExecutionPackage, ExecutionRequest, ExecutionTermination, LoweringInput,
    PackageExecutionError, V2Closure, admit_package, admit_v2_package, approve_package,
    approve_package_v2, execute_approved_package, execute_approved_package_v2, execute_function,
    lower_function, package_digests,
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

fn limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 10_000,
        max_fuel: 100_000,
        max_value_units: 1_000_000,
        max_output_units: 100_000,
        cancel_at_fuel: None,
    }
}

fn sint_type(bits: u16) -> TypeExpr {
    TypeExpr::SInt(IntegerWidth::from_bits(bits))
}

fn uint_type(bits: u16) -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(bits))
}

fn sint(bits: u16, value: i128) -> ConstValue {
    ConstValue {
        value_type: sint_type(bits),
        data: ConstData::SInt(value),
    }
}

fn uint(bits: u16, value: u128) -> ConstValue {
    ConstValue {
        value_type: uint_type(bits),
        data: ConstData::UInt(value),
    }
}

fn arithmetic(value_type: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(value_type),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}

/// One single-block Function plus the inventories its package carries.
struct Program {
    types: TypeEnvironment,
    entry: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

impl Program {
    /// `(T, T) -> Result<T, Arithmetic>` (the shift amount is `UInt(32)`),
    /// or `(T) -> Result<T, Arithmetic>` for negation.
    fn checked(opcode: Opcode, value_type: &TypeExpr) -> Self {
        let parameter_types = match opcode {
            Opcode::IntNegChecked => vec![value_type.clone()],
            Opcode::IntShlChecked | Opcode::IntShrChecked => {
                vec![value_type.clone(), uint_type(32)]
            }
            _ => vec![value_type.clone(), value_type.clone()],
        };
        let operation = id(100);
        let result_type = arithmetic(value_type.clone());
        let parameter_ids = (0..parameter_types.len())
            .map(|index| id(10 + u8::try_from(index).unwrap()))
            .collect::<Vec<_>>();
        let operations = vec![Operation {
            entity_id: operation,
            block: id(2),
            ordinal: 0,
            opcode,
            operands: parameter_ids
                .iter()
                .map(|parameter| ValueRef::Parameter(*parameter))
                .collect(),
            result_types: vec![result_type.clone()],
            immediate: Immediate::None,
        }];
        Self::build(
            parameter_types,
            result_type,
            operations,
            ValueRef::OperationResult(OperationResultRef {
                operation,
                result_index: 0,
            }),
        )
    }

    /// `(T) -> T`: returns its parameter, so the input itself is the result.
    fn identity(value_type: &TypeExpr) -> Self {
        Self::build(
            vec![value_type.clone()],
            value_type.clone(),
            Vec::new(),
            ValueRef::Parameter(id(10)),
        )
    }

    fn build(
        parameter_types: Vec<TypeExpr>,
        result_type: TypeExpr,
        operations: Vec<Operation>,
        returned: ValueRef,
    ) -> Self {
        let function = id(1);
        let block = id(2);
        let parameters = parameter_types
            .into_iter()
            .enumerate()
            .map(|(index, value_type)| Parameter {
                entity_id: id(10 + u8::try_from(index).unwrap()),
                owner: function,
                role: ParameterRole::Function,
                ordinal: u32::try_from(index).unwrap(),
                value_type,
            })
            .collect::<Vec<_>>();
        Self {
            types: TypeEnvironment::new(Vec::new()).unwrap(),
            entry: FunctionGraph {
                entity_id: function,
                type_parameters: Vec::new(),
                parameters: parameters
                    .iter()
                    .map(|parameter| parameter.entity_id)
                    .collect(),
                result_type,
                effects: Vec::new(),
                entry_block: block,
                blocks: vec![block],
                contracts: Vec::new(),
                visibility: Visibility::Private,
            },
            parameters,
            blocks: vec![Block {
                entity_id: block,
                function,
                parameters: Vec::new(),
                operations: operations
                    .iter()
                    .map(|operation| operation.entity_id)
                    .collect(),
                terminator: Terminator::Return(ReturnTerminator { value: returned }),
                reachability: Reachability::Required,
            }],
            operations,
        }
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
            adapters: &[],
        }
    }

    fn package(&self, version: BootstrapProfileVersion) -> ExecutionPackage {
        let lowered = lower_function(self.lowering_input()).expect("fixture lowers");
        let gate = self.gate(&lowered.bytes, version);
        ExecutionPackage {
            image_bytes: lowered.bytes.clone(),
            constants: Vec::new(),
            type_definitions: Vec::new(),
            imports: Vec::new(),
            globals: Vec::new(),
            contracts: Vec::new(),
            entry: self.entry.entity_id,
            schema_epoch: epoch(),
            state_root: root(),
            profile: CacheProfile::EXTENDED_V1,
            admitted_limits: limits(),
            gate_operation_count: gate.operation_count(),
            gate_bridge_uses: gate.bridge_uses(),
            gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
        }
    }

    fn gate(
        &self,
        image: &[u8],
        version: BootstrapProfileVersion,
    ) -> sley_vm::bootstrap::BootstrapProfileReport {
        judge_bootstrap_profile(&BootstrapProfileInput {
            types: &self.types,
            schema_epoch: epoch(),
            entry: &self.entry,
            presented_image_bytes: image,
            functions: std::slice::from_ref(&self.entry),
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            adapters: &[],
            constants: &[],
            profile_version: version,
        })
        .expect("fixture closure is gate-admitted")
    }

    fn approved_v1(&self) -> (ExecutionPackage, ApprovedExecutionPackage) {
        let package = self.package(BootstrapProfileVersion::V1);
        let digests = package_digests(&package).expect("fixture digests");
        let receipt = admit_package(digests.package_digest);
        let gate = self.gate(&package.image_bytes, BootstrapProfileVersion::V1);
        let approved =
            approve_package(&package, &digests, receipt, &gate).expect("fixture approves");
        (package, approved)
    }

    /// The v2 package minted by the production staged authority.
    fn approved_v2(&self) -> (ExecutionPackage, ApprovedExecutionPackage) {
        let package = self.package(BootstrapProfileVersion::V2);
        let closure = V2Closure {
            types: &self.types,
            schema_epoch: epoch(),
            state_root: root(),
            entry: self.entry.entity_id,
            functions: std::slice::from_ref(&self.entry),
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            adapters: &[],
            constants: &[],
            globals: &[],
            contracts: &[],
        };
        let (digests, receipt, gate) =
            admit_v2_package(&closure, &package).expect("authority admits the fixture");
        let approved = approve_package_v2(&package, &digests, receipt, &gate).expect("v2 approves");
        (package, approved)
    }

    /// Runs `inputs` through both package versions and requires them to agree.
    fn run(&self, inputs: &[ConstValue]) -> Result<ExecutionOutcome, PackageExecutionError> {
        let request = || ExecutionRequest {
            inputs: inputs.to_vec(),
            limits: limits(),
        };
        let (package, approved) = self.approved_v1();
        let v1 = execute_approved_package(&package, &approved, request());
        let (package, approved) = self.approved_v2();
        let v2 = execute_approved_package_v2(&package, &approved, request());
        match (&v1, &v2) {
            (Ok(v1), Ok(v2)) => assert_eq!(v1.termination, v2.termination),
            (Err(v1), Err(v2)) => assert_eq!(v1, v2),
            _ => panic!("package versions disagree: v1 {v1:?}, v2 {v2:?}"),
        }
        v2
    }

    fn lowering_path(&self, inputs: &[ConstValue]) -> Result<ExecutionOutcome, ExecutionError> {
        execute_function(
            self.lowering_input(),
            ExecutionRequest {
                inputs: inputs.to_vec(),
                limits: limits(),
            },
        )
    }
}

fn assert_not_canonical(result: Result<ExecutionOutcome, PackageExecutionError>, case: &str) {
    match result {
        Err(PackageExecutionError::Execution(ExecutionError::Exec(
            ExecutionErrorCode::InputNotCanonical,
        ))) => {}
        other => panic!("{case}: expected VM_EXEC_INPUT_NOT_CANONICAL, got {other:?}"),
    }
}

/// The `Ok` payload of a checked-arithmetic success, or the failure code.
fn arithmetic_answer(outcome: &ExecutionOutcome) -> Result<ConstData, u16> {
    let ExecutionTermination::Success(value) = &outcome.termination else {
        panic!("expected success, got {:?}", outcome.termination);
    };
    match &value.data {
        ConstData::Result(ResultConst::Ok(value)) => Ok(value.data.clone()),
        ConstData::Result(ResultConst::Err(failure)) => match failure.data {
            ConstData::BuiltinFailure(BuiltinFailureValue {
                kind: BuiltinFailureKind::Arithmetic,
                code,
            }) => Err(code),
            ref other => panic!("unexpected failure data {other:?}"),
        },
        other => panic!("unexpected result {other:?}"),
    }
}

#[test]
fn out_of_width_integer_inputs_are_refused_before_execution() {
    // `shr` would have answered `Ok(500)` in a `UInt(8)` result, and `div`
    // an in-width `Ok(100)` computed from a garbage operand.
    let shr = Program::checked(Opcode::IntShrChecked, &uint_type(8));
    assert_not_canonical(shr.run(&[uint(8, 1_000), uint(32, 1)]), "u8 shr operand");
    assert_not_canonical(
        shr.run(&[uint(8, 1), uint(32, 1 << 40)]),
        "u32 shift amount",
    );
    let div = Program::checked(Opcode::IntDivChecked, &uint_type(8));
    assert_not_canonical(div.run(&[uint(8, 1_000), uint(8, 10)]), "u8 dividend");
    assert_not_canonical(div.run(&[uint(8, 10), uint(8, 256)]), "u8 divisor");
    let rem = Program::checked(Opcode::IntRemChecked, &sint_type(64));
    assert_not_canonical(
        rem.run(&[sint(64, i128::MIN), sint(64, -1)]),
        "i64 dividend at i128::MIN",
    );
    let add = Program::checked(Opcode::IntAddChecked, &sint_type(8));
    assert_not_canonical(add.run(&[sint(8, 1), sint(8, -129)]), "i8 addend");
    let neg = Program::checked(Opcode::IntNegChecked, &sint_type(32));
    assert_not_canonical(
        neg.run(&[sint(32, i128::from(i32::MAX) + 1)]),
        "i32 negation operand",
    );
    // Data of the other signedness is not an integer of the declared type.
    assert_not_canonical(
        add.run(&[
            sint(8, 1),
            ConstValue {
                value_type: sint_type(8),
                data: ConstData::UInt(1),
            },
        ]),
        "unsigned data under SInt(8)",
    );
    // The S20-270 lowering path keeps its earlier judgment for the same input.
    match div.lowering_path(&[uint(8, 1_000), uint(8, 10)]) {
        Err(ExecutionError::Type(error)) => assert_eq!(error.code(), TypeErrorCode::ConstRange),
        other => panic!("lowering path: expected TYPE_CONST_RANGE, got {other:?}"),
    }
}

#[test]
fn nested_out_of_width_integers_are_refused_before_execution() {
    let option = TypeExpr::Option(Box::new(uint_type(8)));
    let program = Program::identity(&option);
    assert_not_canonical(
        program.run(&[ConstValue {
            value_type: option.clone(),
            data: ConstData::Option(Some(Box::new(uint(8, 300)))),
        }]),
        "u8 inside Option",
    );
    let vector = TypeExpr::Vector(Box::new(sint_type(16)));
    let program = Program::identity(&vector);
    assert_not_canonical(
        program.run(&[ConstValue {
            value_type: vector.clone(),
            data: ConstData::Sequence(vec![sint(16, 1), sint(16, i128::from(i16::MIN) - 1)]),
        }]),
        "i16 inside Vector",
    );
    // In width, the same shapes return exactly what they were given.
    for value in [
        ConstValue {
            value_type: option.clone(),
            data: ConstData::Option(Some(Box::new(uint(8, 255)))),
        },
        ConstValue {
            value_type: vector.clone(),
            data: ConstData::Sequence(vec![
                sint(16, i128::from(i16::MIN)),
                sint(16, i128::from(i16::MAX)),
            ]),
        },
    ] {
        let program = Program::identity(&value.value_type);
        let outcome = program.run(std::slice::from_ref(&value)).expect("executes");
        assert_eq!(outcome.termination, ExecutionTermination::Success(value));
    }
}

#[test]
fn in_width_integer_inputs_keep_their_exact_answers() {
    let shr = Program::checked(Opcode::IntShrChecked, &uint_type(8));
    assert_eq!(
        arithmetic_answer(&shr.run(&[uint(8, 255), uint(32, 1)]).expect("executes")),
        Ok(ConstData::UInt(127))
    );
    assert_eq!(
        arithmetic_answer(&shr.run(&[uint(8, 1), uint(32, 8)]).expect("executes")),
        Err(3),
        "a shift of the width is the invalid-shift code"
    );
    let div = Program::checked(Opcode::IntDivChecked, &sint_type(8));
    assert_eq!(
        arithmetic_answer(&div.run(&[sint(8, -7), sint(8, 2)]).expect("executes")),
        Ok(ConstData::SInt(-3))
    );
    assert_eq!(
        arithmetic_answer(&div.run(&[sint(8, -128), sint(8, -1)]).expect("executes")),
        Err(1),
        "the width's own MIN / -1 overflows"
    );
    let rem = Program::checked(Opcode::IntRemChecked, &uint_type(128));
    assert_eq!(
        arithmetic_answer(
            &rem.run(&[uint(128, u128::MAX), uint(128, 10)])
                .expect("executes")
        ),
        Ok(ConstData::UInt(u128::MAX % 10))
    );
    let add = Program::checked(Opcode::IntAddChecked, &sint_type(64));
    assert_eq!(
        arithmetic_answer(
            &add.run(&[sint(64, i128::from(i64::MAX)), sint(64, 0)])
                .expect("executes")
        ),
        Ok(ConstData::SInt(i128::from(i64::MAX)))
    );
    // Package and lowering paths agree on in-width inputs.
    let inputs = [sint(8, -7), sint(8, 2)];
    assert_eq!(
        div.run(&inputs).expect("package").termination,
        div.lowering_path(&inputs).expect("lowering").termination
    );
}
