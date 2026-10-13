//! `jg_*`: extended operation judgment and the bootstrap-profile gate
//! against the native `sley-vm` judgments.
#![allow(
    clippy::cast_precision_loss,
    clippy::items_after_statements,
    clippy::same_functions_in_if_condition,
    clippy::too_many_lines,
    clippy::type_complexity
)]
mod common;
use common::*;

use sley_check::TypeEnvironment;
use sley_id::EntityId;
use sley_selfhost::canonical::{self, CanonicalS};
use sley_selfhost::native::{self, decode_s, dev_epoch, dev_root};
use sley_selfhost::seed::Program;
use sley_ssmc::{ConstData, ConstValue, TypeExpr};
use sley_vm::bootstrap::{BootstrapProfileInput, BootstrapProfileVersion, judge_bootstrap_profile};
use sley_vm::{CacheProfile, LoweringError, LoweringInput};
use std::time::Instant;

fn lowering_code(error: &LoweringError) -> u32 {
    match error {
        LoweringError::Lower(e) => e.code().numeric(),
        LoweringError::Cfg(_) => 1,
    }
}

fn s_inputs(s: &CanonicalS) -> Vec<ConstValue> {
    vec![objects_input(s), bytes(s.epoch.as_bytes())]
}

fn ids_input(ids: &[EntityId]) -> ConstValue {
    ConstValue {
        value_type: sley_selfhost::seed::t::vec(TypeExpr::Bytes),
        data: ConstData::Sequence(ids.iter().map(|id| bytes(id.as_bytes())).collect()),
    }
}

fn result_list(value: ConstValue) -> Vec<Result<(), u32>> {
    match result(value).expect("loads").data {
        ConstData::Sequence(items) => items.into_iter().map(|c| result(c).map(|_| ())).collect(),
        other => panic!("not a vector: {other:?}"),
    }
}

fn byte_list(value: &ConstValue) -> Vec<Vec<u8>> {
    match &value.data {
        ConstData::Sequence(items) => items.iter().map(as_bytes).collect(),
        other => panic!("not a vector: {other:?}"),
    }
}

/// Native `judge_function_operations` for one function of `program`.
fn native_judge(program: &Program, types: &TypeEnvironment, fid: EntityId) -> Result<(), u32> {
    let function = program
        .functions
        .iter()
        .find(|f| f.entity_id == fid)
        .expect("function");
    sley_vm::judge_function_operations(LoweringInput {
        types,
        function,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        schema_epoch: dev_epoch(),
        state_root: dev_root(),
        profile: CacheProfile::EXTENDED_V1,
        constants: &program.constants,
        globals: &[],
        functions: &program.functions,
        contracts: &[],
        adapters: &program.imports,
    })
    .map(|_| ())
    .map_err(|e| lowering_code(&e))
}

/// Native gate report: (functions, ops, bridges, imports).
type GateClaims = (Vec<Vec<u8>>, u64, u64, Vec<Vec<u8>>);

fn native_gate(program: &Program, entry: EntityId) -> Result<GateClaims, u32> {
    let types = TypeEnvironment::new(program.type_definitions.clone()).expect("types");
    let dummy;
    let entry_fn = if let Some(found) = program.functions.iter().find(|f| f.entity_id == entry) {
        found
    } else {
        let mut graph = program.functions[0].clone();
        graph.entity_id = entry;
        dummy = graph;
        &dummy
    };
    judge_bootstrap_profile(&BootstrapProfileInput {
        types: &types,
        profile_version: BootstrapProfileVersion::V3,
        schema_epoch: canonical::epoch(),
        presented_image_bytes: &[],
        entry: entry_fn,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.imports,
        constants: &program.constants,
    })
    .map(|report| {
        (
            report
                .functions()
                .iter()
                .map(|id| id.as_bytes().to_vec())
                .collect(),
            u64::from(report.operation_count()),
            u64::from(report.bridge_uses()),
            report
                .imports()
                .iter()
                .map(|id| id.as_bytes().to_vec())
                .collect(),
        )
    })
    .map_err(|e| lowering_code(&e))
}

fn sley_gate(s: &CanonicalS, entry: EntityId, image: &[u8]) -> (Result<GateClaims, u32>, u64) {
    let mut inputs = s_inputs(s);
    inputs.push(bytes(entry.as_bytes()));
    inputs.push(bytes(image));
    let (value, count) = run_counted("t_jg_gate", inputs);
    let claims = result(value).map(|v| {
        let items = tuple_items(&v);
        (
            byte_list(&items[0]),
            as_u64(&items[1]),
            as_u64(&items[2]),
            byte_list(&items[3]),
        )
    });
    (claims, count)
}

#[test]
#[ignore = "full toolchain inventory (tens of minutes); the SH2 bootstrap covers the closure"]
fn every_toolchain_function_is_judged_like_native() {
    let (program, names) = program();
    let s = canonical::build(program, names["t_jg_all"], "jg-all");
    let decoded = decode_s(&s).expect("decodes").program;
    let types = TypeEnvironment::new(decoded.type_definitions.clone()).expect("types");
    for function in &decoded.functions {
        assert_eq!(
            native_judge(&decoded, &types, function.entity_id),
            Ok(()),
            "native judges every toolchain function"
        );
    }
    // One run (the full-size load dominates; `jg_function_cost` reports
    // the judgment's own instructions on a closure).
    let start = Instant::now();
    let (value, count) = run_counted("t_jg_all", s_inputs(&s));
    let elapsed = start.elapsed();
    let codes = result_list(value);
    assert_eq!(codes.len(), decoded.functions.len());
    for (function, code) in decoded.functions.iter().zip(&codes) {
        assert_eq!(
            *code,
            Ok(()),
            "jg_function refuses {:?}",
            function.entity_id
        );
    }
    eprintln!(
        "jg_function over all {} functions / {} operations: {count} instructions including \
         the load ({elapsed:?})",
        decoded.functions.len(),
        decoded.operations.len(),
    );
}

/// The judgment's own instruction cost over every function of a closure.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn jg_function_cost() {
    let (program, names) = program();
    let entry = names["cd_object"];
    let pruned = canonical::prune_to_closure(program, entry);
    let s = canonical::build(&pruned, entry, "jg-cost");
    let decoded = decode_s(&s).expect("decodes").program;
    let (load, load_count) = run_counted("t_jg_load", s_inputs(&s));
    result(load).expect("loads");
    let (value, count) = run_counted("t_jg_all", s_inputs(&s));
    assert!(result_list(value).iter().all(Result::is_ok));
    eprintln!(
        "jg_function over the cd_object closure ({} functions, {} operations): {} \
         instructions after the {load_count}-instruction load ({:.1} per operation)",
        decoded.functions.len(),
        decoded.operations.len(),
        count - load_count,
        (count - load_count) as f64 / decoded.operations.len() as f64
    );
}

