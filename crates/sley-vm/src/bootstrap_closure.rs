//! Composed `BOOTSTRAP_PROFILE_1` closure workloads (RW-050 slice 2, §4).
//!
//! Representative multi-operation, multi-block compositions proving the
//! frozen profile is internally closed and expressive enough for
//! compiler-pass-shaped logic: byte decode/reconstruction, vector growth
//! under bounded block-parameter iteration (Form A), symbol tables,
//! sets-as-maps, records/variants, multi-function calls with error
//! propagation, bounds-checked traversal, content hashing, exhaustive
//! variant switches, iterative graph worklists, and image-assembly
//! emission. Every workload is gate-admitted by the production
//! `judge_bootstrap_profile` before it executes, executes to an exact
//! expected termination under the frozen reference budgets, and is emitted
//! as a conformance vector by the ignored emitter at the bottom.
//!
//! State threading uses block parameters (the second Form-A mechanism,
//! unexercised by RW-040's cell-based `cond-drain-loop`); cells stay
//! admitted for RW-040-style state but no closure workload needs them.

//! Each workload builder is one declarative program: overlong by
//! construction, like the fixture builders in `extended_tests.rs`.
#![allow(clippy::too_many_lines)]

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BranchTerminator, BuiltinCase, BuiltinFailureKind, CaseKey, ConstData,
    ConstValue, ConstantDefinition, FunctionGraph, Immediate, MemberId, NamedType, Opcode,
    Operation, OperationResultRef, Parameter, ParameterRole, Reachability, RecordField,
    ResultConst, ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge, TargetEdge, Terminator,
    TrapCode, TrapTerminator, TypeDefForm, TypeDefinition, TypeExpr, ValueRef, VariantCase,
    VariantImmediate, Visibility,
};

use crate::bootstrap::{BootstrapProfileInput, BootstrapProfileVersion, judge_bootstrap_profile};
use crate::extended::{bridge_entry_id, bridge_test_imports};
use crate::{
    CacheProfile, ExecutionLimits, ExecutionOutcome, ExecutionRequest, ExecutionTermination,
    LoweringInput, execute_function, lower_function,
};

/// The invoked subset of the frozen test imports. Gate admission covers
/// exactly the reached imports, so workloads carry no unreferenced rows
/// (RW-070 closure repair); lowering and execution resolve only invoked
/// rows, so emitted bytes and observations are unchanged.
fn bridge_subset(codes: &[[u8; 4]]) -> Vec<sley_ssmc::AdapterImport> {
    bridge_test_imports()
        .into_iter()
        .filter(|row| {
            codes
                .iter()
                .any(|code| row.entity_id == bridge_entry_id(*code))
        })
        .collect()
}

/// Frozen reference budgets every closure vector runs under. The freeze
/// records these values plus the measured per-vector costs proving fit;
/// a closure vector terminating on resources under these budgets is a
/// profile violation, not a passing vector.
const CLOSURE_LIMITS: ExecutionLimits = ExecutionLimits {
    max_instructions: 100_000,
    max_fuel: 10_000_000,
    max_value_units: 100_000_000,
    max_output_units: 10_000_000,
    cancel_at_fuel: None,
};

fn id(byte: u8) -> EntityId {
    EntityId::from_bytes([byte; 32])
}

fn member(byte: u8) -> MemberId {
    MemberId::from_bytes([byte; 32])
}

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(sley_ssmc::IntegerWidth::from_bits(8))
}

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(sley_ssmc::IntegerWidth::from_bits(64))
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

fn uint(value: u128) -> ConstValue {
    ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(value),
    }
}

fn text(value: &str) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Text,
        data: ConstData::Text(value.to_string()),
    }
}

fn boolean(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

fn p(entity: EntityId) -> ValueRef {
    ValueRef::Parameter(entity)
}

fn r(operation: EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation,
        result_index: 0,
    })
}

fn oper(
    entity: EntityId,
    block: EntityId,
    ordinal: u32,
    opcode: Opcode,
    operands: Vec<ValueRef>,
    result: TypeExpr,
    immediate: Immediate,
) -> Operation {
    Operation {
        entity_id: entity,
        block,
        ordinal,
        opcode,
        operands,
        result_types: vec![result],
        immediate,
    }
}

fn blk(
    entity: EntityId,
    function: EntityId,
    operations: Vec<EntityId>,
    terminator: Terminator,
    parameters: Vec<EntityId>,
) -> Block {
    Block {
        entity_id: entity,
        function,
        parameters,
        operations,
        terminator,
        reachability: Reachability::Required,
    }
}

fn fparam(entity: EntityId, owner: EntityId, ordinal: u32, value_type: TypeExpr) -> Parameter {
    Parameter {
        entity_id: entity,
        owner,
        role: ParameterRole::Function,
        ordinal,
        value_type,
    }
}

fn bparam(entity: EntityId, owner: EntityId, ordinal: u32, value_type: TypeExpr) -> Parameter {
    Parameter {
        entity_id: entity,
        owner,
        role: ParameterRole::Block,
        ordinal,
        value_type,
    }
}

fn fun(
    entity: EntityId,
    parameters: Vec<EntityId>,
    result_type: TypeExpr,
    entry_block: EntityId,
    blocks: Vec<EntityId>,
) -> FunctionGraph {
    FunctionGraph {
        entity_id: entity,
        type_parameters: Vec::new(),
        parameters,
        result_type,
        effects: Vec::new(),
        entry_block,
        blocks,
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn ret(value: ValueRef) -> Terminator {
    Terminator::Return(ReturnTerminator { value })
}

fn br(target: EntityId, arguments: Vec<ValueRef>) -> Terminator {
    Terminator::Branch(BranchTerminator {
        edge: TargetEdge { target, arguments },
    })
}

fn cbr(
    condition: ValueRef,
    if_true: EntityId,
    true_args: Vec<ValueRef>,
    if_false: EntityId,
    false_args: Vec<ValueRef>,
) -> Terminator {
    Terminator::CondBranch(sley_ssmc::CondBranchTerminator {
        condition,
        if_true: TargetEdge {
            target: if_true,
            arguments: true_args,
        },
        if_false: TargetEdge {
            target: if_false,
            arguments: false_args,
        },
    })
}

fn switch(
    value: ValueRef,
    cases: Vec<(CaseKey, EntityId, Vec<sley_ssmc::SwitchArgument>)>,
) -> Terminator {
    Terminator::VariantSwitch(sley_ssmc::VariantSwitchTerminator {
        value,
        cases: cases
            .into_iter()
            .map(|(case_key, target, arguments)| SwitchCase {
                case_key,
                edge: SwitchEdge { target, arguments },
            })
            .collect(),
    })
}

fn trap(code: TrapCode) -> Terminator {
    Terminator::Trap(TrapTerminator {
        code,
        payload: None,
    })
}

fn ok_case(
    target: EntityId,
    arguments: Vec<SwitchArgument>,
) -> (CaseKey, EntityId, Vec<SwitchArgument>) {
    (CaseKey::Builtin(BuiltinCase::Ok), target, arguments)
}

fn err_case(
    target: EntityId,
    arguments: Vec<SwitchArgument>,
) -> (CaseKey, EntityId, Vec<SwitchArgument>) {
    (CaseKey::Builtin(BuiltinCase::Err), target, arguments)
}

fn some_case(
    target: EntityId,
    arguments: Vec<SwitchArgument>,
) -> (CaseKey, EntityId, Vec<SwitchArgument>) {
    (CaseKey::Builtin(BuiltinCase::Some), target, arguments)
}

fn none_case(
    target: EntityId,
    arguments: Vec<SwitchArgument>,
) -> (CaseKey, EntityId, Vec<SwitchArgument>) {
    (CaseKey::Builtin(BuiltinCase::None), target, arguments)
}

fn const_def(entity: EntityId, value: ConstValue) -> ConstantDefinition {
    ConstantDefinition {
        entity_id: entity,
        value,
    }
}

/// One closure workload: an owned program plus its vector cases.
struct Workload {
    program: Program,
    cases: Vec<Case>,
}

/// Owned inventories for one program (each workload is self-contained).
struct Program {
    types: TypeEnvironment,
    entry: FunctionGraph,
    functions: Vec<FunctionGraph>,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    adapters: Vec<AdapterImport>,
    constants: Vec<ConstantDefinition>,
}

/// One emitted vector: inputs plus the exact expected termination.
struct Case {
    id: &'static str,
    subject: u32,
    inputs: Vec<ConstValue>,
    expect: Expect,
}

#[derive(Clone)]
enum Expect {
    Success(ConstValue),
}

impl Program {
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
            constants: &self.constants,
            globals: &[],
            functions: &self.functions,
            contracts: &[],
            adapters: &self.adapters,
        }
    }

    fn gate_input(&self) -> BootstrapProfileInput<'_> {
        BootstrapProfileInput {
            types: &self.types,
            schema_epoch: SchemaEpochId::from_bytes([8; 32]),
            entry: &self.entry,
            presented_image_bytes: &[],
            functions: &self.functions,
            parameters: &self.parameters,
            blocks: &self.blocks,
            operations: &self.operations,
            adapters: &self.adapters,
            constants: &self.constants,
            // Frozen v1 workloads: no workload names the successor
            // raw-hash row, so every closure judges under v1.
            profile_version: BootstrapProfileVersion::V1,
        }
    }

    fn run(&self, inputs: &[ConstValue]) -> ExecutionOutcome {
        execute_function(
            self.lowering_input(),
            ExecutionRequest {
                inputs: inputs.to_vec(),
                limits: CLOSURE_LIMITS,
            },
        )
        .expect("closure workload executes")
    }
}

