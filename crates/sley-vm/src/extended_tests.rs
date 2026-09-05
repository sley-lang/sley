//! Extended opcode profile tests (slice E1): positive semantics for the data
//! family, the lowering rejection matrix, repeat determinism, the restricted
//! profile's continued refusal, and the fixture emitter.

#![allow(clippy::too_many_lines)]

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    Block, BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue, ConstantDefinition,
    ContractBinding, ContractDefinition, ContractKind, ContractSource, FieldConst, FunctionGraph,
    FunctionRefValue, FunctionType, GlobalValueDefinition, Immediate, IntegerWidth, MapEntryConst,
    MemberId, NamedType, Opcode, Operation, OperationResultRef, Parameter, ParameterRole,
    Reachability, RecordConst, RecordField, ResourceLimits, ResultConst, ReturnTerminator,
    Terminator, TypeDefForm, TypeDefinition, TypeExpr, ValueRef, VariantCase, VariantConst,
    VariantImmediate, Visibility, fingerprint::hash_validated_value,
};

use crate::{
    CacheProfile, ExecutionError, ExecutionLimits, ExecutionRequest, ExecutionTermination,
    LowerErrorCode, LoweringError, LoweringInput, ResourceKind, execute_function,
    judge_function_operations, lower_function,
};

fn id(byte: u8) -> EntityId {
    EntityId::from_bytes([byte; 32])
}

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}

fn uint(value: u128) -> ConstValue {
    ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(value),
    }
}

fn sint(value: i128) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::SInt(IntegerWidth::from_bits(32)),
        data: ConstData::SInt(value),
    }
}

fn boolean(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

fn text(value: &str) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Text,
        data: ConstData::Text(value.to_string()),
    }
}

/// One operation of a straight-line fixture: opcode, operands (parameter
/// index `P(i)` or earlier operation index `R(i)`), immediate, declared result.
#[derive(Clone)]
enum Arg {
    P(usize),
    R(usize),
}

struct Step {
    opcode: Opcode,
    operands: Vec<Arg>,
    immediate: Immediate,
    result: TypeExpr,
}

fn step(opcode: Opcode, operands: Vec<Arg>, immediate: Immediate, result: TypeExpr) -> Step {
    Step {
        opcode,
        operands,
        immediate,
        result,
    }
}

/// A one-block Function over the given parameter types whose steps run in
/// order and whose result is the last step's result.
struct Fixture {
    types: TypeEnvironment,
    function: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
    globals: Vec<GlobalValueDefinition>,
    functions: Vec<FunctionGraph>,
    contracts: Vec<ContractDefinition>,
}

impl Fixture {
    fn new(
        parameter_types: &[TypeExpr],
        steps: &[Step],
        constants: Vec<ConstantDefinition>,
    ) -> Self {
        Self::with_types(parameter_types, steps, constants, Vec::new())
    }

    fn with_types(
        parameter_types: &[TypeExpr],
        steps: &[Step],
        constants: Vec<ConstantDefinition>,
        definitions: Vec<TypeDefinition>,
    ) -> Self {
        Self::with_base(0, parameter_types, steps, constants, definitions)
    }

    /// Builds a fixture whose entity ids are offset by `base` so several
    /// fixtures can share one inventory (direct calls).
    fn with_base(
        base: u8,
        parameter_types: &[TypeExpr],
        steps: &[Step],
        constants: Vec<ConstantDefinition>,
        definitions: Vec<TypeDefinition>,
    ) -> Self {
        let function = id(base + 1);
        let block = id(base + 2);
        let parameter_ids: Vec<EntityId> = (0..parameter_types.len())
            .map(|index| id(base + u8::try_from(10 + index).unwrap()))
            .collect();
        let operation_ids: Vec<EntityId> = (0..steps.len())
            .map(|index| id(base + u8::try_from(100 + index).unwrap()))
            .collect();
        let resolve = |arg: &Arg| match arg {
            Arg::P(index) => ValueRef::Parameter(parameter_ids[*index]),
            Arg::R(index) => ValueRef::OperationResult(OperationResultRef {
                operation: operation_ids[*index],
                result_index: 0,
            }),
        };
        let operations: Vec<Operation> = steps
            .iter()
            .enumerate()
            .map(|(index, step)| Operation {
                entity_id: operation_ids[index],
                block,
                ordinal: u32::try_from(index).unwrap(),
                opcode: step.opcode,
                operands: step.operands.iter().map(resolve).collect(),
                result_types: vec![step.result.clone()],
                immediate: step.immediate.clone(),
            })
            .collect();
        let last = steps.last().expect("at least one step");
        Self {
            types: TypeEnvironment::new(definitions).unwrap(),
            function: FunctionGraph {
                entity_id: function,
                type_parameters: Vec::new(),
                parameters: parameter_ids.clone(),
                result_type: last.result.clone(),
                effects: Vec::new(),
                entry_block: block,
                blocks: vec![block],
                contracts: Vec::new(),
                visibility: Visibility::Private,
            },
            parameters: parameter_types
                .iter()
                .enumerate()
                .map(|(index, value_type)| Parameter {
                    entity_id: parameter_ids[index],
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: u32::try_from(index).unwrap(),
                    value_type: value_type.clone(),
                })
                .collect(),
            blocks: vec![Block {
                entity_id: block,
                function,
                parameters: Vec::new(),
                operations: operation_ids.clone(),
                terminator: Terminator::Return(ReturnTerminator {
                    value: ValueRef::OperationResult(OperationResultRef {
                        operation: *operation_ids.last().unwrap(),
                        result_index: 0,
                    }),
                }),
                reachability: Reachability::Required,
            }],
            operations,
            constants,
            globals: Vec::new(),
            functions: Vec::new(),
            contracts: Vec::new(),
        }
    }

    fn with_globals(
        mut self,
        globals: Vec<GlobalValueDefinition>,
        constants: Vec<ConstantDefinition>,
    ) -> Self {
        self.globals = globals;
        self.constants.extend(constants);
        self
    }

    fn with_contracts(mut self, contracts: Vec<ContractDefinition>) -> Self {
        self.contracts = contracts;
        self
    }

    fn with_functions(mut self, functions: Vec<FunctionGraph>) -> Self {
        self.functions = functions;
        self
    }

    /// Merges a callee fixture into this inventory and Function list.
    fn with_callee(mut self, callee: Fixture) -> Self {
        self.parameters.extend(callee.parameters);
        self.blocks.extend(callee.blocks);
        self.operations.extend(callee.operations);
        self.constants.extend(callee.constants);
        self.functions.push(callee.function);
        self
    }

    /// Lists the entry Function in its own inventory (self-calls).
    fn with_self(mut self) -> Self {
        self.functions.push(self.function.clone());
        self
    }

    fn input(&self, profile: CacheProfile) -> LoweringInput<'_> {
        LoweringInput {
            types: &self.types,
            function: &self.function,
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            schema_epoch: SchemaEpochId::from_bytes([8; 32]),
            state_root: StateRoot::from_bytes([9; 32]),
            profile,
            constants: &self.constants,
            globals: &self.globals,
            functions: &self.functions,
            contracts: &self.contracts,
        }
    }
}

fn limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 1_000,
        max_fuel: 10_000,
        max_value_units: 100_000,
        max_output_units: 10_000,
        cancel_at_fuel: None,
    }
}

fn run_extended(fixture: &Fixture, inputs: Vec<ConstValue>) -> ExecutionTermination {
    execute_function(
        fixture.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs,
            limits: limits(),
        },
    )
    .expect("executes")
    .termination
}

fn success(fixture: &Fixture, inputs: Vec<ConstValue>) -> ConstValue {
    match run_extended(fixture, inputs) {
        ExecutionTermination::Success(value) => value,
        other => panic!("not a success: {other:?}"),
    }
}

fn lowering_code(fixture: &Fixture) -> LowerErrorCode {
    match lower_function(fixture.input(CacheProfile::EXTENDED_V1)).unwrap_err() {
        LoweringError::Lower(error) => error.code(),
        LoweringError::Cfg(error) => panic!("cfg failure: {error}"),
    }
}

fn vector_of(items: Vec<ConstValue>, element: TypeExpr) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Vector(Box::new(element)),
        data: ConstData::Sequence(items),
    }
}

#[test]
fn e1_tuples_vectors_options_and_results_construct_and_project() {
    // tuple_new(a: UInt64, b: Text) then tuple_get index 1 -> Text
    let tuple = Fixture::new(
        &[u64_type(), TypeExpr::Text],
        &[
            step(
                Opcode::TupleNew,
                vec![Arg::P(0), Arg::P(1)],
                Immediate::None,
                TypeExpr::Tuple(vec![u64_type(), TypeExpr::Text]),
            ),
            step(
                Opcode::TupleGet,
                vec![Arg::R(0)],
                Immediate::Index(1),
                TypeExpr::Text,
            ),
        ],
        Vec::new(),
    );
    assert_eq!(success(&tuple, vec![uint(7), text("seven")]), text("seven"));

    // vector_new(a, b, c) ; vector_len ; vector_get 1 ; vector_set 5 -> Err(Index)
    let vector = Fixture::new(
        &[u64_type(), u64_type(), u64_type()],
        &[
            step(
                Opcode::VectorNew,
                vec![Arg::P(0), Arg::P(1), Arg::P(2)],
                Immediate::None,
                TypeExpr::Vector(Box::new(u64_type())),
            ),
            step(
                Opcode::VectorLen,
                vec![Arg::R(0)],
                Immediate::None,
                u64_type(),
            ),
        ],
        Vec::new(),
    );
    assert_eq!(success(&vector, vec![uint(1), uint(2), uint(3)]), uint(3));
    let get = Fixture::new(
        &[u64_type(), u64_type()],
        &[
            step(
                Opcode::VectorNew,
                vec![Arg::P(0)],
                Immediate::None,
                TypeExpr::Vector(Box::new(u64_type())),
            ),
            step(
                Opcode::VectorGet,
                vec![Arg::R(0), Arg::P(1)],
                Immediate::None,
                TypeExpr::Option(Box::new(u64_type())),
            ),
        ],
        Vec::new(),
    );
    let some = success(&get, vec![uint(42), uint(0)]);
    assert_eq!(some.data, ConstData::Option(Some(Box::new(uint(42)))));
    let none = success(&get, vec![uint(42), uint(1)]);
    assert_eq!(none.data, ConstData::Option(None));
    let set_type = TypeExpr::Result {
        ok: Box::new(TypeExpr::Vector(Box::new(u64_type()))),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
    };
    let set = Fixture::new(
        &[u64_type(), u64_type(), u64_type()],
        &[
            step(
                Opcode::VectorNew,
                vec![Arg::P(0)],
                Immediate::None,
                TypeExpr::Vector(Box::new(u64_type())),
            ),
            step(
                Opcode::VectorSet,
                vec![Arg::R(0), Arg::P(1), Arg::P(2)],
                Immediate::None,
                set_type.clone(),
            ),
        ],
        Vec::new(),
    );
    let ok = success(&set, vec![uint(1), uint(0), uint(9)]);
    assert_eq!(
        ok.data,
        ConstData::Result(ResultConst::Ok(Box::new(vector_of(
            vec![uint(9)],
            u64_type()
        ))))
    );
    let err = success(&set, vec![uint(1), uint(3), uint(9)]);
    let ConstData::Result(ResultConst::Err(failure)) = err.data else {
        panic!("expected an index failure");
    };
    assert_eq!(
        failure.data,
        ConstData::BuiltinFailure(BuiltinFailureValue {
            kind: BuiltinFailureKind::Index,
            code: 1
        })
    );
    assert_eq!(
        failure.value_type,
        TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)
    );

    // option_some / option_none / result_ok / result_err
    let option = Fixture::new(
        &[TypeExpr::Text],
        &[step(
            Opcode::OptionSome,
            vec![Arg::P(0)],
            Immediate::None,
            TypeExpr::Option(Box::new(TypeExpr::Text)),
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&option, vec![text("x")]).data,
        ConstData::Option(Some(Box::new(text("x"))))
    );
    let none = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::OptionNone,
            vec![],
            Immediate::None,
            TypeExpr::Option(Box::new(TypeExpr::Text)),
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&none, vec![boolean(true)]).data,
        ConstData::Option(None)
    );
    let result_type = TypeExpr::Result {
        ok: Box::new(u64_type()),
        error: Box::new(TypeExpr::Text),
    };
    let ok = Fixture::new(
        &[u64_type()],
        &[step(
            Opcode::ResultOk,
            vec![Arg::P(0)],
            Immediate::None,
            result_type.clone(),
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&ok, vec![uint(5)]).data,
        ConstData::Result(ResultConst::Ok(Box::new(uint(5))))
    );
    let err = Fixture::new(
        &[TypeExpr::Text],
        &[step(
            Opcode::ResultErr,
            vec![Arg::P(0)],
            Immediate::None,
            result_type,
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&err, vec![text("bad")]).data,
        ConstData::Result(ResultConst::Err(Box::new(text("bad"))))
    );
    // An empty vector takes its element type from the declared result.
    let empty = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::VectorNew,
            vec![],
            Immediate::None,
            TypeExpr::Vector(Box::new(TypeExpr::Text)),
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&empty, vec![boolean(false)]),
        vector_of(Vec::new(), TypeExpr::Text)
    );
}