fn check_gate(entry_name: &str, baseline: bool) {
    let (program, names) = program();
    let entry = names[entry_name];
    let pruned = canonical::prune_to_closure(program, entry);
    let s = canonical::build(&pruned, entry, "jg-gate");
    let decoded = decode_s(&s).expect("decodes").program;
    let image = native::lower(&pruned, entry).expect("lowers").bytes;
    let expected = native_gate(&decoded, entry).expect("native gate admits");
    let start = Instant::now();
    let load_count = if baseline {
        let (load, load_count) = run_counted("t_jg_load", s_inputs(&s));
        result(load).expect("loads");
        load_count
    } else {
        0
    };
    let (got, count) = sley_gate(&s, entry, &image);
    let elapsed = start.elapsed();
    assert_eq!(got, Ok(expected.clone()), "{entry_name}: gate claims");
    eprintln!(
        "{entry_name}: gate over {} functions, {} operations, {} bridge uses, {} imports: \
         {} instructions after a {load_count}-instruction load baseline ({elapsed:?})",
        expected.0.len(),
        expected.1,
        expected.2,
        expected.3.len(),
        count - load_count
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn gate_matches_native_cd_object() {
    check_gate("cd_object", true);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn gate_matches_native_drv_load() {
    check_gate("drv_load", false);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn gate_matches_native_sr_root() {
    check_gate("sr_root", true);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn gate_matches_native_judge_closure() {
    // The loader plus every `jg_*` function (the driver entry is not yet
    // part of `toolchain::module()`).
    check_gate("t_jg_all", false);
}

// ===================================================================
// Mutation corpus: single-operation functions judged by Sley and native.
// ===================================================================

use sley_check::cfg::validate_function_graph;
use sley_selfhost::seed::{bridge_id, t};
use sley_ssmc::{
    AdapterImport, Block, BuiltinFailureKind, ConstantDefinition, FieldConst, FunctionGraph,
    FunctionRefValue, FunctionType, Immediate, IntegerWidth, MapEntryConst, MemberId, NamedType,
    Opcode, Operation, Parameter, ParameterRole, Reachability, RecordConst, RecordField,
    ReturnTerminator, Terminator, TypeDefForm, TypeDefinition, TypeParameterDef, ValueRef,
    VariantCase, VariantImmediate, Visibility,
};
use std::collections::BTreeSet;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }
    fn pick<T: Clone>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len())].clone()
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

fn eid(tag: u8, n: u64) -> EntityId {
    let mut bytes = [0xA5_u8; 32];
    bytes[0] = tag;
    bytes[24..].copy_from_slice(&n.to_be_bytes());
    EntityId::from_bytes(bytes)
}

fn mid(n: u64) -> MemberId {
    MemberId::from_bytes(*eid(0x4D, n).as_bytes())
}

fn named(def: EntityId, args: Vec<TypeExpr>) -> TypeExpr {
    TypeExpr::Named(NamedType {
        definition: def,
        arguments: args,
    })
}

fn bf(kind: BuiltinFailureKind) -> TypeExpr {
    TypeExpr::BuiltinFailure(kind)
}

fn cell(inner: TypeExpr) -> TypeExpr {
    TypeExpr::LocalCell(Box::new(inner))
}

fn uconst(ty: TypeExpr, x: u128) -> ConstValue {
    ConstValue {
        value_type: ty,
        data: ConstData::UInt(x),
    }
}

/// The fixed (non-case) inventory of a corpus program.
struct Fixed {
    pt: EntityId,
    sh: EntityId,
    gen_: EntityId,
    flt: EntityId,
    hold: EntityId,
    pt_x: MemberId,
    pt_y: MemberId,
    sh_dot: MemberId,
    sh_line: MemberId,
    sh_box: MemberId,
    flt_f: MemberId,
    consts: Vec<EntityId>,
    callees: Vec<EntityId>,
    imports: Vec<EntityId>,
}

fn simple_function(
    program: &mut Program,
    fid: EntityId,
    seq: &mut u64,
    param_types: &[TypeExpr],
    result: TypeExpr,
    body: Option<(Opcode, Immediate, Vec<TypeExpr>)>,
) {
    let mut next = || {
        *seq += 1;
        eid(0x10, *seq)
    };
    let block = next();
    let mut params = Vec::new();
    for (ordinal, ty) in param_types.iter().enumerate() {
        let pid = next();
        program.parameters.push(Parameter {
            entity_id: pid,
            owner: fid,
            role: ParameterRole::Function,
            ordinal: u32::try_from(ordinal).unwrap(),
            value_type: ty.clone(),
        });
        params.push(pid);
    }
    let mut operations = Vec::new();
    if let Some((opcode, immediate, declared)) = body {
        let oid = next();
        program.operations.push(Operation {
            entity_id: oid,
            block,
            ordinal: 0,
            opcode,
            operands: params[1..]
                .iter()
                .map(|p| ValueRef::Parameter(*p))
                .collect(),
            result_types: declared,
            immediate,
        });
        operations.push(oid);
    }
    program.blocks.push(Block {
        entity_id: block,
        function: fid,
        parameters: Vec::new(),
        operations,
        terminator: Terminator::Return(ReturnTerminator {
            value: ValueRef::Parameter(params[0]),
        }),
        reachability: Reachability::Required,
    });
    program.functions.push(FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: params,
        result_type: result,
        effects: Vec::new(),
        entry_block: block,
        blocks: vec![block],
        contracts: Vec::new(),
        visibility: Visibility::Private,
    });
}

fn row(id: EntityId, request: TypeExpr, response: TypeExpr) -> AdapterImport {
    AdapterImport {
        entity_id: id,
        adapter_id: *id.as_bytes(),
        abi_version: 1,
        request_type: request,
        response_type: response,
        failure_type: bf(BuiltinFailureKind::Index),
        effects: Vec::new(),
    }
}

fn push_id_raw(discriminator: u32, tail: u8) -> EntityId {
    let mut bytes = *bridge_id(*b"PSH1", discriminator).as_bytes();
    bytes[31] = tail;
    EntityId::from_bytes(bytes)
}

/// Bridge rows: `good` carries the frozen rows; otherwise every frozen row
/// carries one malformed field.
fn corpus_imports(good: bool) -> Vec<AdapterImport> {
    let b = |c: &[u8; 4]| bridge_id(*c, 0);
    let u8v = t::vec(t::u8());
    let range = t::tuple(vec![t::u64(), t::u64()]);
    let mut rows = vec![
        row(b(b"B2V1"), t::bytes(), u8v.clone()),
        row(b(b"V2B1"), u8v.clone(), t::bytes()),
        row(b(b"RHW1"), t::bytes(), t::bytes()),
        row(b(b"RHW2"), t::bytes(), t::bytes()),
        row(b(b"RSH2"), t::bytes(), t::bytes()),
        row(b(b"BCT1"), t::vec(t::bytes()), t::bytes()),
        row(b(b"BSL1"), range, t::bytes()),
        row(b(b"BLN1"), t::bytes(), t::u64()),
        row(b(b"PSH1"), t::u8(), u8v.clone()),
        row(bridge_id(*b"PSH1", 1), t::u64(), t::vec(t::u64())),
        row(bridge_id(*b"PSH1", 2), t::bytes(), t::vec(t::bytes())),
        // Never frozen: unknown code, trailing push bytes, a broken push pin.
        row(b(b"XXX1"), t::bytes(), t::bytes()),
        row(push_id_raw(3, 9), t::u64(), t::vec(t::u64())),
        row(bridge_id(*b"PSH1", 4), t::u64(), t::vec(t::bytes())),
    ];
    {
        let mut r = row(bridge_id(*b"PSH1", 5), t::u64(), t::vec(t::u64()));
        r.abi_version = 2;
        rows.push(r);
        let mut r = row(bridge_id(*b"PSH1", 6), t::u64(), t::vec(t::u64()));
        r.failure_type = bf(BuiltinFailureKind::Arithmetic);
        rows.push(r);
        let mut r = row(bridge_id(*b"PSH1", 7), t::u64(), t::vec(t::u64()));
        r.adapter_id = [7; 32];
        rows.push(r);
        let mut r = row(bridge_id(*b"PSH1", 8), t::u64(), t::vec(t::u64()));
        r.effects = vec![eid(0xEF, 1)];
        rows.push(r);
    }
    if !good {
        for (i, r) in rows.iter_mut().enumerate().take(9) {
            match i % 6 {
                0 => r.abi_version = 3,
                1 => r.adapter_id = [1; 32],
                2 => r.failure_type = bf(BuiltinFailureKind::DuplicateKey),
                3 => r.effects = vec![eid(0xEF, 2)],
                4 => r.request_type = t::u64(),
                _ => r.response_type = TypeExpr::Unit,
            }
        }
    }
    rows
}

/// Builds the fixed inventory: type definitions, constants, callees,
/// imports.
fn corpus_fixed(program: &mut Program, seq: &mut u64, good_rows: bool) -> Fixed {
    let pt = eid(0x70, 1);
    let sh = eid(0x70, 2);
    let gen_ = eid(0x70, 3);
    let flt = eid(0x70, 4);
    let hold = eid(0x70, 5);
    let (pt_x, pt_y, sh_dot, sh_line, sh_box, flt_f, gen_v, hold_c) = (
        mid(1),
        mid(2),
        mid(3),
        mid(4),
        mid(5),
        mid(6),
        mid(7),
        mid(8),
    );
    let field = |member_id, value_type| RecordField {
        member_id,
        value_type,
        visibility: Visibility::Private,
    };
    let def = |entity_id, type_parameters, form| TypeDefinition {
        entity_id,
        type_parameters,
        form,
        invariants: Vec::new(),
        visibility: Visibility::Private,
    };
    program.type_definitions = vec![
        def(
            pt,
            vec![],
            TypeDefForm::Record(vec![field(pt_x, t::u64()), field(pt_y, t::u64())]),
        ),
        def(
            sh,
            vec![],
            TypeDefForm::Variant(vec![
                VariantCase {
                    member_id: sh_dot,
                    payload_type: None,
                },
                VariantCase {
                    member_id: sh_line,
                    payload_type: Some(t::u64()),
                },
                VariantCase {
                    member_id: sh_box,
                    payload_type: Some(named(pt, vec![])),
                },
            ]),
        ),
        def(
            gen_,
            vec![TypeParameterDef { ordinal: 0 }],
            TypeDefForm::Record(vec![field(gen_v, TypeExpr::TypeParameter(0))]),
        ),
        def(
            flt,
            vec![],
            TypeDefForm::Record(vec![field(flt_f, TypeExpr::F64)]),
        ),
        def(
            hold,
            vec![],
            TypeDefForm::Record(vec![field(hold_c, cell(t::u64()))]),
        ),
    ];
    let mut consts = Vec::new();
    let mut add_const = |program: &mut Program, n: u64, value: ConstValue| {
        let id = eid(0xC0, n);
        program.constants.push(ConstantDefinition {
            entity_id: id,
            value,
        });
        consts.push(id);
    };
    add_const(program, 1, uconst(t::u64(), 7));
    add_const(
        program,
        2,
        ConstValue {
            value_type: t::bytes(),
            data: ConstData::Bytes(vec![1, 2, 3]),
        },
    );
    add_const(
        program,
        3,
        ConstValue {
            value_type: t::map(t::u64(), t::bytes()),
            data: ConstData::Map(vec![MapEntryConst {
                key: uconst(t::u64(), 1),
                value: ConstValue {
                    value_type: t::bytes(),
                    data: ConstData::Bytes(vec![9]),
                },
            }]),
        },
    );
    add_const(
        program,
        4,
        ConstValue {
            value_type: named(pt, vec![]),
            data: ConstData::Record(RecordConst {
                definition: pt,
                fields: vec![
                    FieldConst {
                        member_id: pt_x,
                        value: uconst(t::u64(), 1),
                    },
                    FieldConst {
                        member_id: pt_y,
                        value: uconst(t::u64(), 2),
                    },
                ],
            }),
        },
    );
    add_const(
        program,
        5,
        ConstValue {
            value_type: TypeExpr::F64,
            data: ConstData::F64Bits(0x3ff0_0000_0000_0000),
        },
    );
    // Callees: plain, generic, with effects, with a missing parameter.
    let callee = eid(0xF0, 1);
    simple_function(
        program,
        callee,
        seq,
        &[t::u64(), t::bytes()],
        t::u64(),
        None,
    );
    let callee_gen = eid(0xF0, 2);
    simple_function(program, callee_gen, seq, &[t::u64()], t::u64(), None);
    program.functions.last_mut().unwrap().type_parameters = vec![TypeParameterDef { ordinal: 0 }];
    let callee_eff = eid(0xF0, 3);
    simple_function(
        program,
        callee_eff,
        seq,
        &[t::u64(), t::bytes()],
        t::u64(),
        None,
    );
    program.functions.last_mut().unwrap().effects = vec![eid(0xEF, 3), eid(0xEF, 4)];
    let callee_hole = eid(0xF0, 4);
    simple_function(program, callee_hole, seq, &[t::u64()], t::u64(), None);
    program
        .functions
        .last_mut()
        .unwrap()
        .parameters
        .push(eid(0xEE, 1));
    let callees = vec![callee, callee_gen, callee_eff, callee_hole, eid(0xF0, 99)];
    program.imports = corpus_imports(good_rows);
    let imports = program.imports.iter().map(|i| i.entity_id).collect();
    Fixed {
        pt,
        sh,
        gen_,
        flt,
        hold,
        pt_x,
        pt_y,
        sh_dot,
        sh_line,
        sh_box,
        flt_f,
        consts,
        callees,
        imports,
    }
}

fn type_pool(fx: &Fixed) -> Vec<TypeExpr> {
    vec![
        t::u64(),
        t::u32(),
        t::u8(),
        t::i64(),
        TypeExpr::Bool,
        TypeExpr::Unit,
        t::bytes(),
        t::text(),
        TypeExpr::F64,
        TypeExpr::F32,
        t::tuple(vec![t::u64(), t::bytes()]),
        t::tuple(vec![]),
        t::vec(t::u8()),
        t::vec(t::u64()),
        t::vec(t::bytes()),
        t::vec(TypeExpr::F64),
        t::opt(t::u64()),
        t::res(t::u64(), t::u32()),
        t::map(t::u64(), t::bytes()),
        t::map(t::bytes(), t::u64()),
        cell(t::u64()),
        cell(t::bytes()),
        named(fx.pt, vec![]),
        named(fx.sh, vec![]),
        named(fx.gen_, vec![t::u64()]),
        named(fx.gen_, vec![TypeExpr::F64]),
        named(fx.gen_, vec![t::vec(t::u64())]),
        named(fx.gen_, vec![t::opt(TypeExpr::AdapterHandle(eid(0xAD, 1)))]),
        named(fx.flt, vec![]),
        named(fx.hold, vec![]),
        TypeExpr::FunctionRef(FunctionType {
            parameters: vec![t::u64()],
            result: Box::new(t::u64()),
            effects: vec![],
        }),
        TypeExpr::AdapterHandle(eid(0xAD, 1)),
        TypeExpr::CapabilityToken(eid(0xAD, 2)),
        bf(BuiltinFailureKind::Index),
    ]
}

const ALL_OPCODES: &[Opcode] = &[
    Opcode::ConstantRef,
    Opcode::TupleNew,
    Opcode::TupleGet,
    Opcode::RecordNew,
    Opcode::RecordGet,
    Opcode::VariantNew,
    Opcode::VariantGet,
    Opcode::VectorNew,
    Opcode::VectorLen,
    Opcode::VectorGet,
    Opcode::VectorSet,
    Opcode::MapNew,
    Opcode::MapGet,
    Opcode::MapContains,
    Opcode::MapInsert,
    Opcode::MapRemove,
    Opcode::IntAddChecked,
    Opcode::IntSubChecked,
    Opcode::IntMulChecked,
    Opcode::IntDivChecked,
    Opcode::IntRemChecked,
    Opcode::IntNegChecked,
    Opcode::IntShlChecked,
    Opcode::IntShrChecked,
    Opcode::FloatAdd,
    Opcode::FloatSub,
    Opcode::FloatMul,
    Opcode::FloatDiv,
    Opcode::FloatNeg,
    Opcode::FloatFma,
    Opcode::Equal,
    Opcode::NotEqual,
    Opcode::LessThan,
    Opcode::LessEqual,
    Opcode::GreaterThan,
    Opcode::GreaterEqual,
    Opcode::BoolNot,
    Opcode::BoolAnd,
    Opcode::BoolOr,
    Opcode::CallDirect,
    Opcode::OptionSome,
    Opcode::OptionNone,
    Opcode::ResultOk,
    Opcode::ResultErr,
    Opcode::ContractAssert,
    Opcode::TestObserve,
    Opcode::EffectRequest,
    Opcode::AdapterInvoke,
    Opcode::CapabilityNarrow,
    Opcode::CellNew,
    Opcode::CellGet,
    Opcode::CellSet,
    Opcode::ValueHash,
    Opcode::GlobalGet,
    Opcode::FunctionRef,
];

fn fref(function: EntityId, type_arguments: Vec<TypeExpr>) -> Immediate {
    Immediate::Function(FunctionRefValue {
        function,
        type_arguments,
    })
}

/// Opcode-aware operand types and immediate (with random perturbation).
#[allow(clippy::too_many_lines)]
fn case_shape(
    rng: &mut Rng,
    fx: &Fixed,
    pool: &[TypeExpr],
    opcode: Opcode,
) -> (Vec<TypeExpr>, Immediate) {
    use Opcode as O;
    let any = |rng: &mut Rng| rng.pick(pool);
    let ints = [
        t::u64(),
        t::u32(),
        t::u8(),
        t::i64(),
        TypeExpr::SInt(IntegerWidth::from_bits(8)),
    ];
    let floats = [TypeExpr::F64, TypeExpr::F32];
    let pt = named(fx.pt, vec![]);
    let sh = named(fx.sh, vec![]);
    let defs = [fx.pt, fx.sh, fx.gen_, fx.flt, fx.hold, eid(0x70, 99)];
    let members = [
        fx.pt_x,
        fx.pt_y,
        fx.sh_dot,
        fx.sh_line,
        fx.sh_box,
        fx.flt_f,
        mid(99),
    ];
    let mut imm = Immediate::None;
    let operands: Vec<TypeExpr> = match opcode {
        O::ConstantRef => {
            imm = Immediate::Entity(if rng.chance(85) {
                rng.pick(&fx.consts)
            } else {
                eid(0xC0, 77)
            });
            if rng.chance(90) {
                vec![]
            } else {
                vec![t::u64()]
            }
        }
        O::TupleNew => (0..rng.below(4)).map(|_| any(rng)).collect(),
        O::TupleGet => {
            imm = Immediate::Index(u32::try_from(rng.below(3)).unwrap());
            vec![rng.pick(&[
                t::tuple(vec![t::u64(), t::bytes()]),
                t::tuple(vec![]),
                t::u64(),
            ])]
        }
        O::RecordNew => {
            imm = Immediate::Entity(rng.pick(&defs));
            rng.pick(&[
                vec![t::u64(), t::u64()],
                vec![t::u64()],
                vec![TypeExpr::F64],
                vec![cell(t::u64())],
                vec![t::u64(), t::u32()],
            ])
        }
        O::RecordGet => {
            imm = Immediate::Field(rng.pick(&members));
            vec![rng.pick(&[
                pt.clone(),
                sh.clone(),
                named(fx.flt, vec![]),
                named(fx.gen_, vec![t::u64()]),
                t::u64(),
            ])]
        }
        O::VariantNew | O::VariantGet => {
            imm = Immediate::Variant(VariantImmediate {
                definition: rng.pick(&defs),
                member_id: rng.pick(&members),
            });
            if opcode == O::VariantNew {
                rng.pick(&[vec![], vec![t::u64()], vec![pt.clone()], vec![t::bytes()]])
            } else {
                vec![rng.pick(&[sh.clone(), pt.clone(), named(fx.gen_, vec![t::u64()])])]
            }
        }
        O::VectorNew => {
            let a = any(rng);
            let b = any(rng);
            rng.pick(&[
                vec![],
                vec![a.clone()],
                vec![a.clone(), a.clone()],
                vec![a, b],
            ])
        }
        O::VectorLen => vec![rng.pick(&[t::vec(t::u8()), t::vec(t::u64()), t::u64()])],
        O::VectorGet => vec![
            rng.pick(&[
                t::vec(t::u64()),
                t::vec(t::u8()),
                t::map(t::u64(), t::bytes()),
            ]),
            rng.pick(&[t::u64(), t::u64(), t::u32()]),
        ],
        O::VectorSet => vec![
            t::vec(t::u64()),
            rng.pick(&[t::u64(), t::u8()]),
            rng.pick(&[t::u64(), t::u8()]),
        ],
        O::MapNew => rng.pick(&[
            vec![],
            vec![t::u64(), t::bytes()],
            vec![t::u64(), t::bytes(), t::u64(), t::bytes()],
            vec![t::u64(), t::bytes(), t::bytes(), t::bytes()],
            vec![TypeExpr::F64, t::u64()],
            vec![t::vec(t::u64()), t::u64()],
            vec![named(fx.gen_, vec![t::vec(t::u64())]), t::u64()],
            vec![named(fx.gen_, vec![t::u64()]), t::u64()],
            vec![named(fx.gen_, vec![TypeExpr::F64]), t::u64()],
            vec![named(fx.flt, vec![]), t::u64()],
            vec![pt.clone(), t::u64()],
            vec![t::tuple(vec![t::u64(), TypeExpr::F64]), t::u64()],
            vec![t::u64()],
        ]),
        O::MapGet | O::MapContains | O::MapRemove => vec![
            rng.pick(&[
                t::map(t::u64(), t::bytes()),
                t::map(t::bytes(), t::u64()),
                t::vec(t::u64()),
            ]),
            rng.pick(&[t::u64(), t::bytes()]),
        ],
        O::MapInsert => vec![
            rng.pick(&[t::map(t::u64(), t::bytes()), t::map(t::bytes(), t::u64())]),
            rng.pick(&[t::u64(), t::bytes()]),
            rng.pick(&[t::u64(), t::bytes()]),
        ],
        O::IntAddChecked
        | O::IntSubChecked
        | O::IntMulChecked
        | O::IntDivChecked
        | O::IntRemChecked => {
            let a = rng.pick(&ints);
            let b = if rng.chance(75) {
                a.clone()
            } else {
                rng.pick(&[t::u32(), TypeExpr::F64, t::bytes()])
            };
            vec![a, b]
        }
        O::IntNegChecked => vec![rng.pick(&[t::i64(), t::u64(), TypeExpr::F64, t::u8()])],
        O::IntShlChecked | O::IntShrChecked => {
            vec![
                rng.pick(&[t::u64(), t::i64(), t::bytes()]),
                rng.pick(&[t::u32(), t::u32(), t::u64()]),
            ]
        }
        O::FloatAdd | O::FloatSub | O::FloatMul | O::FloatDiv => {
            let a = rng.pick(&floats);
            vec![
                a.clone(),
                if rng.chance(70) {
                    a
                } else {
                    rng.pick(&[t::u64(), TypeExpr::F32])
                },
            ]
        }
        O::FloatNeg => vec![rng.pick(&[TypeExpr::F64, TypeExpr::F32, t::u64()])],
        O::FloatFma => {
            let a = rng.pick(&floats);
            vec![
                a.clone(),
                a.clone(),
                if rng.chance(70) { a } else { t::u64() },
            ]
        }
        O::Equal | O::NotEqual | O::LessThan | O::LessEqual | O::GreaterThan | O::GreaterEqual => {
            let a = any(rng);
            vec![a.clone(), if rng.chance(85) { a } else { any(rng) }]
        }
        O::BoolNot => vec![rng.pick(&[TypeExpr::Bool, TypeExpr::Bool, t::u64()])],
        O::BoolAnd | O::BoolOr => vec![
            TypeExpr::Bool,
            rng.pick(&[TypeExpr::Bool, TypeExpr::Bool, t::u64()]),
        ],
        O::CallDirect | O::FunctionRef => {
            let targs = if rng.chance(10) {
                vec![t::u64()]
            } else {
                vec![]
            };
            imm = fref(rng.pick(&fx.callees), targs);
            if opcode == O::FunctionRef {
                if rng.chance(90) {
                    vec![]
                } else {
                    vec![t::u64()]
                }
            } else {
                rng.pick(&[
                    vec![t::u64(), t::bytes()],
                    vec![t::u64(), t::bytes()],
                    vec![t::u64()],
                    vec![t::bytes(), t::u64()],
                ])
            }
        }
        O::OptionSome | O::ResultOk | O::ResultErr | O::CellNew | O::ValueHash => vec![any(rng)],
        O::OptionNone | O::GlobalGet | O::ContractAssert => {
            if opcode != O::OptionNone {
                imm = Immediate::Entity(rng.pick(&[eid(0xC0, 1), eid(0x99, 1)]));
            }
            if rng.chance(90) {
                vec![]
            } else {
                vec![t::u64()]
            }
        }
        O::CellGet => vec![if rng.chance(80) {
            cell(any(rng))
        } else {
            any(rng)
        }],
        O::CellSet => {
            let inner = any(rng);
            vec![
                cell(inner.clone()),
                if rng.chance(75) { inner } else { any(rng) },
            ]
        }
        O::AdapterInvoke => {
            let id = if rng.chance(90) {
                rng.pick(&fx.imports)
            } else {
                eid(0x99, 2)
            };
            imm = Immediate::Entity(id);
            let kinds: Vec<(TypeExpr, TypeExpr)> = vec![
                (TypeExpr::Unit, t::bytes()),
                (TypeExpr::Unit, t::vec(t::u8())),
                (TypeExpr::Unit, t::vec(t::bytes())),
                (t::bytes(), t::tuple(vec![t::u64(), t::u64()])),
                (t::vec(t::u8()), t::u8()),
                (t::vec(t::u64()), t::u64()),
                (t::vec(t::bytes()), t::bytes()),
                (TypeExpr::Unit, t::u64()),
            ];
            let (scope, request) = rng.pick(&kinds);
            if rng.chance(8) {
                vec![scope]
            } else if rng.chance(8) {
                vec![any(rng), any(rng)]
            } else {
                vec![scope, request]
            }
        }
        O::TestObserve | O::EffectRequest | O::CapabilityNarrow => {
            (0..rng.below(3)).map(|_| any(rng)).collect()
        }
    };
    if rng.chance(12) {
        imm = match rng.below(7) {
            0 => Immediate::None,
            1 => Immediate::Entity(rng.pick(&fx.consts)),
            2 => Immediate::Index(1),
            3 => Immediate::Field(fx.pt_x),
            4 => Immediate::Variant(VariantImmediate {
                definition: fx.sh,
                member_id: fx.sh_line,
            }),
            5 => Immediate::Observation([3; 32]),
            _ => fref(fx.callees[0], vec![]),
        };
    }
    (operands, imm)
}

/// The derived result candidates for one shape.
fn candidates(
    fx: &Fixed,
    pool: &[TypeExpr],
    operands: &[TypeExpr],
    program: &Program,
) -> Vec<TypeExpr> {
    let mut out = Vec::new();
    let mut parts = operands.to_vec();
    for o in operands {
        match o {
            TypeExpr::Vector(x) | TypeExpr::Option(x) | TypeExpr::LocalCell(x) => {
                parts.push((**x).clone());
            }
            TypeExpr::OrderedMap { key, value }
            | TypeExpr::Result {
                ok: key,
                error: value,
            } => {
                parts.push((**key).clone());
                parts.push((**value).clone());
            }
            TypeExpr::Tuple(items) => parts.extend(items.iter().cloned()),
            _ => {}
        }
    }
    for p in &parts {
        out.push(p.clone());
        out.push(t::opt(p.clone()));
        out.push(t::vec(p.clone()));
        out.push(cell(p.clone()));
        for kind in [BuiltinFailureKind::Arithmetic, BuiltinFailureKind::Index] {
            out.push(t::res(p.clone(), bf(kind)));
        }
    }
    if operands.len() >= 2 {
        out.push(t::res(
            t::map(operands[0].clone(), operands[1].clone()),
            bf(BuiltinFailureKind::DuplicateKey),
        ));
    }
    out.push(t::tuple(operands.to_vec()));
    out.extend(pool.iter().cloned());
    out.push(t::res(
        t::map(t::u64(), t::bytes()),
        bf(BuiltinFailureKind::DuplicateKey),
    ));
    out.push(t::res(
        t::map(TypeExpr::F64, t::bytes()),
        bf(BuiltinFailureKind::DuplicateKey),
    ));
    out.push(t::res(
        t::map(t::vec(t::u64()), t::bytes()),
        bf(BuiltinFailureKind::DuplicateKey),
    ));
    out.push(t::res(t::bytes(), bf(BuiltinFailureKind::Index)));
    out.push(t::res(t::vec(t::u8()), bf(BuiltinFailureKind::Index)));
    out.push(t::res(t::u64(), bf(BuiltinFailureKind::Index)));
    out.push(t::opt(named(fx.pt, vec![])));
    for c in &program.constants {
        out.push(c.value.value_type.clone());
    }
    for callee in &fx.callees {
        if let Some(g) = program.functions.iter().find(|f| f.entity_id == *callee) {
            let params: Option<Vec<TypeExpr>> = g
                .parameters
                .iter()
                .map(|p| {
                    program
                        .parameters
                        .iter()
                        .find(|q| q.entity_id == *p)
                        .map(|q| q.value_type.clone())
                })
                .collect();
            if let Some(parameters) = params {
                out.push(TypeExpr::FunctionRef(FunctionType {
                    parameters,
                    result: Box::new(g.result_type.clone()),
                    effects: g.effects.clone(),
                }));
            }
        }
    }
    out
}

struct CaseSpec {
    operands: Vec<TypeExpr>,
    opcode: Opcode,
    immediate: Immediate,
    declared: Vec<TypeExpr>,
    result: TypeExpr,
}

/// Generates `count` single-operation cases deterministically.
fn generate_cases(seed: u64, count: usize) -> Vec<CaseSpec> {
    let mut program = Program::default();
    let mut seq = 0;
    let fx = corpus_fixed(&mut program, &mut seq, true);
    let types = TypeEnvironment::new(program.type_definitions.clone()).expect("corpus types");
    let pool = type_pool(&fx);
    let mut rng = Rng(seed);
    let mut cases = Vec::new();
    let dummy = eid(0x01, 1);
    for i in 0..count {
        let opcode = ALL_OPCODES[i % ALL_OPCODES.len()];
        let (operands, immediate) = case_shape(&mut rng, &fx, &pool, opcode);
        let context = sley_vm::extended::LoweringContext {
            types: &types,
            constants: &program.constants,
            globals: &[],
            functions: &program.functions,
            parameters: &program.parameters,
            contracts: &[],
            adapters: &program.imports,
            function: dummy,
        };
        let refs: Vec<&TypeExpr> = operands.iter().collect();
        let found = candidates(&fx, &pool, &operands, &program)
            .into_iter()
            .find(|candidate| {
                sley_vm::extended::judge_extended_operation(
                    &context,
                    opcode,
                    &immediate,
                    &refs,
                    std::slice::from_ref(candidate),
                )
                .is_ok()
            });
        let roll = rng.below(100);
        let declared = match (found, roll) {
            (Some(ok), 0..=64) => vec![ok],
            (Some(ok), 65..=69) => vec![t::opt(ok)],
            (Some(ok), 70..=72) => vec![ok.clone(), ok],
            (_, 73..=76) => vec![],
            _ => vec![rng.pick(&pool)],
        };
        let result = if rng.chance(4) {
            rng.pick(&[
                cell(t::u64()),
                TypeExpr::AdapterHandle(eid(0xAD, 1)),
                t::vec(TypeExpr::CapabilityToken(eid(0xAD, 2))),
                named(fx.hold, vec![]),
                TypeExpr::F64,
            ])
        } else {
            TypeExpr::Unit
        };
        cases.push(CaseSpec {
            operands,
            opcode,
            immediate,
            declared,
            result,
        });
    }
    cases
}

/// One corpus program: the fixed inventory plus one function per case.
fn corpus_program(cases: &[CaseSpec], good_rows: bool) -> (Program, Vec<EntityId>) {
    let mut program = Program::default();
    let mut seq = 0;
    let _fx = corpus_fixed(&mut program, &mut seq, good_rows);
    let mut fids = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let fid = eid(0xFC, i as u64);
        let mut params = vec![case.result.clone()];
        params.extend(case.operands.iter().cloned());
        simple_function(
            &mut program,
            fid,
            &mut seq,
            &params,
            case.result.clone(),
            Some((case.opcode, case.immediate.clone(), case.declared.clone())),
        );
        fids.push(fid);
    }
    (program, fids)
}