/// Bytes decode then re-encode in one execution: B2V1, `VariantSwitch` over
/// the answered `Result`, V2B1 over the payload, switch again, return.
fn bytes_round_trip() -> Workload {
    let function = id(1);
    let entry = id(2);
    let ok = id(3);
    let err = id(4);
    let ok2 = id(5);
    let scope = id(10);
    let request = id(11);
    let payload = id(12);
    let failure = id(13);
    let payload2 = id(14);
    let decode = id(100);
    let encode = id(101);
    let wrap = id(102);
    let wrap2 = id(103);
    let no_payload = TypeExpr::Bytes;
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![scope, request],
            bridge_result(no_payload.clone()),
            entry,
            vec![entry, ok, err, ok2],
        ),
        functions: vec![fun(
            function,
            vec![scope, request],
            bridge_result(TypeExpr::Bytes),
            entry,
            vec![entry, ok, err, ok2],
        )],
        parameters: vec![
            fparam(scope, function, 0, TypeExpr::Unit),
            fparam(request, function, 1, TypeExpr::Bytes),
            bparam(payload, ok, 0, u8vec_type()),
            bparam(failure, err, 0, index_error()),
            bparam(payload2, ok2, 0, TypeExpr::Bytes),
        ],
        blocks: vec![
            blk(
                entry,
                function,
                vec![decode],
                switch(
                    r(decode),
                    vec![
                        ok_case(ok, vec![SwitchArgument::CasePayload]),
                        err_case(err, vec![SwitchArgument::CasePayload]),
                    ],
                ),
                Vec::new(),
            ),
            blk(
                ok,
                function,
                vec![encode],
                switch(
                    r(encode),
                    vec![
                        ok_case(ok2, vec![SwitchArgument::CasePayload]),
                        err_case(err, vec![SwitchArgument::CasePayload]),
                    ],
                ),
                vec![payload],
            ),
            blk(err, function, vec![wrap], ret(r(wrap)), vec![failure]),
            blk(ok2, function, vec![wrap2], ret(r(wrap2)), vec![payload2]),
        ],
        operations: vec![
            oper(
                decode,
                entry,
                0,
                Opcode::AdapterInvoke,
                vec![p(scope), p(request)],
                bridge_result(u8vec_type()),
                Immediate::Entity(crate::extended::bridge_entry_id(*b"B2V1")),
            ),
            oper(
                encode,
                ok,
                0,
                Opcode::AdapterInvoke,
                vec![p(scope), p(payload)],
                bridge_result(TypeExpr::Bytes),
                Immediate::Entity(crate::extended::bridge_entry_id(*b"V2B1")),
            ),
            oper(
                wrap,
                err,
                0,
                Opcode::ResultErr,
                vec![p(failure)],
                bridge_result(TypeExpr::Bytes),
                Immediate::None,
            ),
            oper(
                wrap2,
                ok2,
                0,
                Opcode::ResultOk,
                vec![p(payload2)],
                bridge_result(TypeExpr::Bytes),
                Immediate::None,
            ),
        ],
        adapters: bridge_subset(&[*b"B2V1", *b"V2B1"]),
        constants: Vec::new(),
    };
    Workload {
        program,
        cases: vec![Case {
            id: "bytes-round-trip",
            subject: 161,
            inputs: vec![
                unit_value(),
                ConstValue {
                    value_type: TypeExpr::Bytes,
                    data: ConstData::Bytes(vec![1, 2, 3]),
                },
            ],
            expect: Expect::Success(ConstValue {
                value_type: bridge_result(TypeExpr::Bytes),
                data: ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
                    value_type: TypeExpr::Bytes,
                    data: ConstData::Bytes(vec![1, 2, 3]),
                }))),
            }),
        }],
    }
}

/// Vector growth under bounded block-parameter iteration (Form A): the
/// entry seeds an empty vector, the loop compares its length against the
/// bound, the body pushes one element and branches back with the grown
/// vector. The push-`Err` arm is statically unreachable for the frozen
/// cases and traps with `Unreachable`.
fn vector_push_loop() -> Workload {
    let function = id(1);
    let entry = id(2);
    let again = id(3);
    let body = id(4);
    let done = id(5);
    let lost = id(6);
    let bound = id(10);
    let element = id(11);
    let current = id(12);
    let held = id(13);
    let finished = id(14);
    let seed = id(100);
    let length = id(101);
    let compare = id(102);
    let push = id(103);
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![bound, element],
            u8vec_type(),
            entry,
            vec![entry, again, body, done, lost],
        ),
        functions: vec![fun(
            function,
            vec![bound, element],
            u8vec_type(),
            entry,
            vec![entry, again, body, done, lost],
        )],
        parameters: vec![
            fparam(bound, function, 0, u64_type()),
            fparam(element, function, 1, u8_type()),
            bparam(current, again, 0, u8vec_type()),
            bparam(held, body, 0, u8vec_type()),
            bparam(finished, done, 0, u8vec_type()),
        ],
        blocks: vec![
            blk(
                entry,
                function,
                vec![seed],
                br(again, vec![r(seed)]),
                Vec::new(),
            ),
            blk(
                again,
                function,
                vec![length, compare],
                cbr(r(compare), body, vec![p(current)], done, vec![p(current)]),
                vec![current],
            ),
            blk(
                body,
                function,
                vec![push],
                switch(
                    r(push),
                    vec![
                        ok_case(again, vec![SwitchArgument::CasePayload]),
                        err_case(lost, Vec::new()),
                    ],
                ),
                vec![held],
            ),
            blk(done, function, Vec::new(), ret(p(finished)), vec![finished]),
            blk(
                lost,
                function,
                Vec::new(),
                trap(TrapCode::Unreachable),
                Vec::new(),
            ),
        ],
        operations: vec![
            oper(
                seed,
                entry,
                0,
                Opcode::VectorNew,
                Vec::new(),
                u8vec_type(),
                Immediate::None,
            ),
            oper(
                length,
                again,
                0,
                Opcode::VectorLen,
                vec![p(current)],
                u64_type(),
                Immediate::None,
            ),
            oper(
                compare,
                again,
                1,
                Opcode::LessThan,
                vec![r(length), p(bound)],
                TypeExpr::Bool,
                Immediate::None,
            ),
            oper(
                push,
                body,
                0,
                Opcode::AdapterInvoke,
                vec![p(held), p(element)],
                bridge_result(u8vec_type()),
                Immediate::Entity(crate::extended::bridge_entry_id(*b"PSH1")),
            ),
        ],
        adapters: bridge_subset(&[*b"PSH1"]),
        constants: Vec::new(),
    };
    Workload {
        program,
        cases: vec![
            Case {
                id: "vector-push-loop-n5",
                subject: 161,
                inputs: vec![uint(5), octet(7)],
                expect: Expect::Success(ConstValue {
                    value_type: u8vec_type(),
                    data: ConstData::Sequence(vec![
                        octet(7),
                        octet(7),
                        octet(7),
                        octet(7),
                        octet(7),
                    ]),
                }),
            },
            Case {
                id: "vector-push-loop-n0",
                subject: 161,
                inputs: vec![uint(0), octet(7)],
                expect: Expect::Success(ConstValue {
                    value_type: u8vec_type(),
                    data: ConstData::Sequence(Vec::new()),
                }),
            },
        ],
    }
}