#[test]
fn e1_constants_and_comparisons_follow_the_contract() {
    let constant = ConstantDefinition {
        entity_id: id(200),
        value: text("frozen"),
    };
    let reference = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::ConstantRef,
            vec![],
            Immediate::Entity(id(200)),
            TypeExpr::Text,
        )],
        vec![constant],
    );
    assert_eq!(success(&reference, vec![boolean(true)]), text("frozen"));

    let compare = |opcode: Opcode, operand_type: TypeExpr| {
        Fixture::new(
            &[operand_type.clone(), operand_type],
            &[step(
                opcode,
                vec![Arg::P(0), Arg::P(1)],
                Immediate::None,
                TypeExpr::Bool,
            )],
            Vec::new(),
        )
    };
    let signed = compare(
        Opcode::LessThan,
        TypeExpr::SInt(IntegerWidth::from_bits(32)),
    );
    assert_eq!(success(&signed, vec![sint(-3), sint(2)]), boolean(true));
    assert_eq!(success(&signed, vec![sint(2), sint(-3)]), boolean(false));
    let unsigned = compare(Opcode::GreaterEqual, u64_type());
    assert_eq!(success(&unsigned, vec![uint(7), uint(7)]), boolean(true));
    assert_eq!(success(&unsigned, vec![uint(6), uint(7)]), boolean(false));
    let texts = compare(Opcode::LessEqual, TypeExpr::Text);
    assert_eq!(
        success(&texts, vec![text("ab"), text("abc")]),
        boolean(true)
    );
    assert_eq!(
        success(&texts, vec![text("b"), text("abc")]),
        boolean(false)
    );
    let bools = compare(Opcode::GreaterThan, TypeExpr::Bool);
    assert_eq!(
        success(&bools, vec![boolean(true), boolean(false)]),
        boolean(true)
    );
    let equal = compare(
        Opcode::Equal,
        TypeExpr::Tuple(vec![u64_type(), TypeExpr::Text]),
    );
    let pair = |n: u128, t: &str| ConstValue {
        value_type: TypeExpr::Tuple(vec![u64_type(), TypeExpr::Text]),
        data: ConstData::Sequence(vec![uint(n), text(t)]),
    };
    assert_eq!(
        success(&equal, vec![pair(1, "a"), pair(1, "a")]),
        boolean(true)
    );
    assert_eq!(
        success(&equal, vec![pair(1, "a"), pair(2, "a")]),
        boolean(false)
    );
    let not_equal = compare(Opcode::NotEqual, TypeExpr::Bytes);
    let bytes = |b: &[u8]| ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(b.to_vec()),
    };
    assert_eq!(
        success(&not_equal, vec![bytes(b"a"), bytes(b"b")]),
        boolean(true)
    );
    // Restricted Boolean opcodes keep their meaning under the extended profile.
    let and = compare(Opcode::BoolAnd, TypeExpr::Bool);
    assert_eq!(
        success(&and, vec![boolean(true), boolean(false)]),
        boolean(false)
    );
}

#[test]
fn e1_rejection_matrix_names_the_frozen_lowering_codes() {
    let cases: Vec<(&str, Fixture, LowerErrorCode)> = vec![
        (
            "tuple_get index out of range",
            Fixture::new(
                &[u64_type()],
                &[
                    step(
                        Opcode::TupleNew,
                        vec![Arg::P(0)],
                        Immediate::None,
                        TypeExpr::Tuple(vec![u64_type()]),
                    ),
                    step(
                        Opcode::TupleGet,
                        vec![Arg::R(0)],
                        Immediate::Index(1),
                        u64_type(),
                    ),
                ],
                Vec::new(),
            ),
            LowerErrorCode::ImmediateMismatch,
        ),
        (
            "tuple_get without an index immediate",
            Fixture::new(
                &[u64_type()],
                &[
                    step(
                        Opcode::TupleNew,
                        vec![Arg::P(0)],
                        Immediate::None,
                        TypeExpr::Tuple(vec![u64_type()]),
                    ),
                    step(
                        Opcode::TupleGet,
                        vec![Arg::R(0)],
                        Immediate::None,
                        u64_type(),
                    ),
                ],
                Vec::new(),
            ),
            LowerErrorCode::ImmediateMismatch,
        ),
        (
            "vector_new over mixed element types",
            Fixture::new(
                &[u64_type(), TypeExpr::Text],
                &[step(
                    Opcode::VectorNew,
                    vec![Arg::P(0), Arg::P(1)],
                    Immediate::None,
                    TypeExpr::Vector(Box::new(u64_type())),
                )],
                Vec::new(),
            ),
            LowerErrorCode::SignatureMismatch,
        ),
        (
            "vector_get index of the wrong width",
            Fixture::new(
                &[u64_type(), TypeExpr::SInt(IntegerWidth::from_bits(32))],
                &[
                    step(
                        Opcode::VectorNew,
                        vec![Arg::P(0)],
                        Immediate::None,
                        TypeExpr::Vector(Box::new(u64_type())),
                    ),
                    step(
                        Opcode::VectorGet,
                        vec![Arg::R(0), Arg::P(1)],
                        Immediate::None,
                        TypeExpr::Option(Box::new(u64_type())),
                    ),
                ],
                Vec::new(),
            ),
            LowerErrorCode::SignatureMismatch,
        ),
        (
            "declared result differs from the derived result",
            Fixture::new(
                &[u64_type()],
                &[step(
                    Opcode::OptionSome,
                    vec![Arg::P(0)],
                    Immediate::None,
                    TypeExpr::Option(Box::new(TypeExpr::Text)),
                )],
                Vec::new(),
            ),
            LowerErrorCode::SignatureMismatch,
        ),
        (
            "order predicate over an unordered type",
            Fixture::new(
                &[TypeExpr::Tuple(vec![]), TypeExpr::Tuple(vec![])],
                &[step(
                    Opcode::LessThan,
                    vec![Arg::P(0), Arg::P(1)],
                    Immediate::None,
                    TypeExpr::Bool,
                )],
                Vec::new(),
            ),
            LowerErrorCode::SignatureMismatch,
        ),
        (
            "equality over a non-hashable type",
            Fixture::new(
                &[
                    TypeExpr::LocalCell(Box::new(TypeExpr::Bool)),
                    TypeExpr::LocalCell(Box::new(TypeExpr::Bool)),
                ],
                &[step(
                    Opcode::Equal,
                    vec![Arg::P(0), Arg::P(1)],
                    Immediate::None,
                    TypeExpr::Bool,
                )],
                Vec::new(),
            ),
            LowerErrorCode::SignatureMismatch,
        ),
        (
            "constant_ref naming no constant",
            Fixture::new(
                &[TypeExpr::Bool],
                &[step(
                    Opcode::ConstantRef,
                    vec![],
                    Immediate::Entity(id(201)),
                    TypeExpr::Text,
                )],
                Vec::new(),
            ),
            LowerErrorCode::ImmediateMismatch,
        ),
        (
            "an opcode outside the profile",
            Fixture::new(
                &[u64_type()],
                &[step(
                    Opcode::EffectRequest,
                    vec![Arg::P(0)],
                    Immediate::None,
                    u64_type(),
                )],
                Vec::new(),
            ),
            LowerErrorCode::OpcodeUnsupported,
        ),
        (
            "an E7 opcode that slice E7a did not land",
            Fixture::new(
                &[TypeExpr::Bool],
                &[step(
                    Opcode::TestObserve,
                    vec![Arg::P(0)],
                    Immediate::Observation([0; 32]),
                    TypeExpr::Unit,
                )],
                Vec::new(),
            ),
            LowerErrorCode::OpcodeUnsupported,
        ),
        (
            "an immediate on an immediate-free opcode",
            Fixture::new(
                &[TypeExpr::Bool],
                &[step(
                    Opcode::OptionSome,
                    vec![Arg::P(0)],
                    Immediate::Index(0),
                    TypeExpr::Option(Box::new(TypeExpr::Bool)),
                )],
                Vec::new(),
            ),
            LowerErrorCode::ImmediateMismatch,
        ),
    ];
    for (label, fixture, expected) in cases {
        assert_eq!(lowering_code(&fixture), expected, "{label}");
    }
    // The restricted profile still refuses every E1 opcode.
    let restricted = Fixture::new(
        &[TypeExpr::Text],
        &[step(
            Opcode::OptionSome,
            vec![Arg::P(0)],
            Immediate::None,
            TypeExpr::Option(Box::new(TypeExpr::Text)),
        )],
        Vec::new(),
    );
    let LoweringError::Lower(error) =
        lower_function(restricted.input(CacheProfile::RESTRICTED_V1)).unwrap_err()
    else {
        panic!("lowering failure expected");
    };
    assert_eq!(error.code(), LowerErrorCode::OpcodeUnsupported);
}

#[test]
fn e1_bytecode_carries_immediates_and_executions_repeat_exactly() {
    let fixture = Fixture::new(
        &[u64_type(), TypeExpr::Text],
        &[
            step(
                Opcode::TupleNew,
                vec![Arg::P(0), Arg::P(1)],
                Immediate::None,
                TypeExpr::Tuple(vec![u64_type(), TypeExpr::Text]),
            ),
            step(
                Opcode::TupleGet,
                vec![Arg::R(0)],
                Immediate::Index(0),
                u64_type(),
            ),
        ],
        Vec::new(),
    );
    let lowered = lower_function(fixture.input(CacheProfile::EXTENDED_V1)).unwrap();
    assert_eq!(&lowered.bytes[..8], b"SLEYBC02");
    assert_eq!(
        lowered.bytecode.blocks[0].instructions[1].immediate,
        Immediate::Index(0)
    );
    let restricted_key = lower_function(
        Fixture::new(
            &[TypeExpr::Bool, TypeExpr::Bool],
            &[step(
                Opcode::BoolAnd,
                vec![Arg::P(0), Arg::P(1)],
                Immediate::None,
                TypeExpr::Bool,
            )],
            Vec::new(),
        )
        .input(CacheProfile::RESTRICTED_V1),
    )
    .unwrap()
    .cache_key;
    assert_ne!(
        lowered.cache_key, restricted_key,
        "profiles never share a cache key"
    );
    let first = execute_function(
        fixture.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![uint(9), text("nine")],
            limits: limits(),
        },
    )
    .unwrap();
    assert_eq!(first.termination, ExecutionTermination::Success(uint(9)));
    for _ in 0..128 {
        let again = execute_function(
            fixture.input(CacheProfile::EXTENDED_V1),
            ExecutionRequest {
                inputs: vec![uint(9), text("nine")],
                limits: limits(),
            },
        )
        .unwrap();
        assert_eq!(again, first);
    }
    let changed = execute_function(
        fixture.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![uint(10), text("nine")],
            limits: limits(),
        },
    )
    .unwrap();
    assert_ne!(changed.observation_id, first.observation_id);
    // A wrong input type is still a pre-execution failure.
    let mismatch = execute_function(
        fixture.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![text("nine"), uint(9)],
            limits: limits(),
        },
    )
    .unwrap_err();
    assert!(matches!(mismatch, ExecutionError::Exec(_)));
}

