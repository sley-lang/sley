//! Package-path nested input type agreement (2.0.1, erratum E2 follow-up).
//!
//! `execute_approved_package` and `execute_approved_package_v2` compare an
//! input's top-level `value_type` with its parameter register, but the
//! codec carries every nested value's own `value_type` without comparing it
//! with the type its container declares. These fixtures use only the public
//! `sley-vm` surface to pin that:
//!
//! * a nested value whose own type differs from the element type its
//!   container declares (an `Option` payload, a `Vector` element, a tuple
//!   element, a map key or value, a `Result` arm, a record field, a variant
//!   payload), even when its data is in width for the type it claims, is
//!   refused before execution with `VM_EXEC_INPUT_NOT_CANONICAL` on both
//!   package versions, at any depth;
//! * data whose form differs from its own declared type, and record or
//!   variant values that disagree with the package layout, are refused the
//!   same way;
//! * the S20-270 lowering path still refuses the same input with its
//!   earlier `TYPE_*` judgment;
//! * every in-type input executes and returns exactly what it was given.

use sley_check::{TypeEnvironment, TypeErrorCode};
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    Block, ConstData, ConstValue, FieldConst, FunctionGraph, IntegerWidth, MapEntryConst, MemberId,
    NamedType, Parameter, ParameterRole, Reachability, RecordConst, RecordField, ResultConst,
    ReturnTerminator, Terminator, TypeDefForm, TypeDefinition, TypeExpr, ValueRef, VariantCase,
    VariantConst, Visibility,
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

fn member(byte: u8) -> MemberId {
    MemberId::from_bytes([byte; 32])
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

fn uint_type(bits: u16) -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(bits))
}

fn uint(bits: u16, value: u128) -> ConstValue {
    ConstValue {
        value_type: uint_type(bits),
        data: ConstData::UInt(value),
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
        data: ConstData::Text(value.to_owned()),
    }
}

/// `record Pair { count: UInt(8), flag: Bool }`.
const PAIR: u8 = 50;
const PAIR_COUNT: u8 = 51;
const PAIR_FLAG: u8 = 52;
/// `variant Choice { empty, small(UInt(8)) }`.
const CHOICE: u8 = 60;
const CHOICE_EMPTY: u8 = 61;
const CHOICE_SMALL: u8 = 62;