/// Symbol-table lookup over text keys: build a two-entry map, probe for
/// the query key, and return the bound value or the query itself. The
/// miss arm returns the query so every arm yields `Text`.
fn map_symbol_table() -> Workload {
    let text_map = || TypeExpr::OrderedMap {
        key: Box::new(TypeExpr::Text),
        value: Box::new(TypeExpr::Text),
    };
    let map_result = || TypeExpr::Result {
        ok: Box::new(text_map()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let function = id(1);
    let entry = id(2);
    let probe = id(3);
    let hit = id(4);
    let answered = id(5);
    let key_a = id(10);
    let key_b = id(11);
    let value_a = id(12);
    let value_b = id(13);
    let query = id(14);
    let table = id(15);
    let present = id(16);
    let found = id(17);
    let held = id(18);
    let build = id(100);
    let contains = id(101);
    let get = id(102);
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![key_a, key_b, value_a, value_b, query],
            TypeExpr::Text,
            entry,
            vec![entry, probe, hit, answered],
        ),
        functions: vec![fun(
            function,
            vec![key_a, key_b, value_a, value_b, query],
            TypeExpr::Text,
            entry,
            vec![entry, probe, hit, answered],
        )],
        parameters: vec![
            fparam(key_a, function, 0, TypeExpr::Text),
            fparam(key_b, function, 1, TypeExpr::Text),
            fparam(value_a, function, 2, TypeExpr::Text),
            fparam(value_b, function, 3, TypeExpr::Text),
            fparam(query, function, 4, TypeExpr::Text),
            bparam(table, probe, 0, text_map()),
            bparam(held, hit, 0, text_map()),
            bparam(present, hit, 1, TypeExpr::Text),
            bparam(found, answered, 0, TypeExpr::Text),
        ],
        blocks: vec![
            blk(
                entry,
                function,
                vec![build],
                switch(
                    r(build),
                    vec![
                        ok_case(probe, vec![SwitchArgument::CasePayload]),
                        err_case(answered, vec![SwitchArgument::Value(p(query))]),
                    ],
                ),
                Vec::new(),
            ),
            blk(
                probe,
                function,
                vec![contains],
                cbr(
                    r(contains),
                    hit,
                    vec![p(table), p(query)],
                    answered,
                    vec![p(query)],
                ),
                vec![table],
            ),
            blk(
                hit,
                function,
                vec![get],
                switch(
                    r(get),
                    vec![
                        none_case(answered, vec![SwitchArgument::Value(p(present))]),
                        some_case(answered, vec![SwitchArgument::CasePayload]),
                    ],
                ),
                vec![held, present],
            ),
            blk(answered, function, Vec::new(), ret(p(found)), vec![found]),
        ],
        operations: vec![
            oper(
                build,
                entry,
                0,
                Opcode::MapNew,
                vec![p(key_a), p(value_a), p(key_b), p(value_b)],
                map_result(),
                Immediate::None,
            ),
            oper(
                contains,
                probe,
                0,
                Opcode::MapContains,
                vec![p(table), p(query)],
                TypeExpr::Bool,
                Immediate::None,
            ),
            oper(
                get,
                hit,
                0,
                Opcode::MapGet,
                vec![p(held), p(present)],
                TypeExpr::Option(Box::new(TypeExpr::Text)),
                Immediate::None,
            ),
        ],
        adapters: Vec::new(),
        constants: Vec::new(),
    };
    Workload {
        program,
        cases: vec![
            Case {
                id: "map-symbol-table-hit",
                subject: 36,
                inputs: vec![text("a"), text("b"), text("1"), text("2"), text("b")],
                expect: Expect::Success(text("2")),
            },
            Case {
                id: "map-symbol-table-miss",
                subject: 36,
                inputs: vec![text("a"), text("b"), text("1"), text("2"), text("z")],
                expect: Expect::Success(text("z")),
            },
        ],
    }
}

/// Deterministic sets as maps-to-`Unit`: insert a key, remove it again,
/// and report absence. The returned `false` is the assertion — a set the
/// toolchain uses for visited-marking and closure membership.
fn set_as_map() -> Workload {
    let unit_set = || TypeExpr::OrderedMap {
        key: Box::new(u64_type()),
        value: Box::new(TypeExpr::Unit),
    };
    let set_result = || TypeExpr::Result {
        ok: Box::new(unit_set()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let function = id(1);
    let entry = id(2);
    let work = id(3);
    let lost = id(4);
    let key = id(10);
    let table = id(11);
    let one = id(12);
    let build = id(100);
    let insert = id(101);
    let remove = id(102);
    let check = id(103);
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![key, one],
            TypeExpr::Bool,
            entry,
            vec![entry, work, lost],
        ),
        functions: vec![fun(
            function,
            vec![key, one],
            TypeExpr::Bool,
            entry,
            vec![entry, work, lost],
        )],
        parameters: vec![
            fparam(key, function, 0, u64_type()),
            fparam(one, function, 1, TypeExpr::Unit),
            bparam(table, work, 0, unit_set()),
        ],
        blocks: vec![
            blk(
                entry,
                function,
                vec![build],
                switch(
                    r(build),
                    vec![
                        ok_case(work, vec![SwitchArgument::CasePayload]),
                        err_case(lost, Vec::new()),
                    ],
                ),
                Vec::new(),
            ),
            blk(
                work,
                function,
                vec![insert, remove, check],
                ret(r(check)),
                vec![table],
            ),
            blk(
                lost,
                function,
                Vec::new(),
                trap(TrapCode::Unreachable),
                Vec::new(),
            ),
        ],
        operations: vec![
            oper(
                build,
                entry,
                0,
                Opcode::MapNew,
                Vec::new(),
                set_result(),
                Immediate::None,
            ),
            oper(
                insert,
                work,
                0,
                Opcode::MapInsert,
                vec![p(table), p(key), p(one)],
                unit_set(),
                Immediate::None,
            ),
            oper(
                remove,
                work,
                1,
                Opcode::MapRemove,
                vec![r(insert), p(key)],
                unit_set(),
                Immediate::None,
            ),
            oper(
                check,
                work,
                2,
                Opcode::MapContains,
                vec![r(remove), p(key)],
                TypeExpr::Bool,
                Immediate::None,
            ),
        ],
        adapters: Vec::new(),
        constants: Vec::new(),
    };
    Workload {
        program,
        cases: vec![Case {
            id: "set-as-map-absent-after-remove",
            subject: 39,
            inputs: vec![uint(7), unit_value()],
            expect: Expect::Success(boolean(false)),
        }],
    }
}