fn int_type(signed: bool, bits: u16) -> TypeExpr {
    if signed {
        TypeExpr::SInt(IntegerWidth::from_bits(bits))
    } else {
        TypeExpr::UInt(IntegerWidth::from_bits(bits))
    }
}

fn int_value(signed: bool, bits: u16, value: i128) -> ConstValue {
    ConstValue {
        value_type: int_type(signed, bits),
        data: if signed {
            ConstData::SInt(value)
        } else {
            ConstData::UInt(u128::try_from(value).unwrap())
        },
    }
}

fn arithmetic(value_type: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(value_type),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}

fn checked_fixture(opcode: Opcode, signed: bool, bits: u16) -> Fixture {
    let value = int_type(signed, bits);
    let operand_types: Vec<TypeExpr> = match opcode {
        Opcode::IntNegChecked => vec![value.clone()],
        Opcode::IntShlChecked | Opcode::IntShrChecked => vec![value.clone(), int_type(false, 32)],
        _ => vec![value.clone(), value.clone()],
    };
    let operands: Vec<Arg> = (0..operand_types.len()).map(Arg::P).collect();
    Fixture::new(
        &operand_types,
        &[step(opcode, operands, Immediate::None, arithmetic(value))],
        Vec::new(),
    )
}

/// Runs one checked operation and returns `Ok(value)` or `Err(code)`.
fn checked(opcode: Opcode, signed: bool, bits: u16, inputs: Vec<ConstValue>) -> Result<i128, u16> {
    let fixture = checked_fixture(opcode, signed, bits);
    match success(&fixture, inputs).data {
        ConstData::Result(ResultConst::Ok(value)) => match value.data {
            ConstData::SInt(value) => Ok(value),
            ConstData::UInt(value) => Ok(i128::try_from(value).unwrap()),
            other => panic!("unexpected ok data {other:?}"),
        },
        ConstData::Result(ResultConst::Err(failure)) => match failure.data {
            ConstData::BuiltinFailure(BuiltinFailureValue {
                kind: BuiltinFailureKind::Arithmetic,
                code,
            }) => Err(code),
            other => panic!("unexpected failure data {other:?}"),
        },
        other => panic!("unexpected result {other:?}"),
    }
}

/// A shift amount, which is always `UInt(32)` whatever the operand width is.
fn amount_of(value: u32) -> ConstValue {
    int_value(false, 32, i128::from(value))
}

/// The unsigned form of `checked`: a `UInt(128)` result can exceed `i128`.
fn checked_unsigned(opcode: Opcode, bits: u16, inputs: Vec<ConstValue>) -> Result<u128, u16> {
    let fixture = checked_fixture(opcode, false, bits);
    match success(&fixture, inputs).data {
        ConstData::Result(ResultConst::Ok(value)) => match value.data {
            ConstData::UInt(value) => Ok(value),
            other => panic!("unexpected ok data {other:?}"),
        },
        ConstData::Result(ResultConst::Err(failure)) => match failure.data {
            ConstData::BuiltinFailure(BuiltinFailureValue {
                kind: BuiltinFailureKind::Arithmetic,
                code,
            }) => Err(code),
            other => panic!("unexpected failure data {other:?}"),
        },
        other => panic!("unexpected result {other:?}"),
    }
}

/// The width's signed minimum, derived without negating it.
fn signed_min_of(bits: u16) -> i128 {
    if bits >= 128 {
        i128::MIN
    } else {
        -(1_i128 << (bits - 1))
    }
}

/// Every width's left-shift boundary, in both signs.
///
/// Ariadne's S20-260 contract review, P0-2: the widths under test were 8 and
/// 32, and the implementation multiplied by `1 << amount`, which is itself out
/// of range at the top of the widest width. The two directions were inverted
/// there. One case per supported width pins the rule where it is hardest.
#[test]
fn e2_left_shift_is_exact_at_every_width_boundary() {
    for bits in [8_u16, 16, 32, 64, 128] {
        let top = u64::from(bits) - 1;
        let shift = u32::try_from(top).unwrap();
        let signed_min = signed_min_of(bits);
        // The only signed value that survives a shift to the sign bit is -1,
        // and it lands exactly on the width's minimum.
        assert_eq!(
            checked(
                Opcode::IntShlChecked,
                true,
                bits,
                vec![int_value(true, bits, -1), amount_of(shift)],
            ),
            Ok(signed_min),
            "signed -1 shl {top} at width {bits}"
        );
        // Its positive mirror cannot: the result would need the sign bit.
        assert_eq!(
            checked(
                Opcode::IntShlChecked,
                true,
                bits,
                vec![int_value(true, bits, 1), amount_of(shift)],
            ),
            Err(1),
            "signed 1 shl {top} at width {bits}"
        );
        // One below the top stays representable in both signs.
        if top >= 1 {
            let below = u32::try_from(top - 1).unwrap();
            assert_eq!(
                checked(
                    Opcode::IntShlChecked,
                    true,
                    bits,
                    vec![int_value(true, bits, 1), amount_of(below)],
                ),
                Ok(1_i128 << (top - 1)),
                "signed 1 shl {} at width {bits}",
                top - 1
            );
        }
        // Unsigned: the high bit is reachable, and one more is not. At width
        // 128 the reachable value exceeds `i128`, which is why this arm reads
        // the unsigned data directly.
        assert_eq!(
            checked_unsigned(
                Opcode::IntShlChecked,
                bits,
                vec![int_value(false, bits, 1), amount_of(shift)],
            ),
            Ok(1_u128 << top),
            "unsigned 1 shl {top} at width {bits}"
        );
        assert_eq!(
            checked_unsigned(
                Opcode::IntShlChecked,
                bits,
                vec![int_value(false, bits, 2), amount_of(shift)],
            ),
            Err(1),
            "unsigned 2 shl {top} at width {bits}"
        );
        // A shift at the width is an invalid shift, not an overflow.
        assert_eq!(
            checked(
                Opcode::IntShlChecked,
                true,
                bits,
                vec![int_value(true, bits, 1), amount_of(u32::from(bits))],
            ),
            Err(3),
            "signed shl at width {bits}"
        );
    }
}

#[test]
fn e2_checked_integers_overflow_divide_and_shift_exactly() {
    let u8v = |value: i128| int_value(false, 8, value);
    let i8v = |value: i128| int_value(true, 8, value);
    let amount = |value: i128| int_value(false, 32, value);
    assert_eq!(
        checked(Opcode::IntAddChecked, false, 8, vec![u8v(200), u8v(55)]),
        Ok(255)
    );
    assert_eq!(
        checked(Opcode::IntAddChecked, false, 8, vec![u8v(200), u8v(56)]),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntSubChecked, false, 8, vec![u8v(0), u8v(1)]),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntSubChecked, true, 8, vec![i8v(-100), i8v(28)]),
        Ok(-128)
    );
    assert_eq!(
        checked(Opcode::IntSubChecked, true, 8, vec![i8v(-100), i8v(29)]),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntMulChecked, true, 8, vec![i8v(-8), i8v(16)]),
        Ok(-128)
    );
    assert_eq!(
        checked(Opcode::IntMulChecked, true, 8, vec![i8v(8), i8v(16)]),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntDivChecked, true, 8, vec![i8v(-7), i8v(2)]),
        Ok(-3)
    );
    assert_eq!(
        checked(Opcode::IntRemChecked, true, 8, vec![i8v(-7), i8v(2)]),
        Ok(-1)
    );
    assert_eq!(
        checked(Opcode::IntDivChecked, false, 8, vec![u8v(7), u8v(0)]),
        Err(2)
    );
    assert_eq!(
        checked(Opcode::IntRemChecked, true, 8, vec![i8v(7), i8v(0)]),
        Err(2)
    );
    assert_eq!(
        checked(Opcode::IntDivChecked, true, 8, vec![i8v(-128), i8v(-1)]),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntRemChecked, true, 8, vec![i8v(-128), i8v(-1)]),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntNegChecked, true, 8, vec![i8v(-128)]),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntNegChecked, true, 8, vec![i8v(127)]),
        Ok(-127)
    );
    assert_eq!(
        checked(Opcode::IntShlChecked, false, 8, vec![u8v(3), amount(2)]),
        Ok(12)
    );
    assert_eq!(
        checked(Opcode::IntShlChecked, false, 8, vec![u8v(0x80), amount(1)]),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntShlChecked, false, 8, vec![u8v(1), amount(8)]),
        Err(3)
    );
    assert_eq!(
        checked(Opcode::IntShlChecked, true, 8, vec![i8v(-1), amount(7)]),
        Ok(-128)
    );
    assert_eq!(
        checked(Opcode::IntShlChecked, true, 8, vec![i8v(1), amount(7)]),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntShrChecked, true, 8, vec![i8v(-8), amount(1)]),
        Ok(-4)
    );
    assert_eq!(
        checked(Opcode::IntShrChecked, false, 8, vec![u8v(0x80), amount(7)]),
        Ok(1)
    );
    assert_eq!(
        checked(Opcode::IntShrChecked, false, 8, vec![u8v(1), amount(9)]),
        Err(3)
    );
    // Width 128 uses the native bounds.
    let big = |value: i128| int_value(true, 128, value);
    assert_eq!(
        checked(
            Opcode::IntAddChecked,
            true,
            128,
            vec![big(i128::MAX), big(1)]
        ),
        Err(1)
    );
    assert_eq!(
        checked(Opcode::IntMulChecked, true, 128, vec![big(1 << 62), big(4)]),
        Ok(1 << 64)
    );
    let ubig = |value: u128| ConstValue {
        value_type: int_type(false, 128),
        data: ConstData::UInt(value),
    };
    let fixture = checked_fixture(Opcode::IntAddChecked, false, 128);
    let ConstData::Result(ResultConst::Err(_)) =
        success(&fixture, vec![ubig(u128::MAX), ubig(1)]).data
    else {
        panic!("u128 overflow expected");
    };
    // Rejections: width mismatch, negation of an unsigned value, a signed
    // shift amount, and a declared result that is not the arithmetic Result.
    let mismatch = Fixture::new(
        &[int_type(true, 32), int_type(true, 64)],
        &[step(
            Opcode::IntAddChecked,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            arithmetic(int_type(true, 32)),
        )],
        Vec::new(),
    );
    assert_eq!(lowering_code(&mismatch), LowerErrorCode::SignatureMismatch);
    let unsigned_neg = Fixture::new(
        &[int_type(false, 16)],
        &[step(
            Opcode::IntNegChecked,
            vec![Arg::P(0)],
            Immediate::None,
            arithmetic(int_type(false, 16)),
        )],
        Vec::new(),
    );
    assert_eq!(
        lowering_code(&unsigned_neg),
        LowerErrorCode::SignatureMismatch
    );
    let signed_amount = Fixture::new(
        &[int_type(false, 16), int_type(true, 32)],
        &[step(
            Opcode::IntShlChecked,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            arithmetic(int_type(false, 16)),
        )],
        Vec::new(),
    );
    assert_eq!(
        lowering_code(&signed_amount),
        LowerErrorCode::SignatureMismatch
    );
    let bare = Fixture::new(
        &[int_type(false, 16), int_type(false, 16)],
        &[step(
            Opcode::IntAddChecked,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            int_type(false, 16),
        )],
        Vec::new(),
    );
    assert_eq!(lowering_code(&bare), LowerErrorCode::SignatureMismatch);
}

fn f64v(value: f64) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::F64,
        data: ConstData::F64Bits(value.to_bits()),
    }
}

fn f32v(value: f32) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::F32,
        data: ConstData::F32Bits(value.to_bits()),
    }
}

fn float_fixture(opcode: Opcode, value_type: TypeExpr, arity: usize) -> Fixture {
    let operand_types = vec![value_type.clone(); arity];
    let operands: Vec<Arg> = (0..arity).map(Arg::P).collect();
    let result = if matches!(
        opcode,
        Opcode::Equal
            | Opcode::NotEqual
            | Opcode::LessThan
            | Opcode::LessEqual
            | Opcode::GreaterThan
            | Opcode::GreaterEqual
    ) {
        TypeExpr::Bool
    } else {
        value_type
    };
    Fixture::new(
        &operand_types,
        &[step(opcode, operands, Immediate::None, result)],
        Vec::new(),
    )
}

