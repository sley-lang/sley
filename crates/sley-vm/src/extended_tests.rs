//! Extended opcode profile tests (slice E1): positive semantics for the data
//! family, the lowering rejection matrix, repeat determinism, the restricted
//! profile's continued refusal, and the fixture emitter.

#![allow(clippy::too_many_lines)]

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    Block, BuiltinFailureKind, BuiltinFailureValue, ConstData, ConstValue, ConstantDefinition,
    FunctionGraph, Immediate, IntegerWidth, Opcode, Operation, OperationResultRef, Parameter,
    ParameterRole, Reachability, ResultConst, ReturnTerminator, Terminator, TypeExpr, ValueRef,
    Visibility,
};

use crate::{
    CacheProfile, ExecutionError, ExecutionLimits, ExecutionRequest, ExecutionTermination,
    LowerErrorCode, LoweringError, LoweringInput, execute_function, lower_function,
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
}

impl Fixture {
    fn new(
        parameter_types: &[TypeExpr],
        steps: &[Step],
        constants: Vec<ConstantDefinition>,
    ) -> Self {
        let function = id(1);
        let block = id(2);
        let parameter_ids: Vec<EntityId> = (0..parameter_types.len())
            .map(|index| id(u8::try_from(10 + index).unwrap()))
            .collect();
        let operation_ids: Vec<EntityId> = (0..steps.len())
            .map(|index| id(u8::try_from(100 + index).unwrap()))
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
            types: TypeEnvironment::new(Vec::new()).unwrap(),
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
        }
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
            globals: &[],
            functions: &[],
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
                &[TypeExpr::F64, TypeExpr::F64],
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
            "an opcode of a later slice",
            Fixture::new(
                &[u64_type(), u64_type()],
                &[step(
                    Opcode::IntAddChecked,
                    vec![Arg::P(0), Arg::P(1)],
                    Immediate::None,
                    TypeExpr::Result {
                        ok: Box::new(u64_type()),
                        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
                    },
                )],
                Vec::new(),
            ),
            LowerErrorCode::OpcodeUnsupported,
        ),
        (
            "an E7 opcode",
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
        println!(
            "VM_EXTENDED_VECTOR|{label}|{}|{}|{}|{}|{}|{}",
            fixture.operations.last().unwrap().opcode.tag(),
            hex(&lowered.bytes),
            hex(lowered.cache_key.as_bytes()),
            hex(value_hash.as_bytes()),
            hex(outcome.observation_id.as_bytes()),
            outcome.instruction_count
        );
    }
}