/// Record/variant construction and inspection: build a pair record, read
/// a field, wrap it in a variant case, and deconstruct through
/// `VariantSwitch`. The toolchain's program-graph values are records and
/// variants; this is their executable shape.
fn record_variant_walk() -> Workload {
    let pair = id(60);
    let wrap = id(61);
    let pair_type = || {
        TypeExpr::Named(NamedType {
            definition: pair,
            arguments: Vec::new(),
        })
    };
    let wrap_type = || {
        TypeExpr::Named(NamedType {
            definition: wrap,
            arguments: Vec::new(),
        })
    };
    let definitions = vec![
        TypeDefinition {
            entity_id: pair,
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
            entity_id: wrap,
            type_parameters: Vec::new(),
            form: TypeDefForm::Variant(vec![
                VariantCase {
                    member_id: member(0xC1),
                    payload_type: Some(TypeExpr::Text),
                },
                VariantCase {
                    member_id: member(0xC2),
                    payload_type: None,
                },
            ]),
            invariants: Vec::new(),
            visibility: Visibility::Private,
        },
    ];
    let function = id(1);
    let entry = id(2);
    let answered = id(3);
    let number = id(10);
    let label = id(11);
    let shown = id(12);
    let make = id(100);
    let field = id(101);
    let pack = id(102);
    let open = id(103);
    let program = Program {
        types: TypeEnvironment::new(definitions).unwrap(),
        entry: fun(
            function,
            vec![number, label],
            TypeExpr::Text,
            entry,
            vec![entry, answered],
        ),
        functions: vec![fun(
            function,
            vec![number, label],
            TypeExpr::Text,
            entry,
            vec![entry, answered],
        )],
        parameters: vec![
            fparam(number, function, 0, u64_type()),
            fparam(label, function, 1, TypeExpr::Text),
            bparam(shown, answered, 0, TypeExpr::Text),
        ],
        blocks: vec![
            blk(
                entry,
                function,
                vec![make, field, pack, open],
                switch(
                    r(open),
                    vec![
                        none_case(answered, vec![SwitchArgument::Value(p(label))]),
                        some_case(answered, vec![SwitchArgument::CasePayload]),
                    ],
                ),
                Vec::new(),
            ),
            blk(answered, function, Vec::new(), ret(p(shown)), vec![shown]),
        ],
        operations: vec![
            oper(
                make,
                entry,
                0,
                Opcode::RecordNew,
                vec![p(number), p(label)],
                pair_type(),
                Immediate::Entity(pair),
            ),
            oper(
                field,
                entry,
                1,
                Opcode::RecordGet,
                vec![r(make)],
                TypeExpr::Text,
                Immediate::Field(member(0xB2)),
            ),
            oper(
                pack,
                entry,
                2,
                Opcode::VariantNew,
                vec![r(field)],
                wrap_type(),
                Immediate::Variant(VariantImmediate {
                    definition: wrap,
                    member_id: member(0xC1),
                }),
            ),
            oper(
                open,
                entry,
                3,
                Opcode::VariantGet,
                vec![r(pack)],
                TypeExpr::Option(Box::new(TypeExpr::Text)),
                Immediate::Variant(VariantImmediate {
                    definition: wrap,
                    member_id: member(0xC1),
                }),
            ),
        ],
        adapters: Vec::new(),
        constants: Vec::new(),
    };
    Workload {
        program,
        cases: vec![Case {
            id: "record-variant-walk",
            subject: 18,
            inputs: vec![uint(4), text("four")],
            expect: Expect::Success(text("four")),
        }],
    }
}

/// Multi-function checked arithmetic with error propagation: the entry
/// calls a callee performing `IntAddChecked`, unwraps the answered
/// `Result` through a switch, and returns the sum or the default. The two
/// cases pin value and failure propagation across the call boundary.
fn multi_function_pass() -> Workload {
    let arith_result = || TypeExpr::Result {
        ok: Box::new(u64_type()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    };
    let entry = id(1);
    let callee = id(6);
    let entry_block = id(2);
    let callee_block = id(7);
    let ok = id(3);
    let err = id(4);
    let left = id(10);
    let right = id(11);
    let fallback = id(12);
    let total = id(13);
    let problem = id(14);
    // Callee parameters live in the same flat inventory with distinct
    // identities (the lowering inventories never share entity ids across
    // functions).
    let sub_left = id(20);
    let sub_right = id(21);
    let call = id(100);
    let add = id(101);
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            entry,
            vec![left, right, fallback],
            u64_type(),
            entry_block,
            vec![entry_block, ok, err],
        ),
        functions: vec![
            fun(
                entry,
                vec![left, right, fallback],
                u64_type(),
                entry_block,
                vec![entry_block, ok, err],
            ),
            fun(
                callee,
                vec![sub_left, sub_right],
                arith_result(),
                callee_block,
                vec![callee_block],
            ),
        ],
        parameters: vec![
            fparam(left, entry, 0, u64_type()),
            fparam(right, entry, 1, u64_type()),
            fparam(fallback, entry, 2, u64_type()),
            fparam(sub_left, callee, 0, u64_type()),
            fparam(sub_right, callee, 1, u64_type()),
            bparam(total, ok, 0, u64_type()),
            bparam(
                problem,
                err,
                0,
                TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic),
            ),
        ],
        blocks: vec![
            blk(
                entry_block,
                entry,
                vec![call],
                switch(
                    r(call),
                    vec![
                        ok_case(ok, vec![SwitchArgument::CasePayload]),
                        err_case(err, vec![SwitchArgument::CasePayload]),
                    ],
                ),
                Vec::new(),
            ),
            blk(ok, entry, Vec::new(), ret(p(total)), vec![total]),
            blk(err, entry, Vec::new(), ret(p(fallback)), vec![problem]),
            blk(callee_block, callee, vec![add], ret(r(add)), Vec::new()),
        ],
        operations: vec![
            oper(
                call,
                entry_block,
                0,
                Opcode::CallDirect,
                vec![p(left), p(right)],
                arith_result(),
                Immediate::Function(sley_ssmc::FunctionRefValue {
                    function: callee,
                    type_arguments: Vec::new(),
                }),
            ),
            oper(
                add,
                callee_block,
                0,
                Opcode::IntAddChecked,
                vec![p(sub_left), p(sub_right)],
                arith_result(),
                Immediate::None,
            ),
        ],
        adapters: Vec::new(),
        constants: Vec::new(),
    };
    Workload {
        program,
        cases: vec![
            Case {
                id: "multi-function-pass-ok",
                subject: 112,
                inputs: vec![uint(10), uint(20), uint(0)],
                expect: Expect::Success(uint(30)),
            },
            Case {
                id: "multi-function-pass-overflow",
                subject: 112,
                inputs: vec![uint(u128::from(u64::MAX)), uint(1), uint(99)],
                expect: Expect::Success(uint(99)),
            },
        ],
    }
}