fn f64_bits(fixture: &Fixture, inputs: Vec<ConstValue>) -> u64 {
    match success(fixture, inputs).data {
        ConstData::F64Bits(bits) => bits,
        other => panic!("not f64: {other:?}"),
    }
}

#[test]
fn e3_floats_round_to_nearest_canonicalize_nan_and_compare_by_ieee() {
    let add = float_fixture(Opcode::FloatAdd, TypeExpr::F64, 2);
    assert_eq!(
        f64_bits(&add, vec![f64v(0.1), f64v(0.2)]),
        (0.1_f64 + 0.2).to_bits()
    );
    let div = float_fixture(Opcode::FloatDiv, TypeExpr::F64, 2);
    assert_eq!(
        f64_bits(&div, vec![f64v(1.0), f64v(0.0)]),
        f64::INFINITY.to_bits()
    );
    assert_eq!(
        f64_bits(&div, vec![f64v(0.0), f64v(0.0)]),
        0x7ff8_0000_0000_0000
    );
    let sub = float_fixture(Opcode::FloatSub, TypeExpr::F64, 2);
    assert_eq!(
        f64_bits(&sub, vec![f64v(f64::INFINITY), f64v(f64::INFINITY)]),
        0x7ff8_0000_0000_0000,
        "every NaN result is the canonical quiet NaN"
    );
    // A NaN input with a payload is not canonical and never reaches execution.
    let payload_nan = ConstValue {
        value_type: TypeExpr::F64,
        data: ConstData::F64Bits(0x7ff8_dead_beef_0001),
    };
    let refused = execute_function(
        add.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![payload_nan, f64v(1.0)],
            limits: limits(),
        },
    )
    .unwrap_err();
    assert!(matches!(refused, ExecutionError::Type(_)), "{refused:?}");
    // Negative zero is not a canonical constant, so a negative-zero result
    // is canonicalized to positive zero.
    let neg = float_fixture(Opcode::FloatNeg, TypeExpr::F64, 1);
    assert_eq!(f64_bits(&neg, vec![f64v(0.0)]), 0.0_f64.to_bits());
    assert_eq!(f64_bits(&neg, vec![f64v(2.5)]), (-2.5_f64).to_bits());
    let fma = float_fixture(Opcode::FloatFma, TypeExpr::F64, 3);
    let fused = f64_bits(&fma, vec![f64v(0.1), f64v(10.0), f64v(-1.0)]);
    assert_eq!(fused, 0.1_f64.mul_add(10.0, -1.0).to_bits());
    assert_ne!(
        fused,
        ((0.1_f64 * 10.0) - 1.0).to_bits(),
        "a single rounding differs from two"
    );
    // F32 preserves subnormals.
    let mul32 = float_fixture(Opcode::FloatMul, TypeExpr::F32, 2);
    let smallest = f32::from_bits(1);
    match success(&mul32, vec![f32v(smallest), f32v(1.0)]).data {
        ConstData::F32Bits(bits) => assert_eq!(bits, 1),
        other => panic!("not f32: {other:?}"),
    }
    // IEEE comparisons: NaN is unordered, zeros are equal.
    let compare = |opcode: Opcode, left: f64, right: f64| -> bool {
        match success(
            &float_fixture(opcode, TypeExpr::F64, 2),
            vec![f64v(left), f64v(right)],
        )
        .data
        {
            ConstData::Bool(value) => value,
            other => panic!("not bool: {other:?}"),
        }
    };
    assert!(!compare(Opcode::Equal, f64::NAN, f64::NAN));
    assert!(compare(Opcode::NotEqual, f64::NAN, f64::NAN));
    assert!(!compare(Opcode::LessThan, f64::NAN, 1.0));
    assert!(!compare(Opcode::GreaterEqual, 1.0, f64::NAN));
    assert!(compare(Opcode::Equal, 0.0, 0.0));
    let negative_zero = ConstValue {
        value_type: TypeExpr::F64,
        data: ConstData::F64Bits((-0.0_f64).to_bits()),
    };
    let refused_zero = execute_function(
        add.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![negative_zero, f64v(1.0)],
            limits: limits(),
        },
    )
    .unwrap_err();
    assert!(matches!(refused_zero, ExecutionError::Type(_)));
    assert!(compare(Opcode::LessThan, f64::NEG_INFINITY, -1.0e308));
    assert!(compare(Opcode::LessEqual, 2.5, 2.5));
    assert!(compare(Opcode::GreaterThan, f64::INFINITY, f64::MAX));
    // Repeat determinism over a NaN-producing path.
    let first = execute_function(
        div.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![f64v(0.0), f64v(0.0)],
            limits: limits(),
        },
    )
    .unwrap();
    for _ in 0..128 {
        let again = execute_function(
            div.input(CacheProfile::EXTENDED_V1),
            ExecutionRequest {
                inputs: vec![f64v(0.0), f64v(0.0)],
                limits: limits(),
            },
        )
        .unwrap();
        assert_eq!(again, first);
    }
    // Rejections: mixed widths, a two-operand fma, and nested floats in equality.
    let mixed = Fixture::new(
        &[TypeExpr::F32, TypeExpr::F64],
        &[step(
            Opcode::FloatAdd,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            TypeExpr::F64,
        )],
        Vec::new(),
    );
    assert_eq!(lowering_code(&mixed), LowerErrorCode::SignatureMismatch);
    let short_fma = Fixture::new(
        &[TypeExpr::F64, TypeExpr::F64],
        &[step(
            Opcode::FloatFma,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            TypeExpr::F64,
        )],
        Vec::new(),
    );
    assert_eq!(lowering_code(&short_fma), LowerErrorCode::SignatureMismatch);
    let nested = Fixture::new(
        &[
            TypeExpr::Tuple(vec![TypeExpr::F64]),
            TypeExpr::Tuple(vec![TypeExpr::F64]),
        ],
        &[step(
            Opcode::Equal,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            TypeExpr::Bool,
        )],
        Vec::new(),
    );
    assert_eq!(lowering_code(&nested), LowerErrorCode::SignatureMismatch);
    let int_operand = Fixture::new(
        &[int_type(false, 64), int_type(false, 64)],
        &[step(
            Opcode::FloatMul,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            int_type(false, 64),
        )],
        Vec::new(),
    );
    assert_eq!(
        lowering_code(&int_operand),
        LowerErrorCode::SignatureMismatch
    );
}

fn member(byte: u8) -> MemberId {
    MemberId::from_bytes([byte; 32])
}