fn owned_inventory(
    program: &Program,
    fid: EntityId,
) -> (Vec<Parameter>, Vec<Block>, Vec<Operation>) {
    let blocks: Vec<Block> = program
        .blocks
        .iter()
        .filter(|b| b.function == fid)
        .cloned()
        .collect();
    let ids: BTreeSet<EntityId> = blocks.iter().map(|b| b.entity_id).collect();
    let parameters = program
        .parameters
        .iter()
        .filter(|p| p.owner == fid || ids.contains(&p.owner))
        .cloned()
        .collect();
    let operations = program
        .operations
        .iter()
        .filter(|o| ids.contains(&o.block))
        .cloned()
        .collect();
    (parameters, blocks, operations)
}

/// Runs one corpus program through Sley and native; returns (compared,
/// refused, skipped by graph validation, per-code counts).
fn run_corpus(
    cases: &[CaseSpec],
    good_rows: bool,
) -> (usize, usize, usize, std::collections::BTreeMap<u32, usize>) {
    let (program, fids) = corpus_program(cases, good_rows);
    let s = canonical::build(&program, fids[0], "jg-corpus");
    let decoded = decode_s(&s).expect("decodes").program;
    let types = TypeEnvironment::new(decoded.type_definitions.clone()).expect("types");
    let mut inputs = s_inputs(&s);
    inputs.push(ids_input(&fids));
    let (value, count) = run_counted("t_jg_each", inputs);
    let got = result_list(value);
    let mut compared = 0;
    let mut refused = 0;
    let mut skipped = 0;
    let mut by_code = std::collections::BTreeMap::new();
    let mut mismatches = Vec::new();
    for (i, fid) in fids.iter().enumerate() {
        let function = decoded
            .functions
            .iter()
            .find(|f| f.entity_id == *fid)
            .unwrap();
        let (parameters, blocks, operations) = owned_inventory(&decoded, *fid);
        if validate_function_graph(&types, function, &parameters, &blocks, &operations).is_err() {
            skipped += 1;
            continue;
        }
        let expected = native_judge(&decoded, &types, *fid);
        compared += 1;
        if let Err(code) = expected {
            refused += 1;
            *by_code.entry(code).or_insert(0) += 1;
        } else {
            *by_code.entry(0).or_insert(0) += 1;
        }
        if got[i] != expected {
            mismatches.push(format!(
                "case {i}: {:?} imm {:?} operands {:?} declared {:?} result {:?}: sley {:?} native {expected:?}",
                cases[i].opcode, cases[i].immediate, cases[i].operands, cases[i].declared, cases[i].result, got[i]
            ));
        }
    }
    eprintln!(
        "corpus (good rows {good_rows}): {} cases, {compared} compared ({refused} refused), {skipped} \
         skipped by graph validation; {count} instructions incl. load; codes {by_code:?}",
        cases.len()
    );
    assert!(
        mismatches.is_empty(),
        "{} mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
    (compared, refused, skipped, by_code)
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn operation_judgment_mutation_corpus_matches_native() {
    let cases = generate_cases(0x5EED_1234_ABCD_0001, 1_650);
    let (compared, refused, _, codes) = run_corpus(&cases, true);
    assert!(
        compared > 1_000 && refused > 300,
        "corpus covers enough cases"
    );
    for code in [0, 26_001, 26_002, 26_003] {
        assert!(codes.contains_key(&code), "corpus covers code {code}");
    }
    let (compared, _, _, _) = run_corpus(&cases, false);
    assert!(compared > 1_000);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn big_bytes_constant_bound_matches_native() {
    for (len, expected) in [
        (0_usize, false),
        (5, false),
        (1_048_576, false),
        (1_048_577, true),
        (3_000_000, true),
    ] {
        let value = ConstValue {
            value_type: TypeExpr::Bytes,
            data: ConstData::Bytes(vec![7; len]),
        };
        let encoded = sley_mutate::encode_const_value(&value).expect("encodes");
        let ty = sley_selfhost::toolchain::types::scb_type_bytes(&TypeExpr::Bytes);
        let got = run("t_jg_big", vec![bytes(&encoded), bytes(&ty)]);
        assert_eq!(got.data, ConstData::Bool(expected), "length {len}");
    }
    // Non-Bytes constants never face the bound.
    let value = ConstValue {
        value_type: t::vec(t::u8()),
        data: ConstData::Sequence(vec![]),
    };
    let encoded = sley_mutate::encode_const_value(&value).expect("encodes");
    let ty = sley_selfhost::toolchain::types::scb_type_bytes(&t::vec(t::u8()));
    assert_eq!(
        run("t_jg_big", vec![bytes(&encoded), bytes(&ty)]).data,
        ConstData::Bool(false)
    );
}

// ===================================================================
// Gate mutation corpus.
// ===================================================================

fn gate_base() -> (Program, EntityId) {
    use sley_selfhost::seed as sd;
    use sley_selfhost::seed::{u64c, v};
    let mut m = sd::Module::new("jg-gate");
    m.record("Pt", &[("x", t::u64()), ("b", t::bytes())]);
    m.func("leaf", &[("x", t::u64())], t::u64(), |f| {
        f.ret(sd::add(v("x"), u64c(1)));
    });
    m.func("mid", &[("x", t::u64())], t::u64(), |f| {
        f.var("acc", u64c(0));
        f.for_u64("i", u64c(0), v("x"), |f| {
            f.set("acc", sd::add(v("acc"), sd::call("leaf", vec![v("i")])));
        });
        f.ret(v("acc"));
    });
    m.func("bytesy", &[("b", t::bytes())], t::bytes(), |f| {
        f.let_("w", sd::b2v(v("b")));
        f.var("o", sd::vnew(t::u8(), vec![]));
        f.set("o", sd::push(v("o"), sd::vget(v("w"), u64c(0))));
        f.let_("h", sd::blake3(sd::v2b(v("o"))));
        f.let_(
            "s",
            sd::sha256(sd::concat(sd::vnew(
                t::bytes(),
                vec![v("h"), sd::slice(v("b"), u64c(0), u64c(1))],
            ))),
        );
        f.ret(sd::blake3_large(v("s")));
    });
    m.func(
        "main",
        &[("x", t::u64()), ("b", t::bytes())],
        t::tuple(vec![t::u64(), t::bytes(), m.ty("Pt")]),
        |f| {
            f.var("xs", sd::vnew(t::u64(), vec![]));
            f.set("xs", sd::push(v("xs"), sd::call("mid", vec![v("x")])));
            f.set(
                "xs",
                sd::push(v("xs"), sd::call("leaf", vec![sd::blen(v("b"))])),
            );
            f.let_("p", sd::rec("Pt", vec![sd::vget(v("xs"), u64c(1)), v("b")]));
            f.ret(sd::tup(vec![
                sd::vlen(v("xs")),
                sd::call("bytesy", vec![sd::field(v("p"), "b")]),
                v("p"),
            ]));
        },
    );
    let (program, names) = m.finish();
    let entry = names["main"];
    (canonical::prune_to_closure(&program, entry), entry)
}

fn function_of(p: &Program, op: usize) -> EntityId {
    let block = p.operations[op].block;
    p.blocks
        .iter()
        .find(|b| b.entity_id == block)
        .unwrap()
        .function
}

fn is_call(o: &Operation) -> bool {
    o.opcode == Opcode::CallDirect
}

/// Renames an import identity everywhere.
fn rename_import(p: &mut Program, from: EntityId, to: EntityId, keep_adapter: bool) {
    for i in &mut p.imports {
        if i.entity_id == from {
            i.entity_id = to;
            if !keep_adapter {
                i.adapter_id = *to.as_bytes();
            }
        }
    }
    for o in &mut p.operations {
        if o.immediate == Immediate::Entity(from) {
            o.immediate = Immediate::Entity(to);
        }
    }
}

type Mutation = (String, Box<dyn Fn(&mut Program, &mut EntityId)>);

#[allow(clippy::too_many_lines)]
fn gate_mutations(base: &Program) -> Vec<Mutation> {
    let mut out: Vec<Mutation> = Vec::new();
    let mut add =
        |name: &str, f: Box<dyn Fn(&mut Program, &mut EntityId)>| out.push((name.to_owned(), f));
    add("base", Box::new(|_, _| {}));
    add("entry missing", Box::new(|_, e| *e = eid(0x99, 5)));
    // Call graph.
    let calls: Vec<usize> = base
        .operations
        .iter()
        .enumerate()
        .filter(|(_, o)| is_call(o))
        .map(|(i, _)| i)
        .collect();
    for (k, &ci) in calls.iter().enumerate() {
        add(
            &format!("self recursion {k}"),
            Box::new(move |p, _| {
                let caller = function_of(p, ci);
                if let Immediate::Function(r) = &mut p.operations[ci].immediate {
                    r.function = caller;
                }
            }),
        );
        add(
            &format!("call to entry {k}"),
            Box::new(move |p, e| {
                let entry = *e;
                if let Immediate::Function(r) = &mut p.operations[ci].immediate {
                    r.function = entry;
                }
            }),
        );
        add(
            &format!("missing callee {k}"),
            Box::new(move |p, _| {
                if let Immediate::Function(r) = &mut p.operations[ci].immediate {
                    r.function = eid(0x99, 6);
                }
            }),
        );
        add(
            &format!("generic call {k}"),
            Box::new(move |p, _| {
                if let Immediate::Function(r) = &mut p.operations[ci].immediate {
                    r.type_arguments = vec![t::u64()];
                }
            }),
        );
        add(
            &format!("call becomes non-function immediate {k}"),
            Box::new(move |p, _| {
                p.operations[ci].immediate = Immediate::None;
            }),
        );
    }
    add(
        "unreached function",
        Box::new(|p, _| {
            let mut seq = 9_000;
            simple_function(
                p,
                eid(0xFD, 1),
                &mut seq,
                &[TypeExpr::Unit],
                TypeExpr::Unit,
                None,
            );
        }),
    );
    add(
        "unreached function calling entry",
        Box::new(|p, e| {
            let mut seq = 9_000;
            let entry = *e;
            simple_function(
                p,
                eid(0xFD, 1),
                &mut seq,
                &[TypeExpr::Unit],
                TypeExpr::Unit,
                Some((Opcode::CallDirect, fref(entry, vec![]), vec![t::u64()])),
            );
        }),
    );
    for n in 0..base.functions.len() {
        add(
            &format!("effects on function {n}"),
            Box::new(move |p, _| p.functions[n].effects = vec![eid(0xEF, 1)]),
        );
        add(
            &format!("contracts on function {n}"),
            Box::new(move |p, _| p.functions[n].contracts = vec![eid(0xEF, 2)]),
        );
        add(
            &format!("type parameters on function {n}"),
            Box::new(move |p, _| {
                p.functions[n].type_parameters = vec![TypeParameterDef { ordinal: 0 }];
            }),
        );
        add(
            &format!("float result on function {n}"),
            Box::new(move |p, _| p.functions[n].result_type = TypeExpr::F64),
        );
        add(
            &format!("function-ref result on function {n}"),
            Box::new(move |p, _| {
                p.functions[n].result_type = TypeExpr::FunctionRef(FunctionType {
                    parameters: vec![],
                    result: Box::new(t::u64()),
                    effects: vec![],
                });
            }),
        );
        add(
            &format!("cell parameter on function {n}"),
            Box::new(move |p, _| {
                let pid = p.functions[n].parameters[0];
                let param = p
                    .parameters
                    .iter_mut()
                    .find(|q| q.entity_id == pid)
                    .unwrap();
                param.value_type = cell(param.value_type.clone());
            }),
        );
        add(
            &format!("unknown named result on function {n}"),
            Box::new(move |p, _| {
                p.functions[n].result_type = named(eid(0x70, 77), vec![]);
            }),
        );
        add(
            &format!("named float result on function {n}"),
            Box::new(move |p, _| {
                let def = eid(0x70, 4);
                p.type_definitions.push(TypeDefinition {
                    entity_id: def,
                    type_parameters: vec![],
                    form: TypeDefForm::Record(vec![RecordField {
                        member_id: mid(6),
                        value_type: t::vec(TypeExpr::F64),
                        visibility: Visibility::Private,
                    }]),
                    invariants: vec![],
                    visibility: Visibility::Private,
                });
                p.functions[n].result_type = t::opt(named(def, vec![]));
            }),
        );
        add(
            &format!("named cell-holder parameter on function {n}"),
            Box::new(move |p, _| {
                let def = eid(0x70, 5);
                let inner = eid(0x70, 6);
                p.type_definitions.push(TypeDefinition {
                    entity_id: inner,
                    type_parameters: vec![],
                    form: TypeDefForm::Variant(vec![VariantCase {
                        member_id: mid(9),
                        payload_type: Some(cell(t::u64())),
                    }]),
                    invariants: vec![],
                    visibility: Visibility::Private,
                });
                p.type_definitions.push(TypeDefinition {
                    entity_id: def,
                    type_parameters: vec![],
                    form: TypeDefForm::Record(vec![RecordField {
                        member_id: mid(8),
                        value_type: named(inner, vec![]),
                        visibility: Visibility::Private,
                    }]),
                    invariants: vec![],
                    visibility: Visibility::Private,
                });
                let pid = p.functions[n].parameters[0];
                p.parameters
                    .iter_mut()
                    .find(|q| q.entity_id == pid)
                    .unwrap()
                    .value_type = named(def, vec![]);
            }),
        );
    }
    // Block parameters: cells admitted, floats refused.
    if let Some(bp) = base
        .parameters
        .iter()
        .position(|q| q.role == ParameterRole::Block)
    {
        add(
            "cell block parameter",
            Box::new(move |p, _| {
                p.parameters[bp].value_type = cell(p.parameters[bp].value_type.clone());
            }),
        );
        add(
            "float block parameter",
            Box::new(move |p, _| p.parameters[bp].value_type = TypeExpr::F32),
        );
        add(
            "token block parameter",
            Box::new(move |p, _| {
                p.parameters[bp].value_type = TypeExpr::CapabilityToken(eid(0xAD, 3));
            }),
        );
    }
    // Operations.
    for (k, oi) in [
        0_usize,
        base.operations.len() / 2,
        base.operations.len() - 1,
    ]
    .into_iter()
    .enumerate()
    {
        for opcode in [
            Opcode::FloatAdd,
            Opcode::ContractAssert,
            Opcode::TestObserve,
            Opcode::EffectRequest,
            Opcode::CapabilityNarrow,
            Opcode::GlobalGet,
            Opcode::FunctionRef,
            Opcode::AdapterInvoke,
        ] {
            add(
                &format!("op {k} becomes {opcode:?}"),
                Box::new(move |p, _| p.operations[oi].opcode = opcode),
            );
        }
        add(
            &format!("op {k} float result"),
            Box::new(move |p, _| p.operations[oi].result_types = vec![TypeExpr::F64]),
        );
        add(
            &format!("op {k} cell result"),
            Box::new(move |p, _| p.operations[oi].result_types = vec![cell(t::u64())]),
        );
        add(
            &format!("op {k} handle result"),
            Box::new(move |p, _| {
                p.operations[oi].result_types =
                    vec![t::u64(), TypeExpr::AdapterHandle(eid(0xAD, 4))];
            }),
        );
    }
    // Bridges.
    let bridges: Vec<usize> = base
        .operations
        .iter()
        .enumerate()
        .filter(|(_, o)| o.opcode == Opcode::AdapterInvoke)
        .map(|(i, _)| i)
        .collect();
    for (k, &bi) in bridges.iter().enumerate() {
        add(
            &format!("bridge {k} non-entity immediate"),
            Box::new(move |p, _| p.operations[bi].immediate = Immediate::Index(0)),
        );
        add(
            &format!("bridge {k} unknown import"),
            Box::new(move |p, _| p.operations[bi].immediate = Immediate::Entity(eid(0x99, 7))),
        );
    }
    for (k, imp) in base.imports.iter().enumerate() {
        let id = imp.entity_id;
        fn at_id(p: &mut Program, id: EntityId) -> &mut AdapterImport {
            p.imports.iter_mut().find(|i| i.entity_id == id).unwrap()
        }
        add(
            &format!("import {k} abi"),
            Box::new(move |p, _| at_id(p, id).abi_version = 2),
        );
        add(
            &format!("import {k} adapter"),
            Box::new(move |p, _| at_id(p, id).adapter_id = [5; 32]),
        );
        add(
            &format!("import {k} failure"),
            Box::new(move |p, _| at_id(p, id).failure_type = bf(BuiltinFailureKind::Arithmetic)),
        );
        add(
            &format!("import {k} effects"),
            Box::new(move |p, _| at_id(p, id).effects = vec![eid(0xEF, 9)]),
        );
        add(
            &format!("import {k} request"),
            Box::new(move |p, _| at_id(p, id).request_type = t::text()),
        );
        add(
            &format!("import {k} response"),
            Box::new(move |p, _| at_id(p, id).response_type = t::vec(t::text())),
        );
        add(
            &format!("import {k} float schemas"),
            Box::new(move |p, _| {
                let r = at_id(p, id);
                if r.response_type == t::vec(r.request_type.clone()) {
                    r.request_type = TypeExpr::F64;
                    r.response_type = t::vec(TypeExpr::F64);
                } else {
                    r.request_type = TypeExpr::F64;
                }
            }),
        );
        add(
            &format!("import {k} renamed to discriminator 0"),
            Box::new(move |p, _| {
                let code: [u8; 4] = id.as_bytes()[12..16].try_into().unwrap();
                rename_import(p, id, bridge_id(code, 0), false);
            }),
        );
        add(
            &format!("import {k} renamed with trailing bytes"),
            Box::new(move |p, _| {
                let mut bytes = *id.as_bytes();
                bytes[31] = 1;
                rename_import(p, id, EntityId::from_bytes(bytes), false);
            }),
        );
        add(
            &format!("import {k} renamed unknown code"),
            Box::new(move |p, _| {
                rename_import(p, id, bridge_id(*b"ZZZ9", 0), false);
            }),
        );
    }
    add(
        "unreferenced push row",
        Box::new(|p, _| {
            p.imports
                .push(row(bridge_id(*b"PSH1", 77), t::u64(), t::vec(t::u64())));
        }),
    );
    add(
        "unreferenced raw-hash row",
        Box::new(|p, _| {
            if p.imports
                .iter()
                .any(|i| i.entity_id == bridge_id(*b"RHW1", 0))
            {
                p.imports
                    .push(row(bridge_id(*b"BLN9", 0), t::bytes(), t::u64()));
            } else {
                p.imports
                    .push(row(bridge_id(*b"RHW1", 0), t::bytes(), t::bytes()));
            }
        }),
    );
    // Constants.
    add(
        "float constant",
        Box::new(|p, _| {
            p.constants.push(ConstantDefinition {
                entity_id: eid(0xC0, 50),
                value: ConstValue {
                    value_type: TypeExpr::F64,
                    data: ConstData::F64Bits(0),
                },
            });
        }),
    );
    add(
        "unused u64 constant",
        Box::new(|p, _| {
            p.constants.push(ConstantDefinition {
                entity_id: eid(0xC0, 51),
                value: uconst(t::u64(), 3),
            });
        }),
    );
    add(
        "function-ref constant",
        Box::new(|p, e| {
            let entry = *e;
            p.constants.push(ConstantDefinition {
                entity_id: eid(0xC0, 52),
                value: ConstValue {
                    value_type: TypeExpr::FunctionRef(FunctionType {
                        parameters: vec![],
                        result: Box::new(t::u64()),
                        effects: vec![],
                    }),
                    data: ConstData::FunctionRef(FunctionRefValue {
                        function: entry,
                        type_arguments: vec![],
                    }),
                },
            });
        }),
    );
    out
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn gate_mutation_corpus_matches_native() {
    let (base, entry0) = gate_base();
    let mutations = gate_mutations(&base);
    let mut by_code = std::collections::BTreeMap::new();
    let mut mismatches = Vec::new();
    let mut instructions = 0;
    for (name, mutate) in &mutations {
        let mut p = base.clone();
        let mut entry = entry0;
        mutate(&mut p, &mut entry);
        let s = canonical::build(&p, entry0, "jg-gate-mut");
        let decoded = decode_s(&s).expect("decodes").program;
        let expected = native_gate(&decoded, entry);
        let (got, count) = sley_gate(&s, entry, &[]);
        instructions += count;
        *by_code
            .entry(expected.as_ref().err().copied().unwrap_or(0))
            .or_insert(0) += 1;
        if got != expected {
            mismatches.push(format!("{name}: sley {got:?} native {expected:?}"));
        }
    }
    eprintln!(
        "gate corpus: {} mutants, codes {by_code:?}, {instructions} instructions total",
        mutations.len()
    );
    assert!(
        mismatches.is_empty(),
        "{} mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
    for code in [0, 26_000, 26_001, 26_002, 26_003] {
        assert!(by_code.contains_key(&code), "gate corpus covers {code}");
    }
}

/// Full-size timing split (slow; run explicitly with `--ignored`).
#[test]
#[ignore = "full-size timing: two full toolchain loads"]
fn jg_full_cost() {
    let (program, names) = program();
    let s = canonical::build(program, names["t_jg_all"], "jg-all");
    let start = Instant::now();
    let (load, load_count) = run_counted("t_jg_load", s_inputs(&s));
    result(load).expect("loads");
    let load_time = start.elapsed();
    let start = Instant::now();
    let (value, count) = run_counted("t_jg_all", s_inputs(&s));
    assert!(result_list(value).iter().all(Result::is_ok));
    eprintln!(
        "full: load {load_count} instructions in {load_time:?}; load + jg_function {count} \
         instructions in {:?}; jg_function alone {} instructions",
        start.elapsed(),
        count - load_count
    );
}