/// Bounds-checked traversal: index into a vector, return the element on
/// `Some` and the default on `None`. No unwrap operation exists by design;
/// the switch is the deconstruction form.
fn checked_length_traverse() -> Workload {
    let indexed = || TypeExpr::Vector(Box::new(u64_type()));
    let function = id(1);
    let entry = id(2);
    let answered = id(3);
    let vector = id(10);
    let index = id(11);
    let default = id(12);
    let element = id(13);
    let get = id(100);
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![vector, index, default],
            u64_type(),
            entry,
            vec![entry, answered],
        ),
        functions: vec![fun(
            function,
            vec![vector, index, default],
            u64_type(),
            entry,
            vec![entry, answered],
        )],
        parameters: vec![
            fparam(vector, function, 0, indexed()),
            fparam(index, function, 1, u64_type()),
            fparam(default, function, 2, u64_type()),
            bparam(element, answered, 0, u64_type()),
        ],
        blocks: vec![
            blk(
                entry,
                function,
                vec![get],
                switch(
                    r(get),
                    vec![
                        none_case(answered, vec![SwitchArgument::Value(p(default))]),
                        some_case(answered, vec![SwitchArgument::CasePayload]),
                    ],
                ),
                Vec::new(),
            ),
            blk(
                answered,
                function,
                Vec::new(),
                ret(p(element)),
                vec![element],
            ),
        ],
        operations: vec![oper(
            get,
            entry,
            0,
            Opcode::VectorGet,
            vec![p(vector), p(index)],
            TypeExpr::Option(Box::new(u64_type())),
            Immediate::None,
        )],
        adapters: Vec::new(),
        constants: Vec::new(),
    };
    let sequence = || ConstValue {
        value_type: indexed(),
        data: ConstData::Sequence(vec![uint(10), uint(20), uint(30)]),
    };
    Workload {
        program,
        cases: vec![
            Case {
                id: "checked-length-traverse-hit",
                subject: 34,
                inputs: vec![sequence(), uint(1), uint(0)],
                expect: Expect::Success(uint(20)),
            },
            Case {
                id: "checked-length-traverse-miss",
                subject: 34,
                inputs: vec![sequence(), uint(9), uint(0)],
                expect: Expect::Success(uint(0)),
            },
        ],
    }
}

/// Content addressing at the native boundary: hash a constructed tuple
/// twice and compare. The `true` answer pins same-input determinism of
/// `value_hash` under the frozen schema epoch.
fn value_hash_chain() -> Workload {
    let pair = || TypeExpr::Tuple(vec![u64_type(), TypeExpr::Text]);
    let function = id(1);
    let entry = id(2);
    let number = id(10);
    let label = id(11);
    let build = id(100);
    let first = id(101);
    let second = id(102);
    let compare = id(103);
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![number, label],
            TypeExpr::Bool,
            entry,
            vec![entry],
        ),
        functions: vec![fun(
            function,
            vec![number, label],
            TypeExpr::Bool,
            entry,
            vec![entry],
        )],
        parameters: vec![
            fparam(number, function, 0, u64_type()),
            fparam(label, function, 1, TypeExpr::Text),
        ],
        blocks: vec![blk(
            entry,
            function,
            vec![build, first, second, compare],
            ret(r(compare)),
            Vec::new(),
        )],
        operations: vec![
            oper(
                build,
                entry,
                0,
                Opcode::TupleNew,
                vec![p(number), p(label)],
                pair(),
                Immediate::None,
            ),
            oper(
                first,
                entry,
                1,
                Opcode::ValueHash,
                vec![r(build)],
                TypeExpr::Bytes,
                Immediate::None,
            ),
            oper(
                second,
                entry,
                2,
                Opcode::ValueHash,
                vec![r(build)],
                TypeExpr::Bytes,
                Immediate::None,
            ),
            oper(
                compare,
                entry,
                3,
                Opcode::Equal,
                vec![r(first), r(second)],
                TypeExpr::Bool,
                Immediate::None,
            ),
        ],
        adapters: Vec::new(),
        constants: Vec::new(),
    };
    Workload {
        program,
        cases: vec![Case {
            id: "value-hash-chain",
            subject: 192,
            inputs: vec![uint(7), text("seven")],
            expect: Expect::Success(boolean(true)),
        }],
    }
}

/// Exhaustive two-level switch with payload threading: an `Option` arm
/// carries its payload into a mid block that switches a `Result`, and the
/// `Ok` edge carries both the outer payload and the inner payload into
/// the comparison block. The four cases pin the full matrix, closing the
/// RW-040 recorded limitation with accepted vectors.
fn variant_switch_exhaustive() -> Workload {
    let maybe = || TypeExpr::Option(Box::new(u8_type()));
    let fallible = || TypeExpr::Result {
        ok: Box::new(u8_type()),
        error: Box::new(TypeExpr::Unit),
    };
    let function = id(1);
    let entry = id(2);
    let middle = id(3);
    let equal = id(4);
    let negative = id(5);
    let first = id(10);
    let second = id(11);
    let outer = id(12);
    let left = id(13);
    let right = id(14);
    let compare = id(100);
    let deny = id(101);
    let no = id(200);
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![first, second],
            TypeExpr::Bool,
            entry,
            vec![entry, middle, equal, negative],
        ),
        functions: vec![fun(
            function,
            vec![first, second],
            TypeExpr::Bool,
            entry,
            vec![entry, middle, equal, negative],
        )],
        parameters: vec![
            fparam(first, function, 0, maybe()),
            fparam(second, function, 1, fallible()),
            bparam(outer, middle, 0, u8_type()),
            bparam(left, equal, 0, u8_type()),
            bparam(right, equal, 1, u8_type()),
        ],
        blocks: vec![
            blk(
                entry,
                function,
                Vec::new(),
                switch(
                    p(first),
                    vec![
                        none_case(negative, Vec::new()),
                        some_case(middle, vec![SwitchArgument::CasePayload]),
                    ],
                ),
                Vec::new(),
            ),
            blk(
                middle,
                function,
                Vec::new(),
                switch(
                    p(second),
                    vec![
                        ok_case(
                            equal,
                            vec![SwitchArgument::Value(p(outer)), SwitchArgument::CasePayload],
                        ),
                        err_case(negative, Vec::new()),
                    ],
                ),
                vec![outer],
            ),
            blk(
                equal,
                function,
                vec![compare],
                ret(r(compare)),
                vec![left, right],
            ),
            blk(negative, function, vec![deny], ret(r(deny)), Vec::new()),
        ],
        operations: vec![
            oper(
                compare,
                equal,
                0,
                Opcode::Equal,
                vec![p(left), p(right)],
                TypeExpr::Bool,
                Immediate::None,
            ),
            oper(
                deny,
                negative,
                0,
                Opcode::ConstantRef,
                Vec::new(),
                TypeExpr::Bool,
                Immediate::Entity(no),
            ),
        ],
        adapters: Vec::new(),
        constants: vec![const_def(no, boolean(false))],
    };
    let some = |value: u8| ConstValue {
        value_type: maybe(),
        data: ConstData::Option(Some(Box::new(octet(value)))),
    };
    let none = || ConstValue {
        value_type: maybe(),
        data: ConstData::Option(None),
    };
    let ok = |value: u8| ConstValue {
        value_type: fallible(),
        data: ConstData::Result(ResultConst::Ok(Box::new(octet(value)))),
    };
    let err = || ConstValue {
        value_type: fallible(),
        data: ConstData::Result(ResultConst::Err(Box::new(unit_value()))),
    };
    Workload {
        program,
        cases: vec![
            Case {
                id: "variant-switch-exhaustive-some-ok-equal",
                subject: 96,
                inputs: vec![some(7), ok(7)],
                expect: Expect::Success(boolean(true)),
            },
            Case {
                id: "variant-switch-exhaustive-some-ok-unequal",
                subject: 96,
                inputs: vec![some(7), ok(8)],
                expect: Expect::Success(boolean(false)),
            },
            Case {
                id: "variant-switch-exhaustive-none",
                subject: 96,
                inputs: vec![none(), ok(7)],
                expect: Expect::Success(boolean(false)),
            },
            Case {
                id: "variant-switch-exhaustive-err",
                subject: 96,
                inputs: vec![some(7), err()],
                expect: Expect::Success(boolean(false)),
            },
        ],
    }
}