/// `Pair { a: UInt64, b: Text }` at entity 50 and `Shape { Circle(UInt64), Empty }` at 51.
fn definitions() -> Vec<TypeDefinition> {
    vec![
        TypeDefinition {
            entity_id: id(50),
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(vec![
                RecordField {
                    member_id: member(0xA1),
                    value_type: u64_type(),
                    visibility: Visibility::Private,
                },
                RecordField {
                    member_id: member(0xB2),
                    value_type: TypeExpr::Text,
                    visibility: Visibility::Private,
                },
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
        TypeDefinition {
            entity_id: id(51),
            type_parameters: Vec::new(),
            form: TypeDefForm::Variant(vec![
                VariantCase {
                    member_id: member(0xC1),
                    payload_type: Some(u64_type()),
                },
                VariantCase {
                    member_id: member(0xC2),
                    payload_type: None,
                },
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
    ]
}

fn pair_type() -> TypeExpr {
    TypeExpr::Named(NamedType {
        definition: id(50),
        arguments: Vec::new(),
    })
}

fn shape_type() -> TypeExpr {
    TypeExpr::Named(NamedType {
        definition: id(51),
        arguments: Vec::new(),
    })
}

fn circle(member_byte: u8) -> Immediate {
    Immediate::Variant(VariantImmediate {
        definition: id(51),
        member_id: member(member_byte),
    })
}

fn map_type() -> TypeExpr {
    TypeExpr::OrderedMap {
        key: Box::new(u64_type()),
        value: Box::new(TypeExpr::Text),
    }
}

fn map_new_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(map_type()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    }
}

fn map_of(entries: Vec<(u128, &str)>) -> ConstValue {
    ConstValue {
        value_type: map_type(),
        data: ConstData::Map(
            entries
                .into_iter()
                .map(|(key, value)| MapEntryConst {
                    key: uint(key),
                    value: text(value),
                })
                .collect(),
        ),
    }
}

#[test]
fn e4_records_variants_and_maps_construct_project_and_keep_canonical_order() {
    let record = Fixture::with_types(
        &[u64_type(), TypeExpr::Text],
        &[
            step(
                Opcode::RecordNew,
                vec![Arg::P(0), Arg::P(1)],
                Immediate::Entity(id(50)),
                pair_type(),
            ),
            step(
                Opcode::RecordGet,
                vec![Arg::R(0)],
                Immediate::Field(member(0xB2)),
                TypeExpr::Text,
            ),
        ],
        Vec::new(),
        definitions(),
    );
    assert_eq!(success(&record, vec![uint(4), text("four")]), text("four"));
    let built = Fixture::with_types(
        &[u64_type(), TypeExpr::Text],
        &[step(
            Opcode::RecordNew,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::Entity(id(50)),
            pair_type(),
        )],
        Vec::new(),
        definitions(),
    );
    let value = success(&built, vec![uint(4), text("four")]);
    assert_eq!(
        value.data,
        ConstData::Record(RecordConst {
            definition: id(50),
            fields: vec![
                FieldConst {
                    member_id: member(0xA1),
                    value: uint(4)
                },
                FieldConst {
                    member_id: member(0xB2),
                    value: text("four")
                },
            ],
        })
    );
    built
        .types
        .check_constant(&value)
        .expect("a constructed record is a canonical constant");

    let variant = Fixture::with_types(
        &[u64_type()],
        &[
            step(
                Opcode::VariantNew,
                vec![Arg::P(0)],
                circle(0xC1),
                shape_type(),
            ),
            step(
                Opcode::VariantGet,
                vec![Arg::R(0)],
                circle(0xC1),
                TypeExpr::Option(Box::new(u64_type())),
            ),
        ],
        Vec::new(),
        definitions(),
    );
    assert_eq!(
        success(&variant, vec![uint(9)]).data,
        ConstData::Option(Some(Box::new(uint(9))))
    );
    let empty = Fixture::with_types(
        &[TypeExpr::Bool],
        &[
            step(Opcode::VariantNew, vec![], circle(0xC2), shape_type()),
            step(
                Opcode::VariantGet,
                vec![Arg::R(0)],
                circle(0xC1),
                TypeExpr::Option(Box::new(u64_type())),
            ),
        ],
        Vec::new(),
        definitions(),
    );
    assert_eq!(
        success(&empty, vec![boolean(true)]).data,
        ConstData::Option(None)
    );
    let empty_value = Fixture::with_types(
        &[TypeExpr::Bool],
        &[step(Opcode::VariantNew, vec![], circle(0xC2), shape_type())],
        Vec::new(),
        definitions(),
    );
    assert_eq!(
        success(&empty_value, vec![boolean(true)]).data,
        ConstData::Variant(VariantConst {
            definition: id(51),
            member_id: member(0xC2),
            payload: None
        })
    );

    // Maps: construction sorts by canonical key bytes, duplicates fail as values.
    let map_new = Fixture::new(
        &[u64_type(), TypeExpr::Text, u64_type(), TypeExpr::Text],
        &[step(
            Opcode::MapNew,
            vec![Arg::P(0), Arg::P(1), Arg::P(2), Arg::P(3)],
            Immediate::None,
            map_new_type(),
        )],
        Vec::new(),
    );
    let ordered = success(
        &map_new,
        vec![uint(300), text("big"), uint(7), text("small")],
    );
    let expected = {
        let mut entries = vec![(300_u128, "big"), (7, "small")];
        entries.sort_by_key(|(key, _)| sley_mutate::encode_const_value(&uint(*key)).unwrap());
        map_of(entries)
    };
    assert_eq!(
        ordered.data,
        ConstData::Result(ResultConst::Ok(Box::new(expected.clone())))
    );
    let duplicate = success(&map_new, vec![uint(7), text("a"), uint(7), text("b")]);
    let ConstData::Result(ResultConst::Err(failure)) = duplicate.data else {
        panic!("duplicate key must fail as a value");
    };
    assert_eq!(
        failure.data,
        ConstData::BuiltinFailure(BuiltinFailureValue {
            kind: BuiltinFailureKind::DuplicateKey,
            code: 1
        })
    );
    let lookup = Fixture::new(
        &[map_type(), u64_type()],
        &[step(
            Opcode::MapGet,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            TypeExpr::Option(Box::new(TypeExpr::Text)),
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&lookup, vec![expected.clone(), uint(7)]).data,
        ConstData::Option(Some(Box::new(text("small"))))
    );
    assert_eq!(
        success(&lookup, vec![expected.clone(), uint(8)]).data,
        ConstData::Option(None)
    );
    let contains = Fixture::new(
        &[map_type(), u64_type()],
        &[step(
            Opcode::MapContains,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            TypeExpr::Bool,
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&contains, vec![expected.clone(), uint(300)]),
        boolean(true)
    );
    let insert = Fixture::new(
        &[map_type(), u64_type(), TypeExpr::Text],
        &[step(
            Opcode::MapInsert,
            vec![Arg::P(0), Arg::P(1), Arg::P(2)],
            Immediate::None,
            map_type(),
        )],
        Vec::new(),
    );
    let replaced = success(&insert, vec![expected.clone(), uint(7), text("replaced")]);
    let ConstData::Map(entries) = &replaced.data else {
        panic!("map expected");
    };
    assert_eq!(entries.len(), 2);
    assert!(
        entries
            .iter()
            .any(|entry| entry.key == uint(7) && entry.value == text("replaced"))
    );
    let grown = success(&insert, vec![expected.clone(), uint(1), text("new")]);
    let ConstData::Map(entries) = &grown.data else {
        panic!("map expected");
    };
    let keys: Vec<Vec<u8>> = entries
        .iter()
        .map(|entry| sley_mutate::encode_const_value(&entry.key).unwrap())
        .collect();
    assert!(
        keys.windows(2).all(|pair| pair[0] < pair[1]),
        "entries stay sorted by canonical key bytes"
    );
    assert_eq!(entries.len(), 3);
    let remove = Fixture::new(
        &[map_type(), u64_type()],
        &[step(
            Opcode::MapRemove,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            map_type(),
        )],
        Vec::new(),
    );
    let ConstData::Map(entries) = success(&remove, vec![expected, uint(300)]).data else {
        panic!("map expected");
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, uint(7));
    // The empty map takes its type from the declared result.
    let empty_map = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::MapNew,
            vec![],
            Immediate::None,
            map_new_type(),
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&empty_map, vec![boolean(true)]).data,
        ConstData::Result(ResultConst::Ok(Box::new(map_of(Vec::new()))))
    );

    // Rejections.
    let short_record = Fixture::with_types(
        &[u64_type()],
        &[step(
            Opcode::RecordNew,
            vec![Arg::P(0)],
            Immediate::Entity(id(50)),
            pair_type(),
        )],
        Vec::new(),
        definitions(),
    );
    assert_eq!(
        lowering_code(&short_record),
        LowerErrorCode::SignatureMismatch
    );
    let unknown_field = Fixture::with_types(
        &[u64_type(), TypeExpr::Text],
        &[
            step(
                Opcode::RecordNew,
                vec![Arg::P(0), Arg::P(1)],
                Immediate::Entity(id(50)),
                pair_type(),
            ),
            step(
                Opcode::RecordGet,
                vec![Arg::R(0)],
                Immediate::Field(member(0xEE)),
                TypeExpr::Text,
            ),
        ],
        Vec::new(),
        definitions(),
    );
    assert_eq!(
        lowering_code(&unknown_field),
        LowerErrorCode::ImmediateMismatch
    );
    let record_as_variant = Fixture::with_types(
        &[u64_type()],
        &[step(
            Opcode::VariantNew,
            vec![Arg::P(0)],
            Immediate::Variant(VariantImmediate {
                definition: id(50),
                member_id: member(0xA1),
            }),
            pair_type(),
        )],
        Vec::new(),
        definitions(),
    );
    assert_eq!(
        lowering_code(&record_as_variant),
        LowerErrorCode::ImmediateMismatch
    );
    let payload_missing = Fixture::with_types(
        &[TypeExpr::Bool],
        &[step(Opcode::VariantNew, vec![], circle(0xC1), shape_type())],
        Vec::new(),
        definitions(),
    );
    assert_eq!(
        lowering_code(&payload_missing),
        LowerErrorCode::SignatureMismatch
    );
    let payload_less_get = Fixture::with_types(
        &[u64_type()],
        &[
            step(
                Opcode::VariantNew,
                vec![Arg::P(0)],
                circle(0xC1),
                shape_type(),
            ),
            step(
                Opcode::VariantGet,
                vec![Arg::R(0)],
                circle(0xC2),
                TypeExpr::Option(Box::new(TypeExpr::Unit)),
            ),
        ],
        Vec::new(),
        definitions(),
    );
    assert_eq!(
        lowering_code(&payload_less_get),
        LowerErrorCode::ImmediateMismatch
    );
    // A float map key never reaches lowering: S20-220 refuses the map type
    // itself with TYPE_NOT_ORDERABLE, so the float guard is defense in depth.
    let odd = Fixture::new(
        &[u64_type(), TypeExpr::Text, u64_type()],
        &[step(
            Opcode::MapNew,
            vec![Arg::P(0), Arg::P(1), Arg::P(2)],
            Immediate::None,
            map_new_type(),
        )],
        Vec::new(),
    );
    assert_eq!(lowering_code(&odd), LowerErrorCode::SignatureMismatch);
}

fn global_seventy_seven() -> (Vec<GlobalValueDefinition>, Vec<ConstantDefinition>) {
    (
        vec![GlobalValueDefinition {
            entity_id: id(60),
            value_type: u64_type(),
            initializer: id(61),
            visibility: Visibility::Private,
        }],
        vec![ConstantDefinition {
            entity_id: id(61),
            value: uint(77),
        }],
    )
}

fn callee_seventy() -> FunctionGraph {
    FunctionGraph {
        entity_id: id(70),
        type_parameters: Vec::new(),
        parameters: Vec::new(),
        result_type: u64_type(),
        effects: Vec::new(),
        entry_block: id(71),
        blocks: vec![id(71)],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn reference_seventy() -> Immediate {
    Immediate::Function(FunctionRefValue {
        function: id(70),
        type_arguments: Vec::new(),
    })
}

fn seventy_type() -> TypeExpr {
    TypeExpr::FunctionRef(FunctionType {
        parameters: Vec::new(),
        result: Box::new(u64_type()),
        effects: Vec::new(),
    })
}

fn cell_type() -> TypeExpr {
    TypeExpr::LocalCell(Box::new(u64_type()))
}

#[test]
fn e5_cells_hashes_globals_and_references_follow_the_contract() {
    let cells = Fixture::new(
        &[u64_type(), u64_type()],
        &[
            step(
                Opcode::CellNew,
                vec![Arg::P(0)],
                Immediate::None,
                cell_type(),
            ),
            step(
                Opcode::CellSet,
                vec![Arg::R(0), Arg::P(1)],
                Immediate::None,
                TypeExpr::Unit,
            ),
            step(
                Opcode::CellGet,
                vec![Arg::R(0)],
                Immediate::None,
                u64_type(),
            ),
        ],
        Vec::new(),
    );
    assert_eq!(success(&cells, vec![uint(1), uint(2)]), uint(2));
    let fresh = Fixture::new(
        &[u64_type(), u64_type()],
        &[
            step(
                Opcode::CellNew,
                vec![Arg::P(0)],
                Immediate::None,
                cell_type(),
            ),
            step(
                Opcode::CellNew,
                vec![Arg::P(1)],
                Immediate::None,
                cell_type(),
            ),
            step(
                Opcode::CellGet,
                vec![Arg::R(1)],
                Immediate::None,
                u64_type(),
            ),
            step(
                Opcode::CellGet,
                vec![Arg::R(0)],
                Immediate::None,
                u64_type(),
            ),
        ],
        Vec::new(),
    );
    assert_eq!(success(&fresh, vec![uint(5), uint(6)]), uint(5));

    let hash = Fixture::new(
        &[TypeExpr::Text],
        &[step(
            Opcode::ValueHash,
            vec![Arg::P(0)],
            Immediate::None,
            TypeExpr::Bytes,
        )],
        Vec::new(),
    );
    let expected =
        hash_validated_value(SchemaEpochId::from_bytes([8; 32]), &text("hash me")).unwrap();
    assert_eq!(
        success(&hash, vec![text("hash me")]).data,
        ConstData::Bytes(expected.as_bytes().to_vec())
    );

    let (globals, constants) = global_seventy_seven();
    let global = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::GlobalGet,
            vec![],
            Immediate::Entity(id(60)),
            u64_type(),
        )],
        Vec::new(),
    )
    .with_globals(globals, constants);
    assert_eq!(success(&global, vec![boolean(true)]), uint(77));

    let reference = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::FunctionRef,
            vec![],
            reference_seventy(),
            seventy_type(),
        )],
        Vec::new(),
    )
    .with_functions(vec![callee_seventy()]);
    let value = success(&reference, vec![boolean(true)]);
    assert_eq!(value.value_type, seventy_type());
    assert_eq!(
        value.data,
        ConstData::FunctionRef(FunctionRefValue {
            function: id(70),
            type_arguments: Vec::new(),
        })
    );

    // Rejections.
    let escaping_cell = Fixture::new(
        &[u64_type()],
        &[step(
            Opcode::CellNew,
            vec![Arg::P(0)],
            Immediate::None,
            cell_type(),
        )],
        Vec::new(),
    );
    assert_eq!(
        lowering_code(&escaping_cell),
        LowerErrorCode::SignatureMismatch
    );
    let cell_in_tuple = Fixture::new(
        &[u64_type()],
        &[
            step(
                Opcode::CellNew,
                vec![Arg::P(0)],
                Immediate::None,
                cell_type(),
            ),
            step(
                Opcode::TupleNew,
                vec![Arg::R(0), Arg::P(0)],
                Immediate::None,
                TypeExpr::Tuple(vec![cell_type(), u64_type()]),
            ),
            step(
                Opcode::TupleGet,
                vec![Arg::R(1)],
                Immediate::Index(1),
                u64_type(),
            ),
        ],
        Vec::new(),
    );
    assert_eq!(
        lowering_code(&cell_in_tuple),
        LowerErrorCode::SignatureMismatch
    );
    let cell_of_cell = Fixture::new(
        &[u64_type()],
        &[
            step(
                Opcode::CellNew,
                vec![Arg::P(0)],
                Immediate::None,
                cell_type(),
            ),
            step(
                Opcode::CellNew,
                vec![Arg::R(0)],
                Immediate::None,
                TypeExpr::LocalCell(Box::new(cell_type())),
            ),
            step(
                Opcode::CellGet,
                vec![Arg::R(0)],
                Immediate::None,
                u64_type(),
            ),
        ],
        Vec::new(),
    );
    assert_eq!(
        lowering_code(&cell_of_cell),
        LowerErrorCode::SignatureMismatch
    );
    let set_wrong_type = Fixture::new(
        &[u64_type(), TypeExpr::Text],
        &[
            step(
                Opcode::CellNew,
                vec![Arg::P(0)],
                Immediate::None,
                cell_type(),
            ),
            step(
                Opcode::CellSet,
                vec![Arg::R(0), Arg::P(1)],
                Immediate::None,
                TypeExpr::Unit,
            ),
        ],
        Vec::new(),
    );
    assert_eq!(
        lowering_code(&set_wrong_type),
        LowerErrorCode::SignatureMismatch
    );
    let unknown_global = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::GlobalGet,
            vec![],
            Immediate::Entity(id(66)),
            u64_type(),
        )],
        Vec::new(),
    );
    assert_eq!(
        lowering_code(&unknown_global),
        LowerErrorCode::ImmediateMismatch
    );
    let (globals, _) = global_seventy_seven();
    let mismatched_global = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::GlobalGet,
            vec![],
            Immediate::Entity(id(60)),
            u64_type(),
        )],
        Vec::new(),
    )
    .with_globals(
        globals,
        vec![ConstantDefinition {
            entity_id: id(61),
            value: text("not a u64"),
        }],
    );
    assert_eq!(
        lowering_code(&mismatched_global),
        LowerErrorCode::ImmediateMismatch
    );
    let unknown_function = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::FunctionRef,
            vec![],
            reference_seventy(),
            seventy_type(),
        )],
        Vec::new(),
    );
    assert_eq!(
        lowering_code(&unknown_function),
        LowerErrorCode::ImmediateMismatch
    );
    let generic_reference = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::FunctionRef,
            vec![],
            Immediate::Function(FunctionRefValue {
                function: id(70),
                type_arguments: vec![u64_type()],
            }),
            seventy_type(),
        )],
        Vec::new(),
    )
    .with_functions(vec![callee_seventy()]);
    assert_eq!(
        lowering_code(&generic_reference),
        LowerErrorCode::ImmediateMismatch
    );
}