fn definitions() -> Vec<TypeDefinition> {
    vec![
        TypeDefinition {
            entity_id: id(PAIR),
            type_parameters: Vec::new(),
            form: TypeDefForm::Record(vec![
                RecordField {
                    member_id: member(PAIR_COUNT),
                    value_type: uint_type(8),
                    visibility: Visibility::Private,
                },
                RecordField {
                    member_id: member(PAIR_FLAG),
                    value_type: TypeExpr::Bool,
                    visibility: Visibility::Private,
                },
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
        TypeDefinition {
            entity_id: id(CHOICE),
            type_parameters: Vec::new(),
            form: TypeDefForm::Variant(vec![
                VariantCase {
                    member_id: member(CHOICE_EMPTY),
                    payload_type: None,
                },
                VariantCase {
                    member_id: member(CHOICE_SMALL),
                    payload_type: Some(uint_type(8)),
                },
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
    ]
}

fn named(definition: u8) -> TypeExpr {
    TypeExpr::Named(NamedType {
        definition: id(definition),
        arguments: Vec::new(),
    })
}

fn pair(count: ConstValue, flag: ConstValue) -> ConstValue {
    ConstValue {
        value_type: named(PAIR),
        data: ConstData::Record(RecordConst {
            definition: id(PAIR),
            fields: vec![
                FieldConst {
                    member_id: member(PAIR_COUNT),
                    value: count,
                },
                FieldConst {
                    member_id: member(PAIR_FLAG),
                    value: flag,
                },
            ],
        }),
    }
}

fn choice(case: u8, payload: Option<ConstValue>) -> ConstValue {
    ConstValue {
        value_type: named(CHOICE),
        data: ConstData::Variant(VariantConst {
            definition: id(CHOICE),
            member_id: member(case),
            payload: payload.map(Box::new),
        }),
    }
}

fn some(value_type: &TypeExpr, payload: ConstValue) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Option(Box::new(value_type.clone())),
        data: ConstData::Option(Some(Box::new(payload))),
    }
}

fn vector(element: &TypeExpr, values: Vec<ConstValue>) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Vector(Box::new(element.clone())),
        data: ConstData::Sequence(values),
    }
}

/// A single-block `(T) -> T` Function that returns its parameter, plus the
/// inventories its package carries, so the input itself is the result.
struct Identity {
    types: TypeEnvironment,
    entry: FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
}

impl Identity {
    fn new(value_type: &TypeExpr) -> Self {
        let function = id(1);
        let block = id(2);
        let parameter = id(10);
        Self {
            types: TypeEnvironment::new(definitions()).expect("fixture definitions are valid"),
            entry: FunctionGraph {
                entity_id: function,
                type_parameters: Vec::new(),
                parameters: vec![parameter],
                result_type: value_type.clone(),
                effects: Vec::new(),
                entry_block: block,
                blocks: vec![block],
                contracts: Vec::new(),
                visibility: Visibility::Private,
            },
            parameters: vec![Parameter {
                entity_id: parameter,
                owner: function,
                role: ParameterRole::Function,
                ordinal: 0,
                value_type: value_type.clone(),
            }],
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
        }
    }

    fn lowering_input(&self) -> LoweringInput<'_> {
        LoweringInput {
            types: &self.types,
            function: &self.entry,
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &[],
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
            type_definitions: definitions(),
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
            operations: &[],
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
            operations: &[],
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

    /// Runs `input` through both package versions and requires them to agree.
    fn run(&self, input: &ConstValue) -> Result<ExecutionOutcome, PackageExecutionError> {
        let request = || ExecutionRequest {
            inputs: vec![input.clone()],
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

    fn lowering_path(&self, input: &ConstValue) -> Result<ExecutionOutcome, ExecutionError> {
        execute_function(
            self.lowering_input(),
            ExecutionRequest {
                inputs: vec![input.clone()],
                limits: limits(),
            },
        )
    }
}

/// Requires `input` (typed exactly as `declared` at the top level) to be
/// refused before execution on both package versions.
fn assert_not_canonical(declared: &TypeExpr, input: &ConstValue, case: &str) {
    assert_eq!(
        &input.value_type, declared,
        "{case}: the top-level type must match, so only nesting is wrong"
    );
    match Identity::new(declared).run(input) {
        Err(PackageExecutionError::Execution(ExecutionError::Exec(
            ExecutionErrorCode::InputNotCanonical,
        ))) => {}
        other => panic!("{case}: expected VM_EXEC_INPUT_NOT_CANONICAL, got {other:?}"),
    }
}

fn assert_round_trips(input: &ConstValue, case: &str) {
    let outcome = Identity::new(&input.value_type)
        .run(input)
        .unwrap_or_else(|error| panic!("{case}: in-type input refused: {error:?}"));
    assert_eq!(
        outcome.termination,
        ExecutionTermination::Success(input.clone()),
        "{case}"
    );
}

#[test]
fn option_with_a_mistyped_payload_is_refused() {
    let option = TypeExpr::Option(Box::new(uint_type(8)));
    // In width for the `UInt(128)` it claims, but the Option declares UInt(8).
    assert_not_canonical(
        &option,
        &some(&uint_type(8), uint(128, 5)),
        "u128 in Option<u8>",
    );
    assert_not_canonical(
        &option,
        &some(&uint_type(8), uint(16, 5)),
        "u16 in Option<u8>",
    );
    assert_not_canonical(
        &option,
        &some(&uint_type(8), text("5")),
        "Text in Option<u8>",
    );
    // The S20-270 lowering path keeps its earlier judgment for the same input.
    match Identity::new(&option).lowering_path(&some(&uint_type(8), uint(128, 5))) {
        Err(ExecutionError::Type(error)) => {
            assert_eq!(error.code(), TypeErrorCode::ImplicitCoercion);
        }
        other => panic!("lowering path: expected TYPE_IMPLICIT_COERCION, got {other:?}"),
    }
}

#[test]
fn list_with_one_mistyped_element_is_refused() {
    let element = uint_type(8);
    let declared = TypeExpr::Vector(Box::new(element.clone()));
    assert_not_canonical(
        &declared,
        &vector(
            &element,
            vec![uint(8, 1), uint(8, 2), uint(64, 3), uint(8, 4)],
        ),
        "u64 among Vector<u8> elements",
    );
    assert_not_canonical(
        &declared,
        &vector(&element, vec![uint(8, 1), boolean(true)]),
        "Bool among Vector<u8> elements",
    );
    // A tuple element is held to its own position's type.
    let tuple = TypeExpr::Tuple(vec![uint_type(8), TypeExpr::Bool]);
    assert_not_canonical(
        &tuple,
        &ConstValue {
            value_type: tuple.clone(),
            data: ConstData::Sequence(vec![uint(32, 1), boolean(true)]),
        },
        "u32 in the u8 slot of (u8, Bool)",
    );
    assert_not_canonical(
        &tuple,
        &ConstValue {
            value_type: tuple.clone(),
            data: ConstData::Sequence(vec![uint(8, 1)]),
        },
        "short tuple",
    );
}

#[test]
fn record_with_a_mistyped_field_is_refused() {
    let declared = named(PAIR);
    assert_not_canonical(
        &declared,
        &pair(uint(32, 7), boolean(true)),
        "u32 in the u8 field",
    );
    assert_not_canonical(
        &declared,
        &pair(uint(8, 7), uint(8, 1)),
        "u8 in the Bool field",
    );
    // Field order and identity come from the package layout.
    let mut swapped = pair(uint(8, 7), boolean(true));
    if let ConstData::Record(record) = &mut swapped.data {
        record.fields.swap(0, 1);
    }
    assert_not_canonical(&declared, &swapped, "fields out of layout order");
    let mut missing = pair(uint(8, 7), boolean(true));
    if let ConstData::Record(record) = &mut missing.data {
        record.fields.pop();
    }
    assert_not_canonical(&declared, &missing, "missing field");
    let mut foreign = pair(uint(8, 7), boolean(true));
    if let ConstData::Record(record) = &mut foreign.data {
        record.definition = id(CHOICE);
    }
    assert_not_canonical(&declared, &foreign, "record naming another definition");
    match Identity::new(&declared).lowering_path(&pair(uint(32, 7), boolean(true))) {
        Err(ExecutionError::Type(error)) => {
            assert_eq!(error.code(), TypeErrorCode::ImplicitCoercion);
        }
        other => panic!("lowering path: expected TYPE_IMPLICIT_COERCION, got {other:?}"),
    }
}

#[test]
fn variant_with_a_mistyped_payload_is_refused() {
    let declared = named(CHOICE);
    assert_not_canonical(
        &declared,
        &choice(CHOICE_SMALL, Some(uint(64, 3))),
        "u64 payload in the u8 case",
    );
    assert_not_canonical(
        &declared,
        &choice(CHOICE_SMALL, None),
        "payload missing from a payload case",
    );
    assert_not_canonical(
        &declared,
        &choice(CHOICE_EMPTY, Some(uint(8, 3))),
        "payload on an empty case",
    );
    assert_not_canonical(&declared, &choice(99, None), "unknown case");
}

#[test]
fn result_map_and_deep_mistypes_are_refused() {
    let result = TypeExpr::Result {
        ok: Box::new(uint_type(8)),
        error: Box::new(TypeExpr::Text),
    };
    for (arm, case) in [
        (
            ResultConst::Ok(Box::new(uint(16, 1))),
            "u16 in the u8 ok arm",
        ),
        (
            ResultConst::Err(Box::new(uint(8, 1))),
            "u8 in the Text err arm",
        ),
    ] {
        assert_not_canonical(
            &result,
            &ConstValue {
                value_type: result.clone(),
                data: ConstData::Result(arm),
            },
            case,
        );
    }
    let map = TypeExpr::OrderedMap {
        key: Box::new(uint_type(8)),
        value: Box::new(TypeExpr::Bool),
    };
    for (key, value, case) in [
        (uint(16, 1), boolean(true), "u16 key in Map<u8, Bool>"),
        (uint(8, 1), text("x"), "Text value in Map<u8, Bool>"),
    ] {
        assert_not_canonical(
            &map,
            &ConstValue {
                value_type: map.clone(),
                data: ConstData::Map(vec![MapEntryConst { key, value }]),
            },
            case,
        );
    }
    // The walk reaches every depth: the containers agree down to the last
    // level, where one element inside a record field inside an Option is
    // mistyped.
    let inner = TypeExpr::Vector(Box::new(named(PAIR)));
    let declared = TypeExpr::Option(Box::new(inner.clone()));
    assert_not_canonical(
        &declared,
        &some(
            &inner,
            vector(
                &named(PAIR),
                vec![
                    pair(uint(8, 1), boolean(true)),
                    pair(uint(128, 2), boolean(false)),
                ],
            ),
        ),
        "u128 field three levels down",
    );
    // Data of one form under a type of another, at the top level.
    assert_not_canonical(
        &TypeExpr::Bool,
        &ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Text("true".to_owned()),
        },
        "Text data under Bool",
    );
}

#[test]
fn in_type_nested_inputs_are_accepted_unchanged() {
    let small = uint_type(8);
    let tuple = TypeExpr::Tuple(vec![uint_type(8), TypeExpr::Bool]);
    let result = TypeExpr::Result {
        ok: Box::new(uint_type(8)),
        error: Box::new(TypeExpr::Text),
    };
    let map = TypeExpr::OrderedMap {
        key: Box::new(uint_type(8)),
        value: Box::new(TypeExpr::Bool),
    };
    let pairs = TypeExpr::Vector(Box::new(named(PAIR)));
    for (input, case) in [
        (some(&small, uint(8, 255)), "Option<u8>"),
        (
            ConstValue {
                value_type: TypeExpr::Option(Box::new(small.clone())),
                data: ConstData::Option(None),
            },
            "empty Option<u8>",
        ),
        (vector(&small, vec![uint(8, 0), uint(8, 255)]), "Vector<u8>"),
        (vector(&small, Vec::new()), "empty Vector<u8>"),
        (
            ConstValue {
                value_type: tuple.clone(),
                data: ConstData::Sequence(vec![uint(8, 1), boolean(false)]),
            },
            "(u8, Bool)",
        ),
        (
            ConstValue {
                value_type: result.clone(),
                data: ConstData::Result(ResultConst::Ok(Box::new(uint(8, 9)))),
            },
            "Result ok",
        ),
        (
            ConstValue {
                value_type: result.clone(),
                data: ConstData::Result(ResultConst::Err(Box::new(text("no")))),
            },
            "Result err",
        ),
        (
            ConstValue {
                value_type: map.clone(),
                data: ConstData::Map(vec![MapEntryConst {
                    key: uint(8, 1),
                    value: boolean(true),
                }]),
            },
            "Map<u8, Bool>",
        ),
        (pair(uint(8, 7), boolean(true)), "record"),
        (
            choice(CHOICE_SMALL, Some(uint(8, 3))),
            "variant with payload",
        ),
        (choice(CHOICE_EMPTY, None), "variant without payload"),
        (
            some(
                &pairs,
                vector(
                    &named(PAIR),
                    vec![
                        pair(uint(8, 1), boolean(true)),
                        pair(uint(8, 2), boolean(false)),
                    ],
                ),
            ),
            "Option<Vector<Pair>>",
        ),
    ] {
        assert_round_trips(&input, case);
        // The lowering path accepts the same input with the same answer.
        let lowering = Identity::new(&input.value_type)
            .lowering_path(&input)
            .unwrap_or_else(|error| panic!("{case}: lowering path refused: {error:?}"));
        assert_eq!(
            lowering.termination,
            ExecutionTermination::Success(input.clone()),
            "{case}"
        );
    }
}