/// Iterative graph worklist over an adjacency map: the loop carries the
/// table, the current node, the visited set-as-map, and a unit trail whose
/// length counts visits. Visited nodes exit with the count; missing
/// successors exit with the count; fresh nodes extend the visited set and
/// the trail (pushed through a `Unit`-monomorphized bridge row) and branch
/// back. No recursion, no arithmetic. This is the toolchain's
/// graph-traversal shape (10.2 items 2, 3, 5: checking passes, Witness
/// analysis, build-driver closure).
///
/// Block parameters are distinct entities per block: values thread through
/// edges positionally, never by shared identity.
fn graph_worklist_dfs(
    name: &'static str,
    links: [(u128, u128); 3],
    start: u128,
    expect_visits: u128,
) -> Workload {
    let node_map = || TypeExpr::OrderedMap {
        key: Box::new(u64_type()),
        value: Box::new(u64_type()),
    };
    let map_result = || TypeExpr::Result {
        ok: Box::new(node_map()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let trail_type = || TypeExpr::Vector(Box::new(TypeExpr::Unit));
    let unit_set = || TypeExpr::OrderedMap {
        key: Box::new(u64_type()),
        value: Box::new(TypeExpr::Unit),
    };
    let visited_result = || TypeExpr::Result {
        ok: Box::new(unit_set()),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
    };
    let unit_const = id(20);
    let link_const = |index: u8| id(30 + index);
    let function = id(1);
    let entry = id(2);
    let again = id(3);
    let fetch = id(4);
    let visit = id(5);
    let done = id(6);
    let lost = id(7);
    let push_lost = id(8);
    let middle = id(9);
    let origin = id(10);
    let sink_trail = id(27);
    let sink = id(28);
    let sink_unit = id(129);
    let sink_grow = id(130);
    let table_a = id(40);
    let current_a = id(41);
    let marks_a = id(42);
    let trail_a = id(43);
    let table_f = id(44);
    let current_f = id(45);
    let marks_f = id(46);
    let trail_f = id(47);
    let table_v = id(48);
    let marks_v = id(50);
    let trail_v = id(51);
    let next_v = id(52);
    let walked = id(53);
    let middle_map = id(54);
    let middle_marks = id(55);
    let middle_trail = id(56);
    let build = id(100);
    let empty_marks = id(101);
    let empty_trail = id(102);
    let link_ref = |index: u8| id(110 + index);
    let known = id(120);
    let probe = id(121);
    let mark_unit = id(122);
    let mark = id(123);
    let grow = id(124);
    let count = id(125);
    let push_unit = id(126);
    let push_row = crate::extended::bridge_import(
        crate::extended::BridgeEntry::VectorPush,
        TypeExpr::Unit,
        trail_type(),
    );
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![origin],
            u64_type(),
            entry,
            vec![
                entry, middle, again, fetch, visit, done, lost, push_lost, sink,
            ],
        ),
        functions: vec![fun(
            function,
            vec![origin],
            u64_type(),
            entry,
            vec![
                entry, middle, again, fetch, visit, done, lost, push_lost, sink,
            ],
        )],
        parameters: vec![
            fparam(origin, function, 0, u64_type()),
            bparam(middle_map, middle, 0, node_map()),
            bparam(middle_marks, middle, 1, visited_result()),
            bparam(middle_trail, middle, 2, trail_type()),
            bparam(table_a, again, 0, node_map()),
            bparam(current_a, again, 1, u64_type()),
            bparam(marks_a, again, 2, unit_set()),
            bparam(trail_a, again, 3, trail_type()),
            bparam(table_f, fetch, 0, node_map()),
            bparam(current_f, fetch, 1, u64_type()),
            bparam(marks_f, fetch, 2, unit_set()),
            bparam(trail_f, fetch, 3, trail_type()),
            bparam(table_v, visit, 0, node_map()),
            bparam(marks_v, visit, 1, unit_set()),
            bparam(trail_v, visit, 2, trail_type()),
            bparam(next_v, visit, 3, u64_type()),
            bparam(walked, done, 0, trail_type()),
            bparam(sink_trail, sink, 0, trail_type()),
        ],
        blocks: vec![
            blk(
                entry,
                function,
                vec![
                    link_ref(0),
                    link_ref(1),
                    link_ref(2),
                    link_ref(3),
                    link_ref(4),
                    link_ref(5),
                    build,
                    empty_marks,
                    empty_trail,
                ],
                switch(
                    r(build),
                    vec![
                        ok_case(
                            middle,
                            vec![
                                SwitchArgument::CasePayload,
                                SwitchArgument::Value(r(empty_marks)),
                                SwitchArgument::Value(r(empty_trail)),
                            ],
                        ),
                        err_case(lost, Vec::new()),
                    ],
                ),
                Vec::new(),
            ),
            blk(
                middle,
                function,
                Vec::new(),
                switch(
                    p(middle_marks),
                    vec![
                        ok_case(
                            again,
                            vec![
                                SwitchArgument::Value(p(middle_map)),
                                SwitchArgument::Value(p(origin)),
                                SwitchArgument::CasePayload,
                                SwitchArgument::Value(p(middle_trail)),
                            ],
                        ),
                        err_case(lost, Vec::new()),
                    ],
                ),
                vec![middle_map, middle_marks, middle_trail],
            ),
            blk(
                again,
                function,
                vec![known],
                cbr(
                    r(known),
                    done,
                    vec![p(trail_a)],
                    fetch,
                    vec![p(table_a), p(current_a), p(marks_a), p(trail_a)],
                ),
                vec![table_a, current_a, marks_a, trail_a],
            ),
            blk(
                fetch,
                function,
                vec![mark_unit, mark, probe],
                switch(
                    r(probe),
                    vec![
                        none_case(sink, vec![SwitchArgument::Value(p(trail_f))]),
                        some_case(
                            visit,
                            vec![
                                SwitchArgument::Value(p(table_f)),
                                SwitchArgument::Value(r(mark)),
                                SwitchArgument::Value(p(trail_f)),
                                SwitchArgument::CasePayload,
                            ],
                        ),
                    ],
                ),
                vec![table_f, current_f, marks_f, trail_f],
            ),
            blk(
                visit,
                function,
                vec![push_unit, grow],
                switch(
                    r(grow),
                    vec![
                        ok_case(
                            again,
                            vec![
                                SwitchArgument::Value(p(table_v)),
                                SwitchArgument::Value(p(next_v)),
                                SwitchArgument::Value(p(marks_v)),
                                SwitchArgument::CasePayload,
                            ],
                        ),
                        err_case(push_lost, Vec::new()),
                    ],
                ),
                vec![table_v, marks_v, trail_v, next_v],
            ),
            blk(done, function, vec![count], ret(r(count)), vec![walked]),
            blk(
                sink,
                function,
                vec![sink_unit, sink_grow],
                switch(
                    r(sink_grow),
                    vec![
                        ok_case(done, vec![SwitchArgument::CasePayload]),
                        err_case(push_lost, Vec::new()),
                    ],
                ),
                vec![sink_trail],
            ),
            blk(
                lost,
                function,
                Vec::new(),
                trap(TrapCode::Unreachable),
                Vec::new(),
            ),
            blk(
                push_lost,
                function,
                Vec::new(),
                trap(TrapCode::Unreachable),
                Vec::new(),
            ),
        ],
        operations: vec![
            oper(
                link_ref(0),
                entry,
                0,
                Opcode::ConstantRef,
                Vec::new(),
                u64_type(),
                Immediate::Entity(link_const(0)),
            ),
            oper(
                link_ref(1),
                entry,
                1,
                Opcode::ConstantRef,
                Vec::new(),
                u64_type(),
                Immediate::Entity(link_const(1)),
            ),
            oper(
                link_ref(2),
                entry,
                2,
                Opcode::ConstantRef,
                Vec::new(),
                u64_type(),
                Immediate::Entity(link_const(2)),
            ),
            oper(
                link_ref(3),
                entry,
                3,
                Opcode::ConstantRef,
                Vec::new(),
                u64_type(),
                Immediate::Entity(link_const(3)),
            ),
            oper(
                link_ref(4),
                entry,
                4,
                Opcode::ConstantRef,
                Vec::new(),
                u64_type(),
                Immediate::Entity(link_const(4)),
            ),
            oper(
                link_ref(5),
                entry,
                5,
                Opcode::ConstantRef,
                Vec::new(),
                u64_type(),
                Immediate::Entity(link_const(5)),
            ),
            oper(
                build,
                entry,
                6,
                Opcode::MapNew,
                vec![
                    r(link_ref(0)),
                    r(link_ref(1)),
                    r(link_ref(2)),
                    r(link_ref(3)),
                    r(link_ref(4)),
                    r(link_ref(5)),
                ],
                map_result(),
                Immediate::None,
            ),
            oper(
                empty_marks,
                entry,
                7,
                Opcode::MapNew,
                Vec::new(),
                visited_result(),
                Immediate::None,
            ),
            oper(
                empty_trail,
                entry,
                8,
                Opcode::VectorNew,
                Vec::new(),
                trail_type(),
                Immediate::None,
            ),
            oper(
                known,
                again,
                0,
                Opcode::MapContains,
                vec![p(marks_a), p(current_a)],
                TypeExpr::Bool,
                Immediate::None,
            ),
            oper(
                mark_unit,
                fetch,
                0,
                Opcode::ConstantRef,
                Vec::new(),
                TypeExpr::Unit,
                Immediate::Entity(unit_const),
            ),
            oper(
                mark,
                fetch,
                1,
                Opcode::MapInsert,
                vec![p(marks_f), p(current_f), r(mark_unit)],
                unit_set(),
                Immediate::None,
            ),
            oper(
                probe,
                fetch,
                2,
                Opcode::MapGet,
                vec![p(table_f), p(current_f)],
                TypeExpr::Option(Box::new(u64_type())),
                Immediate::None,
            ),
            oper(
                push_unit,
                visit,
                0,
                Opcode::ConstantRef,
                Vec::new(),
                TypeExpr::Unit,
                Immediate::Entity(unit_const),
            ),
            oper(
                grow,
                visit,
                1,
                Opcode::AdapterInvoke,
                vec![p(trail_v), r(push_unit)],
                bridge_result(trail_type()),
                Immediate::Entity(push_row.entity_id),
            ),
            oper(
                count,
                done,
                0,
                Opcode::VectorLen,
                vec![p(walked)],
                u64_type(),
                Immediate::None,
            ),
            oper(
                sink_unit,
                sink,
                0,
                Opcode::ConstantRef,
                Vec::new(),
                TypeExpr::Unit,
                Immediate::Entity(unit_const),
            ),
            oper(
                sink_grow,
                sink,
                1,
                Opcode::AdapterInvoke,
                vec![p(sink_trail), r(sink_unit)],
                bridge_result(trail_type()),
                Immediate::Entity(push_row.entity_id),
            ),
        ],
        adapters: vec![push_row],
        constants: vec![
            const_def(unit_const, unit_value()),
            const_def(link_const(0), uint(links[0].0)),
            const_def(link_const(1), uint(links[0].1)),
            const_def(link_const(2), uint(links[1].0)),
            const_def(link_const(3), uint(links[1].1)),
            const_def(link_const(4), uint(links[2].0)),
            const_def(link_const(5), uint(links[2].1)),
        ],
    };
    Workload {
        program,
        cases: vec![Case {
            id: name,
            subject: 38,
            inputs: vec![uint(start)],
            expect: Expect::Success(uint(expect_visits)),
        }],
    }
}