/// Function 61: `(a) -> a and a`, a `Bool` contract predicate.
fn bool_predicate() -> Fixture {
    Fixture::with_base(
        60,
        &[TypeExpr::Bool],
        &[step(
            Opcode::BoolAnd,
            vec![Arg::P(0), Arg::P(0)],
            Immediate::None,
            TypeExpr::Bool,
        )],
        Vec::new(),
        Vec::new(),
    )
}

/// Function 61 again, answering a tuple instead of `Bool`.
fn tuple_predicate() -> Fixture {
    Fixture::with_base(
        60,
        &[TypeExpr::Bool],
        &[step(
            Opcode::TupleNew,
            vec![Arg::P(0)],
            Immediate::None,
            TypeExpr::Tuple(vec![TypeExpr::Bool]),
        )],
        Vec::new(),
        Vec::new(),
    )
}

/// The exact slice E7a result type.
fn assertion_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Unit),
        error: Box::new(TypeExpr::BuiltinFailure(
            BuiltinFailureKind::ContractViolation,
        )),
    }
}

fn assertion_value(held: bool) -> ConstValue {
    ConstValue {
        value_type: assertion_type(),
        data: ConstData::Result(if held {
            ResultConst::Ok(Box::new(ConstValue {
                value_type: TypeExpr::Unit,
                data: ConstData::Unit,
            }))
        } else {
            ResultConst::Err(Box::new(ConstValue {
                value_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::ContractViolation),
                data: ConstData::BuiltinFailure(BuiltinFailureValue {
                    kind: BuiltinFailureKind::ContractViolation,
                    code: 1,
                }),
            }))
        }),
    }
}

/// One `Precondition` on the entry function, discharged by function 61.
fn precondition(target: EntityId, predicate: EntityId) -> ContractDefinition {
    ContractDefinition {
        entity_id: id(200),
        target,
        contract_kind: ContractKind::Precondition,
        predicate,
        bindings: vec![ContractBinding {
            predicate_parameter: 0,
            source: ContractSource::Parameter(id(10)),
        }],
        resource_limits: None,
    }
}

/// The entry asserts one contract over its own `Bool` parameter.
fn asserting_entry() -> Fixture {
    let entry = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::ContractAssert,
            vec![Arg::P(0)],
            Immediate::Entity(id(200)),
            assertion_type(),
        )],
        Vec::new(),
    );
    let target = entry.function.entity_id;
    let predicate = bool_predicate();
    let predicate_id = predicate.function.entity_id;
    entry
        .with_callee(predicate)
        .with_contracts(vec![precondition(target, predicate_id)])
}

#[test]
fn e7a_contract_assertions_call_the_predicate_and_carry_its_verdict() {
    let fixture = asserting_entry();
    assert_eq!(
        success(&fixture, vec![boolean(true)]),
        assertion_value(true),
        "a predicate that holds yields Ok(Unit)"
    );
    assert_eq!(
        success(&fixture, vec![boolean(false)]),
        assertion_value(false),
        "a predicate that fails yields the contract violation, not a trap"
    );
    // The predicate frame is a real call charged against the caller's budget:
    // the assertion and the predicate's own operation both count, so a
    // one-instruction ceiling cannot cover the pair.
    let ceiling = |max_fuel: u64| {
        execute_function(
            fixture.input(CacheProfile::EXTENDED_V1),
            ExecutionRequest {
                inputs: vec![boolean(true)],
                limits: ExecutionLimits {
                    max_fuel,
                    ..limits()
                },
            },
        )
        .expect("executes")
        .termination
    };
    // Exactly five fuel: the assertion's own dispatch, the frame, the
    // predicate's operation, and both terminators. Four refuses, and the
    // refusal is a resource limit rather than a violation.
    assert_eq!(
        ceiling(4),
        ExecutionTermination::ResourceLimit(ResourceKind::Fuel)
    );
    assert_eq!(
        ceiling(5),
        ExecutionTermination::Success(assertion_value(true))
    );
    // Deterministic across repetitions.
    for _ in 0..128 {
        assert_eq!(
            success(&fixture, vec![boolean(false)]),
            assertion_value(false)
        );
    }
}

#[test]
fn e7a_rejection_matrix_names_the_frozen_lowering_codes() {
    let entry = || {
        Fixture::new(
            &[TypeExpr::Bool],
            &[step(
                Opcode::ContractAssert,
                vec![Arg::P(0)],
                Immediate::Entity(id(200)),
                assertion_type(),
            )],
            Vec::new(),
        )
    };
    let target = entry().function.entity_id;
    let predicate_id = bool_predicate().function.entity_id;

    let no_contract = entry().with_callee(bool_predicate());
    assert_eq!(
        lowering_code(&no_contract),
        LowerErrorCode::ImmediateMismatch,
        "an immediate naming no contract"
    );

    let mut wrong_target = precondition(target, predicate_id);
    wrong_target.target = id(250);
    assert_eq!(
        lowering_code(
            &entry()
                .with_callee(bool_predicate())
                .with_contracts(vec![wrong_target])
        ),
        LowerErrorCode::ImmediateMismatch,
        "a contract that targets another function"
    );

    let mut unsupported_kind = precondition(target, predicate_id);
    unsupported_kind.contract_kind = ContractKind::Invariant;
    assert_eq!(
        lowering_code(
            &entry()
                .with_callee(bool_predicate())
                .with_contracts(vec![unsupported_kind])
        ),
        LowerErrorCode::ImmediateMismatch,
        "a contract kind epoch 1 does not support"
    );

    let mut ceiling = precondition(target, predicate_id);
    ceiling.resource_limits = Some(ResourceLimits {
        fuel: 1,
        memory_bytes: 1,
        output_bytes: 1,
        effect_count: 0,
        call_depth: 1,
        wall_timeout_millis: 1,
    });
    assert_eq!(
        lowering_code(
            &entry()
                .with_callee(bool_predicate())
                .with_contracts(vec![ceiling])
        ),
        LowerErrorCode::ImmediateMismatch,
        "a contract carrying a resource ceiling"
    );

    // A predicate that answers something other than Bool.
    let tuple_predicate_id = tuple_predicate().function.entity_id;
    assert_eq!(
        lowering_code(
            &entry()
                .with_callee(tuple_predicate())
                .with_contracts(vec![precondition(target, tuple_predicate_id)])
        ),
        LowerErrorCode::SignatureMismatch,
        "a predicate that does not answer Bool"
    );

    // Operands that do not equal the predicate parameters.
    assert_eq!(
        lowering_code(
            &Fixture::new(
                &[TypeExpr::Bool],
                &[step(
                    Opcode::ContractAssert,
                    vec![],
                    Immediate::Entity(id(200)),
                    assertion_type(),
                )],
                Vec::new(),
            )
            .with_callee(bool_predicate())
            .with_contracts(vec![precondition(target, predicate_id)])
        ),
        LowerErrorCode::SignatureMismatch,
        "operands that do not equal the predicate parameters"
    );

    // The declared result must be the exact contract result type.
    assert_eq!(
        lowering_code(
            &Fixture::new(
                &[TypeExpr::Bool],
                &[step(
                    Opcode::ContractAssert,
                    vec![Arg::P(0)],
                    Immediate::Entity(id(200)),
                    TypeExpr::Bool,
                )],
                Vec::new(),
            )
            .with_callee(bool_predicate())
            .with_contracts(vec![precondition(target, predicate_id)])
        ),
        LowerErrorCode::SignatureMismatch,
        "a declared result that is not the contract result"
    );

    // The restricted profile still refuses the opcode itself.
    assert!(matches!(
        lower_function(entry().input(CacheProfile::RESTRICTED_V1)).unwrap_err(),
        LoweringError::Lower(error) if error.code() == LowerErrorCode::OpcodeUnsupported
    ));
}

fn call(function: u8) -> Immediate {
    Immediate::Function(FunctionRefValue {
        function: id(function),
        type_arguments: Vec::new(),
    })
}

/// Function 41: `(a, b) -> b` through a tuple.
fn second_of_two() -> Fixture {
    Fixture::with_base(
        40,
        &[u64_type(), u64_type()],
        &[
            step(
                Opcode::TupleNew,
                vec![Arg::P(0), Arg::P(1)],
                Immediate::None,
                TypeExpr::Tuple(vec![u64_type(), u64_type()]),
            ),
            step(
                Opcode::TupleGet,
                vec![Arg::R(0)],
                Immediate::Index(1),
                u64_type(),
            ),
        ],
        Vec::new(),
        Vec::new(),
    )
}

/// Function 81: `(a, b) -> second_of_two(b, a)`.
fn swapped_call() -> Fixture {
    Fixture::with_base(
        80,
        &[u64_type(), u64_type()],
        &[step(
            Opcode::CallDirect,
            vec![Arg::P(1), Arg::P(0)],
            call(41),
            u64_type(),
        )],
        Vec::new(),
        Vec::new(),
    )
}

fn calling_entry(callee: u8) -> Fixture {
    Fixture::new(
        &[u64_type(), u64_type()],
        &[step(
            Opcode::CallDirect,
            vec![Arg::P(0), Arg::P(1)],
            call(callee),
            u64_type(),
        )],
        Vec::new(),
    )
}

#[test]
fn e6_direct_calls_open_frames_share_budgets_and_stop_at_the_depth_ceiling() {
    let entry = calling_entry(41).with_callee(second_of_two());
    let outcome = execute_function(
        entry.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![uint(3), uint(4)],
            limits: limits(),
        },
    )
    .unwrap();
    assert_eq!(outcome.termination, ExecutionTermination::Success(uint(4)));
    // One call instruction plus the callee's two instructions.
    assert_eq!(outcome.instruction_count, 3);
    let lowered = lower_function(entry.input(CacheProfile::EXTENDED_V1)).unwrap();
    assert_eq!(lowered.callees.len(), 1);
    assert_eq!(lowered.callees[0].function, id(41));

    let nested = calling_entry(81)
        .with_callee(second_of_two())
        .with_callee(swapped_call());
    assert_eq!(success(&nested, vec![uint(3), uint(4)]), uint(3));
    let lowered = lower_function(nested.input(CacheProfile::EXTENDED_V1)).unwrap();
    assert_eq!(
        lowered
            .callees
            .iter()
            .map(|callee| callee.function)
            .collect::<Vec<_>>(),
        vec![id(41), id(81)]
    );
    // The callee table is part of the bytes: the same entry without the
    // nested callee encodes differently.
    let direct = lower_function(entry.input(CacheProfile::EXTENDED_V1)).unwrap();
    assert_ne!(direct.bytes, lowered.bytes);

    let recursive = Fixture::new(
        &[u64_type()],
        &[step(
            Opcode::CallDirect,
            vec![Arg::P(0)],
            call(1),
            u64_type(),
        )],
        Vec::new(),
    )
    .with_self();
    let outcome = execute_function(
        recursive.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![uint(1)],
            limits: ExecutionLimits {
                max_instructions: 100_000,
                max_fuel: 100_000,
                max_value_units: 100_000_000,
                max_output_units: 10_000,
                cancel_at_fuel: None,
            },
        },
    )
    .unwrap();
    assert_eq!(
        outcome.termination,
        ExecutionTermination::ResourceLimit(ResourceKind::CallDepth)
    );
    // Fuel is charged per call up front and shared across frames; the
    // instruction count of a call lands when the callee returns.
    let starved = execute_function(
        recursive.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![uint(1)],
            limits: ExecutionLimits {
                max_instructions: 100_000,
                max_fuel: 10,
                max_value_units: 100_000_000,
                max_output_units: 10_000,
                cancel_at_fuel: None,
            },
        },
    )
    .unwrap();
    assert_eq!(
        starved.termination,
        ExecutionTermination::ResourceLimit(ResourceKind::Fuel)
    );

    // Rejections.
    let wrong_types = Fixture::new(
        &[u64_type(), TypeExpr::Text],
        &[step(
            Opcode::CallDirect,
            vec![Arg::P(0), Arg::P(1)],
            call(41),
            u64_type(),
        )],
        Vec::new(),
    )
    .with_callee(second_of_two());
    assert_eq!(
        lowering_code(&wrong_types),
        LowerErrorCode::SignatureMismatch
    );
    let unknown = calling_entry(66);
    assert_eq!(lowering_code(&unknown), LowerErrorCode::ImmediateMismatch);
    let generic = Fixture::new(
        &[u64_type(), u64_type()],
        &[step(
            Opcode::CallDirect,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::Function(FunctionRefValue {
                function: id(41),
                type_arguments: vec![u64_type()],
            }),
            u64_type(),
        )],
        Vec::new(),
    )
    .with_callee(second_of_two());
    assert_eq!(lowering_code(&generic), LowerErrorCode::ImmediateMismatch);
    let broken_callee = Fixture::with_base(
        40,
        &[u64_type(), u64_type()],
        &[step(
            Opcode::TupleGet,
            vec![Arg::P(0)],
            Immediate::Index(0),
            u64_type(),
        )],
        Vec::new(),
        Vec::new(),
    );
    let propagated = calling_entry(41).with_callee(broken_callee);
    assert_eq!(
        lowering_code(&propagated),
        LowerErrorCode::SignatureMismatch
    );
}