/// Image-assembly emission: copy a source octet vector element by element
/// into a destination vector, then cross it back to bytes. The running
/// length of the destination IS the source index, so no arithmetic is
/// needed; a missing element ends the copy (`None` switches to emission).
/// This is the toolchain's output assembly shape (10.2 item 4).
fn image_assemble_emit() -> Workload {
    let function = id(1);
    let entry = id(2);
    let again = id(3);
    let copy = id(4);
    let emit = id(5);
    let answer = id(6);
    let lost = id(7);
    let push_lost = id(8);
    let source = id(10);
    let unit_param = id(11);
    let current = id(12);
    let held = id(13);
    let got = id(14);
    let ready = id(15);
    let shown = id(16);
    let seed = id(100);
    let length = id(101);
    let take = id(102);
    let grow = id(103);
    let cross = id(104);
    let wrap = id(105);
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![source, unit_param],
            bridge_result(TypeExpr::Bytes),
            entry,
            vec![entry, again, copy, emit, answer, lost, push_lost],
        ),
        functions: vec![fun(
            function,
            vec![source, unit_param],
            bridge_result(TypeExpr::Bytes),
            entry,
            vec![entry, again, copy, emit, answer, lost, push_lost],
        )],
        parameters: vec![
            fparam(source, function, 0, u8vec_type()),
            fparam(unit_param, function, 1, TypeExpr::Unit),
            bparam(current, again, 0, u8vec_type()),
            bparam(held, copy, 0, u8vec_type()),
            bparam(got, copy, 1, u8_type()),
            bparam(ready, emit, 0, u8vec_type()),
            bparam(shown, answer, 0, TypeExpr::Bytes),
        ],
        blocks: vec![
            blk(
                entry,
                function,
                vec![seed],
                br(again, vec![r(seed)]),
                Vec::new(),
            ),
            blk(
                again,
                function,
                vec![length, take],
                switch(
                    r(take),
                    vec![
                        none_case(emit, vec![SwitchArgument::Value(p(current))]),
                        some_case(
                            copy,
                            vec![
                                SwitchArgument::Value(p(current)),
                                SwitchArgument::CasePayload,
                            ],
                        ),
                    ],
                ),
                vec![current],
            ),
            blk(
                copy,
                function,
                vec![grow],
                switch(
                    r(grow),
                    vec![
                        ok_case(again, vec![SwitchArgument::CasePayload]),
                        err_case(push_lost, Vec::new()),
                    ],
                ),
                vec![held, got],
            ),
            blk(
                emit,
                function,
                vec![cross],
                switch(
                    r(cross),
                    vec![
                        ok_case(answer, vec![SwitchArgument::CasePayload]),
                        err_case(lost, Vec::new()),
                    ],
                ),
                vec![ready],
            ),
            blk(answer, function, vec![wrap], ret(r(wrap)), vec![shown]),
            blk(
                lost,
                function,
                Vec::new(),
                trap(TrapCode::Unreachable),
                Vec::new(),
            ),
            blk(
                push_lost,
                function,
                Vec::new(),
                trap(TrapCode::Unreachable),
                Vec::new(),
            ),
        ],
        operations: vec![
            oper(
                seed,
                entry,
                0,
                Opcode::VectorNew,
                Vec::new(),
                u8vec_type(),
                Immediate::None,
            ),
            oper(
                length,
                again,
                0,
                Opcode::VectorLen,
                vec![p(current)],
                u64_type(),
                Immediate::None,
            ),
            oper(
                take,
                again,
                1,
                Opcode::VectorGet,
                vec![p(source), r(length)],
                TypeExpr::Option(Box::new(u8_type())),
                Immediate::None,
            ),
            oper(
                grow,
                copy,
                0,
                Opcode::AdapterInvoke,
                vec![p(held), p(got)],
                bridge_result(u8vec_type()),
                Immediate::Entity(crate::extended::bridge_entry_id(*b"PSH1")),
            ),
            oper(
                cross,
                emit,
                0,
                Opcode::AdapterInvoke,
                vec![p(unit_param), p(ready)],
                bridge_result(TypeExpr::Bytes),
                Immediate::Entity(crate::extended::bridge_entry_id(*b"V2B1")),
            ),
            oper(
                wrap,
                answer,
                0,
                Opcode::ResultOk,
                vec![p(shown)],
                bridge_result(TypeExpr::Bytes),
                Immediate::None,
            ),
        ],
        adapters: bridge_subset(&[*b"PSH1", *b"V2B1"]),
        constants: Vec::new(),
    };
    let octets = |bytes: &[u8]| ConstValue {
        value_type: u8vec_type(),
        data: ConstData::Sequence(bytes.iter().map(|byte| octet(*byte)).collect()),
    };
    Workload {
        program,
        cases: vec![
            Case {
                id: "image-assemble-emit-n3",
                subject: 161,
                inputs: vec![octets(&[1, 2, 3]), unit_value()],
                expect: Expect::Success(ConstValue {
                    value_type: bridge_result(TypeExpr::Bytes),
                    data: ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
                        value_type: TypeExpr::Bytes,
                        data: ConstData::Bytes(vec![1, 2, 3]),
                    }))),
                }),
            },
            Case {
                id: "image-assemble-emit-n0",
                subject: 161,
                inputs: vec![octets(&[]), unit_value()],
                expect: Expect::Success(ConstValue {
                    value_type: bridge_result(TypeExpr::Bytes),
                    data: ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
                        value_type: TypeExpr::Bytes,
                        data: ConstData::Bytes(Vec::new()),
                    }))),
                }),
            },
        ],
    }
}

/// Explicit trap on a violated expectation: the false arm aborts with
/// `InternalInvariant` instead of returning a value. In-crate only (the
/// emitted vector set stays success-valued like the vm-extended family);
/// the exact trap termination is asserted here.
fn trap_on_violation() -> Workload {
    let function = id(1);
    let entry = id(2);
    let good = id(3);
    let bad = id(4);
    let flag = id(10);
    let program = Program {
        types: TypeEnvironment::new(Vec::new()).unwrap(),
        entry: fun(
            function,
            vec![flag],
            TypeExpr::Bool,
            entry,
            vec![entry, good, bad],
        ),
        functions: vec![fun(
            function,
            vec![flag],
            TypeExpr::Bool,
            entry,
            vec![entry, good, bad],
        )],
        parameters: vec![fparam(flag, function, 0, TypeExpr::Bool)],
        blocks: vec![
            blk(
                entry,
                function,
                Vec::new(),
                cbr(p(flag), good, Vec::new(), bad, Vec::new()),
                Vec::new(),
            ),
            blk(good, function, Vec::new(), ret(p(flag)), Vec::new()),
            blk(
                bad,
                function,
                Vec::new(),
                trap(TrapCode::InternalInvariant),
                Vec::new(),
            ),
        ],
        operations: Vec::new(),
        adapters: Vec::new(),
        constants: Vec::new(),
    };
    // `flag` is both the condition and the good-arm answer: a held
    // expectation returns itself, a violated one traps.
    Workload {
        program,
        cases: Vec::new(),
    }
}

/// Every closure workload with its vector cases.
fn workloads() -> Vec<Workload> {
    vec![
        bytes_round_trip(),
        vector_push_loop(),
        map_symbol_table(),
        set_as_map(),
        record_variant_walk(),
        multi_function_pass(),
        checked_length_traverse(),
        value_hash_chain(),
        variant_switch_exhaustive(),
        graph_worklist_dfs("graph-worklist-dfs-chain", [(1, 2), (2, 3), (3, 4)], 1, 4),
        graph_worklist_dfs("graph-worklist-dfs-cycle", [(1, 2), (2, 1), (9, 9)], 1, 2),
        image_assemble_emit(),
        trap_on_violation(),
    ]
}

/// Every closure workload is gate-admitted before it executes: the vectors
/// below prove profile membership, not just executability.
#[test]
fn closure_workloads_are_gate_admitted() {
    for workload in workloads() {
        let report = judge_bootstrap_profile(&workload.program.gate_input())
            .expect("closure workload is gate-admitted");
        assert!(
            !report.functions().is_empty(),
            "admission covers the entry function"
        );
        if !workload.program.operations.is_empty() {
            assert!(report.operation_count() > 0, "admission covers operations");
        }
    }
    // The bridge-carrying workloads demonstrably use the registry.
    for workload in workloads() {
        if workload.program.adapters.is_empty() {
            continue;
        }
        let report = judge_bootstrap_profile(&workload.program.gate_input())
            .expect("bridge workload is gate-admitted");
        assert!(
            report.bridge_uses() > 0,
            "bridge workloads admit through the registry"
        );
    }
}

/// Every vector case executes to its exact expected termination under the
/// frozen reference budgets, deterministically.
#[test]
fn closure_workloads_execute_to_exact_terminations() {
    for workload in workloads() {
        for case in &workload.cases {
            let first = workload.program.run(&case.inputs);
            let second = workload.program.run(&case.inputs);
            assert_eq!(first, second, "closure case {} repeats", case.id);
            let Expect::Success(expected) = &case.expect;
            let ExecutionTermination::Success(value) = &first.termination else {
                panic!("closure case {} succeeds: {:?}", case.id, first.termination);
            };
            assert_eq!(value, expected, "closure case {} value", case.id);
        }
    }
}

/// The trap workload (in-crate only): a held expectation returns itself,
/// a violated one traps with `InternalInvariant` and no payload.
#[test]
fn trap_on_violation_traps_exactly() {
    let workload = trap_on_violation();
    let held = workload.program.run(&[boolean(true)]);
    assert_eq!(
        held.termination,
        ExecutionTermination::Success(boolean(true))
    );
    let violated = workload.program.run(&[boolean(false)]);
    assert!(
        matches!(
            &violated.termination,
            ExecutionTermination::Trap {
                trap_tag: 4,
                payload: None
            }
        ),
        "violated expectation traps: {:?}",
        violated.termination
    );
}

#[test]
#[ignore = "emits the frozen closure vectors; run via scripts/generate_bootstrap_profile_fixtures.py"]
fn emit_bootstrap_profile_vectors_for_freeze() {
    use sley_ssmc::fingerprint::hash_validated_value;
    fn hex(bytes: &[u8]) -> String {
        use core::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
    }
    for workload in workloads() {
        // Gate admission precedes emission: every vector proves profile
        // membership, so the frozen set cannot silently include an
        // out-of-profile program.
        judge_bootstrap_profile(&workload.program.gate_input())
            .expect("emitted vectors are gate-admitted");
        for case in &workload.cases {
            let lowered = lower_function(workload.program.lowering_input()).unwrap();
            let outcome = workload.program.run(&case.inputs);
            let ExecutionTermination::Success(value) = &outcome.termination else {
                panic!("{}: not a success", case.id);
            };
            let value_hash =
                hash_validated_value(SchemaEpochId::from_bytes([8; 32]), value).unwrap();
            println!(
                "BOOTSTRAP_PROFILE_VECTOR|{}|{}|{}|{}|{}|{}|{}|{}",
                case.id,
                case.subject,
                hex(&lowered.bytes),
                hex(lowered.cache_key.as_bytes()),
                hex(outcome.observation_id.as_bytes()),
                outcome.instruction_count,
                outcome.fuel_used,
                hex(value_hash.as_bytes()),
            );
        }
    }
}