/// Prints the E1 vectors for `scripts/generate_vm_extended_fixtures.py`.
#[test]
#[ignore = "fixture refresh emitter"]
fn emit_vm_extended_vectors_for_fixture_refresh() {
    use sley_ssmc::fingerprint::hash_validated_value;
    fn hex(bytes: &[u8]) -> String {
        use core::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
    }
    let vectors: Vec<(&str, Fixture, Vec<ConstValue>)> = vec![
        (
            "tuple-project",
            Fixture::new(
                &[u64_type(), TypeExpr::Text],
                &[
                    step(
                        Opcode::TupleNew,
                        vec![Arg::P(0), Arg::P(1)],
                        Immediate::None,
                        TypeExpr::Tuple(vec![u64_type(), TypeExpr::Text]),
                    ),
                    step(
                        Opcode::TupleGet,
                        vec![Arg::R(0)],
                        Immediate::Index(1),
                        TypeExpr::Text,
                    ),
                ],
                Vec::new(),
            ),
            vec![uint(7), text("seven")],
        ),
        (
            "vector-set-out-of-range",
            Fixture::new(
                &[u64_type(), u64_type(), u64_type()],
                &[
                    step(
                        Opcode::VectorNew,
                        vec![Arg::P(0)],
                        Immediate::None,
                        TypeExpr::Vector(Box::new(u64_type())),
                    ),
                    step(
                        Opcode::VectorSet,
                        vec![Arg::R(0), Arg::P(1), Arg::P(2)],
                        Immediate::None,
                        TypeExpr::Result {
                            ok: Box::new(TypeExpr::Vector(Box::new(u64_type()))),
                            error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
                        },
                    ),
                ],
                Vec::new(),
            ),
            vec![uint(1), uint(3), uint(9)],
        ),
        (
            "signed-less-than",
            Fixture::new(
                &[
                    TypeExpr::SInt(IntegerWidth::from_bits(32)),
                    TypeExpr::SInt(IntegerWidth::from_bits(32)),
                ],
                &[step(
                    Opcode::LessThan,
                    vec![Arg::P(0), Arg::P(1)],
                    Immediate::None,
                    TypeExpr::Bool,
                )],
                Vec::new(),
            ),
            vec![sint(-3), sint(2)],
        ),
        (
            "constant-ref",
            Fixture::new(
                &[TypeExpr::Bool],
                &[step(
                    Opcode::ConstantRef,
                    vec![],
                    Immediate::Entity(id(200)),
                    TypeExpr::Text,
                )],
                vec![ConstantDefinition {
                    entity_id: id(200),
                    value: text("frozen"),
                }],
            ),
            vec![boolean(true)],
        ),
        (
            "int-add-overflow",
            checked_fixture(Opcode::IntAddChecked, false, 8),
            vec![int_value(false, 8, 200), int_value(false, 8, 56)],
        ),
        (
            "int-div-signed-min",
            checked_fixture(Opcode::IntDivChecked, true, 16),
            vec![int_value(true, 16, -32_768), int_value(true, 16, -1)],
        ),
        (
            "int-shl-signed",
            checked_fixture(Opcode::IntShlChecked, true, 32),
            vec![int_value(true, 32, -3), int_value(false, 32, 4)],
        ),
        (
            "float-div-canonical-nan",
            float_fixture(Opcode::FloatDiv, TypeExpr::F64, 2),
            vec![f64v(0.0), f64v(0.0)],
        ),
        (
            "float-fma-single-rounding",
            float_fixture(Opcode::FloatFma, TypeExpr::F64, 3),
            vec![f64v(0.1), f64v(10.0), f64v(-1.0)],
        ),
        (
            "float-less-than-nan",
            float_fixture(Opcode::LessThan, TypeExpr::F32, 2),
            vec![f32v(f32::NAN), f32v(1.0)],
        ),
        (
            "record-get-field",
            Fixture::with_types(
                &[u64_type(), TypeExpr::Text],
                &[
                    step(
                        Opcode::RecordNew,
                        vec![Arg::P(0), Arg::P(1)],
                        Immediate::Entity(id(50)),
                        pair_type(),
                    ),
                    step(
                        Opcode::RecordGet,
                        vec![Arg::R(0)],
                        Immediate::Field(member(0xB2)),
                        TypeExpr::Text,
                    ),
                ],
                Vec::new(),
                definitions(),
            ),
            vec![uint(4), text("four")],
        ),
        (
            "variant-get-none",
            Fixture::with_types(
                &[TypeExpr::Bool],
                &[
                    step(Opcode::VariantNew, vec![], circle(0xC2), shape_type()),
                    step(
                        Opcode::VariantGet,
                        vec![Arg::R(0)],
                        circle(0xC1),
                        TypeExpr::Option(Box::new(u64_type())),
                    ),
                ],
                Vec::new(),
                definitions(),
            ),
            vec![boolean(true)],
        ),
        (
            "map-new-sorted",
            Fixture::new(
                &[u64_type(), TypeExpr::Text, u64_type(), TypeExpr::Text],
                &[step(
                    Opcode::MapNew,
                    vec![Arg::P(0), Arg::P(1), Arg::P(2), Arg::P(3)],
                    Immediate::None,
                    map_new_type(),
                )],
                Vec::new(),
            ),
            vec![uint(300), text("big"), uint(7), text("small")],
        ),
        (
            "cell-set-get",
            Fixture::new(
                &[u64_type(), u64_type()],
                &[
                    step(
                        Opcode::CellNew,
                        vec![Arg::P(0)],
                        Immediate::None,
                        cell_type(),
                    ),
                    step(
                        Opcode::CellSet,
                        vec![Arg::R(0), Arg::P(1)],
                        Immediate::None,
                        TypeExpr::Unit,
                    ),
                    step(
                        Opcode::CellGet,
                        vec![Arg::R(0)],
                        Immediate::None,
                        u64_type(),
                    ),
                ],
                Vec::new(),
            ),
            vec![uint(1), uint(2)],
        ),
        (
            "value-hash-text",
            Fixture::new(
                &[TypeExpr::Text],
                &[step(
                    Opcode::ValueHash,
                    vec![Arg::P(0)],
                    Immediate::None,
                    TypeExpr::Bytes,
                )],
                Vec::new(),
            ),
            vec![text("hash me")],
        ),
        (
            "global-get-constant",
            {
                let (globals, constants) = global_seventy_seven();
                Fixture::new(
                    &[TypeExpr::Bool],
                    &[step(
                        Opcode::GlobalGet,
                        vec![],
                        Immediate::Entity(id(60)),
                        u64_type(),
                    )],
                    Vec::new(),
                )
                .with_globals(globals, constants)
            },
            vec![boolean(true)],
        ),
        (
            "call-direct-second",
            calling_entry(41).with_callee(second_of_two()),
            vec![uint(3), uint(4)],
        ),
        (
            "call-direct-nested",
            calling_entry(81)
                .with_callee(second_of_two())
                .with_callee(swapped_call()),
            vec![uint(3), uint(4)],
        ),
        (
            "result-err",
            Fixture::new(
                &[TypeExpr::Text],
                &[step(
                    Opcode::ResultErr,
                    vec![Arg::P(0)],
                    Immediate::None,
                    TypeExpr::Result {
                        ok: Box::new(u64_type()),
                        error: Box::new(TypeExpr::Text),
                    },
                )],
                Vec::new(),
            ),
            vec![text("bad")],
        ),
        // Slice E7a: the same assertion, once holding and once violated.
        (
            "contract-assert-holds",
            asserting_entry(),
            vec![boolean(true)],
        ),
        (
            "contract-assert-violated",
            asserting_entry(),
            vec![boolean(false)],
        ),
    ];
    for (label, fixture, inputs) in vectors {
        let lowered = lower_function(fixture.input(CacheProfile::EXTENDED_V1)).unwrap();
        let outcome = execute_function(
            fixture.input(CacheProfile::EXTENDED_V1),
            ExecutionRequest {
                inputs,
                limits: limits(),
            },
        )
        .unwrap();
        let ExecutionTermination::Success(value) = &outcome.termination else {
            panic!("{label}: not a success");
        };
        let value_hash = hash_validated_value(SchemaEpochId::from_bytes([8; 32]), value).unwrap();
        // The vector's subject is the entry function's last operation; a
        // fixture with a callee also carries the callee's operations.
        let entry_blocks: Vec<EntityId> = fixture.function.blocks.clone();
        let subject = fixture
            .operations
            .iter()
            .rfind(|operation| entry_blocks.contains(&operation.block))
            .expect("the entry function runs at least one operation");
        println!(
            "VM_EXTENDED_VECTOR|{label}|{}|{}|{}|{}|{}|{}",
            subject.opcode.tag(),
            hex(&lowered.bytes),
            hex(lowered.cache_key.as_bytes()),
            hex(value_hash.as_bytes()),
            hex(outcome.observation_id.as_bytes()),
            outcome.instruction_count
        );
    }
}

/// Two ordered maps that differ only in entry order are one semantic value,
/// and the extended profile must never give them two identities.
///
/// E4 fixes map order at the keys' S20-350 canonical bytes, but S20-210 does
/// not establish it: `TYPE_SYSTEM_V1.md` section 5 reserves the byte ordering
/// to the SCB encoder/decoder and forbids the checker from reimplementing it
/// or silently sorting a decoded constant. Every production path into the VM
/// crosses that codec, so the invariant holds there; the public
/// `execute_function` boundary did not check it, and `equal` and `value_hash`
/// read entry order structurally. So the VM refuses a value the codec will
/// not encode, at each boundary where one is supplied from outside: the
/// request inputs, and the constants `constant_ref` and `global_get` name.
#[test]
fn e4_maps_differing_only_in_entry_order_never_reach_equality_or_hashing() {
    let canonical = {
        let mut entries = vec![(300_u128, "big"), (7, "small")];
        entries.sort_by_key(|(key, _)| sley_mutate::encode_const_value(&uint(*key)).unwrap());
        map_of(entries)
    };
    let ConstData::Map(ordered) = &canonical.data else {
        panic!("map expected");
    };
    let mut flipped = ordered.clone();
    flipped.reverse();
    let reversed = ConstValue {
        value_type: map_type(),
        data: ConstData::Map(flipped),
    };
    // The two carry exactly the same entries, so any difference in what the
    // VM observes of them is a difference in representation alone.
    let entries_of = |value: &ConstValue| {
        let ConstData::Map(entries) = &value.data else {
            panic!("map expected");
        };
        let mut entries = entries.clone();
        entries.sort_by_key(|entry| sley_mutate::encode_const_value(&entry.key).unwrap());
        entries
    };
    assert_eq!(entries_of(&canonical), entries_of(&reversed));
    assert_ne!(canonical.data, reversed.data, "they differ only in order");

    // S20-210 accepts both, deliberately, and the codec separates them.
    let environment = TypeEnvironment::new(Vec::new()).unwrap();
    environment
        .check_constant(&canonical)
        .expect("the canonical order is a valid constant");
    environment
        .check_constant(&reversed)
        .expect("S20-210 imposes no entry order and must not sort");
    sley_mutate::encode_const_value(&canonical).expect("the canonical order encodes");
    sley_mutate::encode_const_value(&reversed).expect_err("the codec rejects the other order");

    // Inputs: the canonical pair is equal and hashes alike; the other order is
    // refused before either observation can be derived from it.
    let equality = Fixture::new(
        &[map_type(), map_type()],
        &[step(
            Opcode::Equal,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            TypeExpr::Bool,
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&equality, vec![canonical.clone(), canonical.clone()]),
        boolean(true)
    );
    let refused = execute_function(
        equality.input(CacheProfile::EXTENDED_V1),
        ExecutionRequest {
            inputs: vec![canonical.clone(), reversed.clone()],
            limits: limits(),
        },
    )
    .unwrap_err();
    assert_eq!(
        refused,
        ExecutionError::Exec(crate::ExecutionErrorCode::InputNotCanonical),
        "a non-canonical map input is refused, not compared"
    );

    let hasher = Fixture::new(
        &[map_type()],
        &[step(
            Opcode::ValueHash,
            vec![Arg::P(0)],
            Immediate::None,
            TypeExpr::Bytes,
        )],
        Vec::new(),
    );
    assert_eq!(
        success(&hasher, vec![canonical.clone()]),
        success(&hasher, vec![canonical.clone()])
    );
    assert_eq!(
        execute_function(
            hasher.input(CacheProfile::EXTENDED_V1),
            ExecutionRequest {
                inputs: vec![reversed.clone()],
                limits: limits(),
            },
        )
        .unwrap_err(),
        ExecutionError::Exec(crate::ExecutionErrorCode::InputNotCanonical),
        "a non-canonical map input is refused, not hashed"
    );

    // Constants: `constant_ref` names an artifact value the codec would reject.
    let constant_fixture = |value: ConstValue| {
        Fixture::new(
            &[TypeExpr::Bool],
            &[step(
                Opcode::ConstantRef,
                vec![],
                Immediate::Entity(id(200)),
                map_type(),
            )],
            vec![ConstantDefinition {
                entity_id: id(200),
                value,
            }],
        )
    };
    assert_eq!(
        success(&constant_fixture(canonical.clone()), vec![boolean(true)]),
        canonical
    );
    assert_eq!(
        lowering_code(&constant_fixture(reversed.clone())),
        LowerErrorCode::ImmediateMismatch
    );

    // Globals: the same value reached through an initializer.
    let global_fixture = |value: ConstValue| {
        Fixture::new(
            &[TypeExpr::Bool],
            &[step(
                Opcode::GlobalGet,
                vec![],
                Immediate::Entity(id(60)),
                map_type(),
            )],
            Vec::new(),
        )
        .with_globals(
            vec![GlobalValueDefinition {
                entity_id: id(60),
                value_type: map_type(),
                initializer: id(61),
                visibility: Visibility::Private,
            }],
            vec![ConstantDefinition {
                entity_id: id(61),
                value,
            }],
        )
    };
    assert_eq!(
        success(&global_fixture(canonical.clone()), vec![boolean(true)]),
        canonical
    );
    assert_eq!(
        lowering_code(&global_fixture(reversed)),
        LowerErrorCode::ImmediateMismatch
    );
}

/// The judgment entry enforces the canonical-constant precondition exactly
/// like lowering does (contract section 3.1 invariant): a candidate that
/// reaches `VALID` is lowerable under `EXTENDED_V1`, so the judgment must
/// refuse the same non-canonical `constant_ref` that lowering refuses.
#[test]
fn judgment_rejects_non_canonical_referenced_constant() {
    let reference = |value: ConstValue| {
        Fixture::new(
            &[TypeExpr::Bool],
            &[step(
                Opcode::ConstantRef,
                vec![],
                Immediate::Entity(id(200)),
                map_type(),
            )],
            vec![ConstantDefinition {
                entity_id: id(200),
                value,
            }],
        )
    };
    let judgment_code = |fixture: &Fixture| match judge_function_operations(
        fixture.input(CacheProfile::EXTENDED_V1),
    )
    .unwrap_err()
    {
        LoweringError::Lower(error) => error.code(),
        LoweringError::Cfg(error) => panic!("cfg failure: {error}"),
    };
    // Canonical order: judgment and lowering both accept.
    let mut entries = vec![(300_u128, "big"), (7, "small")];
    entries.sort_by_key(|(key, _)| sley_mutate::encode_const_value(&uint(*key)).unwrap());
    let canonical = reference(map_of(entries));
    assert!(lower_function(canonical.input(CacheProfile::EXTENDED_V1)).is_ok());
    assert!(judge_function_operations(canonical.input(CacheProfile::EXTENDED_V1)).is_ok());
    // Out-of-order keys: the codec rejects the value, and both entries
    // refuse it with the immediate code.
    let mut flipped = vec![(300_u128, "big"), (7, "small")];
    flipped.sort_by_key(|(key, _)| sley_mutate::encode_const_value(&uint(*key)).unwrap());
    flipped.reverse();
    let reversed = reference(map_of(flipped));
    assert_eq!(lowering_code(&reversed), LowerErrorCode::ImmediateMismatch);
    assert_eq!(judgment_code(&reversed), LowerErrorCode::ImmediateMismatch);
    // Duplicate keys likewise: refused by the codec, the lowering, and the
    // judgment with the same code.
    let duplicated = reference(map_of(vec![(7_u128, "first"), (7, "second")]));
    assert_eq!(
        lowering_code(&duplicated),
        LowerErrorCode::ImmediateMismatch
    );
    assert_eq!(
        judgment_code(&duplicated),
        LowerErrorCode::ImmediateMismatch
    );
}

/// Judgment accepts exactly what lowering accepts, minus the documented
/// judgment exclusions (contract section 3.1 invariant, section 5
/// differential-test obligation): lowering accepted implies judgment
/// accepted, and judgment refused implies lowering refused. The designed
/// asymmetry runs one way only — the judgment skips the graph validation,
/// the cache key, and the type-parameter/effect/contract refusal, so it may
/// accept what lowering refuses, never the reverse.
#[test]
fn judgment_acceptance_matches_lowering_acceptance() {
    let boolean = Fixture::new(
        &[TypeExpr::Bool, TypeExpr::Bool],
        &[step(
            Opcode::BoolAnd,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            TypeExpr::Bool,
        )],
        Vec::new(),
    );
    let mut entries = vec![(300_u128, "big"), (7, "small")];
    entries.sort_by_key(|(key, _)| sley_mutate::encode_const_value(&uint(*key)).unwrap());
    let canonical = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::ConstantRef,
            vec![],
            Immediate::Entity(id(200)),
            map_type(),
        )],
        vec![ConstantDefinition {
            entity_id: id(200),
            value: map_of(entries),
        }],
    );
    let mut flipped = vec![(300_u128, "big"), (7, "small")];
    flipped.sort_by_key(|(key, _)| sley_mutate::encode_const_value(&uint(*key)).unwrap());
    flipped.reverse();
    let reversed = Fixture::new(
        &[TypeExpr::Bool],
        &[step(
            Opcode::ConstantRef,
            vec![],
            Immediate::Entity(id(200)),
            map_type(),
        )],
        vec![ConstantDefinition {
            entity_id: id(200),
            value: map_of(flipped),
        }],
    );
    let bad_signature = Fixture::new(
        &[TypeExpr::Bool, TypeExpr::Bool],
        &[step(
            Opcode::BoolAnd,
            vec![Arg::P(0)],
            Immediate::None,
            TypeExpr::Bool,
        )],
        Vec::new(),
    );
    let mut effected = Fixture::new(
        &[TypeExpr::Bool, TypeExpr::Bool],
        &[step(
            Opcode::BoolAnd,
            vec![Arg::P(0), Arg::P(1)],
            Immediate::None,
            TypeExpr::Bool,
        )],
        Vec::new(),
    );
    effected.function.effects.push(id(50));
    for fixture in [&boolean, &canonical, &reversed, &bad_signature, &effected] {
        let judged = judge_function_operations(fixture.input(CacheProfile::EXTENDED_V1));
        let lowered = lower_function(fixture.input(CacheProfile::EXTENDED_V1));
        match (&judged, &lowered) {
            (Ok(_), Ok(_)) | (Err(_), Err(_)) | (Ok(_), Err(_)) => {}
            (Err(error), Ok(_)) => {
                panic!("judgment refused what lowering accepted: {error:?}")
            }
        }
    }
    // The designed asymmetry, pinned: effects belong to the S20-230 owner,
    // so the judgment accepts while lowering refuses with the profile code.
    assert!(judge_function_operations(effected.input(CacheProfile::EXTENDED_V1)).is_ok());
    assert_eq!(lowering_code(&effected), LowerErrorCode::ProfileUnsupported);
}

/// A cell's contents are live value units, not a free handle.
///
/// `cell_new` and `cell_set` clone their value into a table that outlives the
/// instruction. Charging only the result charged the `LocalCell` handle, so a
/// value of any size cost the same few units: the budget stayed intact while
/// host memory grew without bound, and contract E5's "their contents count as
/// live value units" was false. Vulcan raised it against the S20-260 surface.
#[test]
fn e5_each_cell_charges_its_contents_so_the_budget_bounds_the_table() {
    let payload = ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(vec![7_u8; 4096]),
    };
    let contents = crate::execution_value_units(&payload);
    let cell_of_bytes = TypeExpr::LocalCell(Box::new(TypeExpr::Bytes));
    let stash = |count: usize| {
        let mut steps: Vec<Step> = (0..count)
            .map(|_| {
                step(
                    Opcode::CellNew,
                    vec![Arg::P(0)],
                    Immediate::None,
                    cell_of_bytes.clone(),
                )
            })
            .collect();
        steps.push(step(
            Opcode::CellGet,
            vec![Arg::R(0)],
            Immediate::None,
            TypeExpr::Bytes,
        ));
        Fixture::new(&[TypeExpr::Bytes], &steps, Vec::new())
    };
    let run = |count: usize, max_value_units: u64| {
        execute_function(
            stash(count).input(CacheProfile::EXTENDED_V1),
            ExecutionRequest {
                inputs: vec![payload.clone()],
                limits: ExecutionLimits {
                    max_instructions: 1_000,
                    max_fuel: 1_000_000,
                    max_value_units,
                    max_output_units: 1_000_000,
                    cancel_at_fuel: None,
                },
            },
        )
        .expect("executes")
    };

    // Every additional cell costs at least what it stores.
    let one = run(1, 1_000_000);
    let two = run(2, 1_000_000);
    let eight = run(8, 1_000_000);
    let per_cell = two.peak_value_units - one.peak_value_units;
    assert!(
        per_cell >= contents,
        "a second cell charged {per_cell} for {contents} units of contents"
    );
    assert_eq!(
        eight.peak_value_units - one.peak_value_units,
        per_cell * 7,
        "cells past the first must each cost the same"
    );

    // So a budget sized for two cells stops at two, instead of letting the
    // execution hold six more tables' worth of memory anyway.
    let two_cell_budget = one.peak_value_units + per_cell;
    assert!(
        matches!(
            run(2, two_cell_budget).termination,
            ExecutionTermination::Success(_)
        ),
        "the budget measured for two cells refused two cells"
    );
    assert_eq!(
        run(8, two_cell_budget).termination,
        ExecutionTermination::ResourceLimit(ResourceKind::ValueUnits),
        "eight cells fit in a budget sized for two"
    );
}
