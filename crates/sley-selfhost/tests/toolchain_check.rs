//! `ck_*`: Sley type-environment and function-graph validation against the
//! native `sley-check` judgments.
//!
//! The mutation corpus runs on a subset module (model, runtime, codec,
//! inventory, types, check) so it does not depend on modules other agents
//! edit concurrently; `toolchain_passes_types_and_every_function` runs the
//! complete toolchain.
#![allow(clippy::format_collect, clippy::items_after_statements)]
mod common;

use common::{as_u64, bytes, objects_input, result};
use sley_check::TypeEnvironment;
use sley_check::cfg::{CfgValidationError, validate_function_graph};
use sley_id::EntityId;
use sley_selfhost::canonical;
use sley_selfhost::native::{big_limits, execute};
use sley_selfhost::seed;
use sley_selfhost::seed::{FnB, Module, Program, add, field, gt, mul, some, t, tri, u32c, u64c, v};
use sley_selfhost::toolchain;
use sley_ssmc::{
    Block, BranchTerminator, BuiltinCase, CaseKey, ConstData, ConstValue, FunctionGraph,
    FunctionType, Immediate, IntegerWidth, MemberId, NamedType, Opcode, Operation,
    OperationResultRef, Parameter, ParameterRole, Reachability, RecordField, ReturnTerminator,
    SwitchArgument, SwitchCase, SwitchEdge, TargetEdge, Terminator, TrapCode, TrapTerminator,
    TypeDefForm, TypeDefinition, TypeExpr, TypeParameterDef, ValueRef, VariantCase,
    VariantSwitchTerminator, Visibility,
};
use sley_vm::ExecutionTermination;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

// ----- harness -----

/// The check subset of the toolchain.
fn ck_program() -> &'static (Program, HashMap<String, EntityId>) {
    static P: OnceLock<(Program, HashMap<String, EntityId>)> = OnceLock::new();
    P.get_or_init(|| {
        use toolchain::*;
        let mut m = Module::new(toolchain::TOOLCHAIN_SEED);
        model::declare(&mut m);
        rt::define(&mut m);
        codec::define(&mut m);
        types::define(&mut m);
        inv::define(&mut m);
        check::define(&mut m);
        scb::define(&mut m);
        scb::define_entry_key(&mut m);
        m.finish()
    })
}

fn run_in(
    (program, names): &(Program, HashMap<String, EntityId>),
    entry: &str,
    inputs: Vec<ConstValue>,
) -> (ConstValue, u64) {
    let outcome = execute(program, names[entry], inputs, big_limits()).expect("executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => (value, outcome.instruction_count),
        other => panic!("{entry}: {other:?}"),
    }
}

/// The native owned inventory of `fid` (the `sley-vm` lowering narrowing).
fn owned(program: &Program, fid: EntityId) -> (Vec<Parameter>, Vec<Block>, Vec<Operation>) {
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

fn native_types(program: &Program) -> Result<TypeEnvironment, u32> {
    TypeEnvironment::new(program.type_definitions.clone()).map_err(|e| e.code().numeric())
}

fn native_fn(types: &TypeEnvironment, program: &Program, fid: EntityId) -> Result<(), u32> {
    let function = program
        .functions
        .iter()
        .find(|f| f.entity_id == fid)
        .expect("function");
    let (parameters, blocks, operations) = owned(program, fid);
    validate_function_graph(types, function, &parameters, &blocks, &operations)
        .map(|_| ())
        .map_err(|e| match e {
            CfgValidationError::Cfg(e) => e.code().numeric(),
            CfgValidationError::Type(e) => e.code().numeric(),
        })
}

fn s_inputs(program: &Program) -> Vec<ConstValue> {
    let s = canonical::build(program, program.functions[0].entity_id, "ck-test");
    vec![objects_input(&s), bytes(s.epoch.as_bytes())]
}

fn unit_result(value: ConstValue) -> Result<(), u32> {
    result(value).map(|_| ())
}

fn sley_types(
    tool: &(Program, HashMap<String, EntityId>),
    program: &Program,
) -> (Result<(), u32>, u64) {
    let (value, count) = run_in(tool, "t_ck_types", s_inputs(program));
    (unit_result(value), count)
}

/// Sley `ck_fn` (shared index and memo) for each of `fids`, one load.
fn sley_each(
    tool: &(Program, HashMap<String, EntityId>),
    program: &Program,
    fids: &[EntityId],
) -> (Vec<Result<(), u32>>, u64) {
    let mut inputs = s_inputs(program);
    inputs.push(ConstValue {
        value_type: t::vec(TypeExpr::Bytes),
        data: ConstData::Sequence(fids.iter().map(|id| bytes(id.as_bytes())).collect()),
    });
    let (value, count) = run_in(tool, "t_ck_each", inputs);
    let codes = match result(value).expect("loads").data {
        ConstData::Sequence(items) => items
            .iter()
            .map(|c| match as_u64(c) {
                0 => Ok(()),
                c => Err(u32::try_from(c).unwrap()),
            })
            .collect(),
        other => panic!("not a vector: {other:?}"),
    };
    (codes, count)
}

fn code_of(value: &ConstValue) -> Result<(), u32> {
    match as_u64(value) {
        0 => Ok(()),
        c => Err(u32::try_from(c).unwrap()),
    }
}

/// Sley `ck_types`, then `ck_fn` for each of `fids` when the types pass.
fn sley_both(
    tool: &(Program, HashMap<String, EntityId>),
    program: &Program,
    fids: &[EntityId],
) -> (Result<(), u32>, Vec<Result<(), u32>>) {
    let mut inputs = s_inputs(program);
    inputs.push(ConstValue {
        value_type: t::vec(TypeExpr::Bytes),
        data: ConstData::Sequence(fids.iter().map(|id| bytes(id.as_bytes())).collect()),
    });
    let (value, _) = run_in(tool, "t_ck_both", inputs);
    let pair = match result(value).expect("loads").data {
        ConstData::Sequence(items) => items,
        other => panic!("not a tuple: {other:?}"),
    };
    let codes = match &pair[1].data {
        ConstData::Sequence(items) => items.iter().map(code_of).collect(),
        other => panic!("not a vector: {other:?}"),
    };
    (code_of(&pair[0]), codes)
}

/// Comparison bookkeeping.
#[derive(Default)]
struct Corpus {
    codes: BTreeMap<u32, usize>,
    cases: usize,
    mismatches: Vec<String>,
}

impl Corpus {
    fn record(&mut self, name: &str, native: Result<(), u32>, sley: Result<(), u32>) {
        self.cases += 1;
        if let Err(code) = native {
            *self.codes.entry(code).or_default() += 1;
        }
        if native != sley {
            self.mismatches
                .push(format!("{name}: native {native:?} sley {sley:?}"));
        }
    }

    /// Types, then (when the environment is valid) every function.
    fn check(&mut self, name: &str, program: &Program) {
        let tool = ck_program();
        let native = native_types(program);
        let fids: Vec<EntityId> = program.functions.iter().map(|f| f.entity_id).collect();
        let (sley, codes) = sley_both(tool, program, &fids);
        self.record(
            &format!("{name}/types"),
            native.as_ref().map(|_| ()).map_err(|c| *c),
            sley,
        );
        let Ok(types) = native else { return };
        assert_eq!(codes.len(), fids.len(), "{name}: function codes");
        for (fid, sley) in fids.iter().zip(codes) {
            let native = native_fn(&types, program, *fid);
            self.record(&format!("{name}/fn {}", short(*fid)), native, sley);
        }
    }

    /// Asserts the named case produced `code` natively.
    fn expect(&mut self, name: &str, program: &Program, code: u32) {
        let before = self.codes.get(&code).copied().unwrap_or(0);
        self.check(name, program);
        if self.codes.get(&code).copied().unwrap_or(0) == before {
            self.mismatches
                .push(format!("{name}: native did not produce {code}"));
        }
    }
}

fn short(id: EntityId) -> String {
    id.as_bytes()[..4]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn fresh(n: u32) -> EntityId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"ck-test-fresh");
    hasher.update(&n.to_be_bytes());
    EntityId::from_bytes(*hasher.finalize().as_bytes())
}

fn named(definition: EntityId, arguments: Vec<TypeExpr>) -> TypeExpr {
    TypeExpr::Named(NamedType {
        definition,
        arguments,
    })
}

fn width(bits: u16) -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(bits))
}

// ----- the full toolchain -----

#[test]
#[ignore = "full toolchain inventory (tens of minutes); the SH2 bootstrap covers the closure"]
fn toolchain_passes_types_and_every_function() {
    static FULL: OnceLock<(Program, HashMap<String, EntityId>)> = OnceLock::new();
    let full = FULL.get_or_init(|| toolchain::module().finish());
    let program = &full.0;
    let types = native_types(program).expect("native environment");
    for function in &program.functions {
        native_fn(&types, program, function.entity_id).expect("native accepts the toolchain");
    }
    let (value, count) = run_in(full, "t_ck_all", s_inputs(program));
    assert_eq!(unit_result(value), Ok(()));
    eprintln!(
        "ck_all over the full toolchain: {count} instructions including the load ({} functions, {} blocks, {} operations, {} parameters)",
        program.functions.len(),
        program.blocks.len(),
        program.operations.len(),
        program.parameters.len()
    );
}

/// The check subset: types and every function, with instruction counts.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn subset_passes_with_instruction_counts() {
    let tool = ck_program();
    let program = &tool.0;
    let types = native_types(program).expect("native environment");
    let mut largest = (0, program.functions[0].entity_id);
    for function in &program.functions {
        native_fn(&types, program, function.entity_id).expect("native accepts");
        if function.blocks.len() > largest.0 {
            largest = (function.blocks.len(), function.entity_id);
        }
    }
    let (load, load_count) = run_in(tool, "t_ck_load", s_inputs(program));
    result(load).expect("loads");
    let (ty, types_count) = sley_types(tool, program);
    assert_eq!(ty, Ok(()));
    let (value, all_count) = run_in(tool, "t_ck_all", s_inputs(program));
    assert_eq!(unit_result(value), Ok(()));
    let (one, one_count) = sley_each(tool, program, &[largest.1]);
    assert_eq!(one, vec![Ok(())]);
    let operations = {
        let (_, _, ops) = owned(program, largest.1);
        ops.len()
    };
    eprintln!(
        "subset: {} functions, {} blocks, {} operations, {} parameters, {} definitions",
        program.functions.len(),
        program.blocks.len(),
        program.operations.len(),
        program.parameters.len(),
        program.type_definitions.len()
    );
    eprintln!("  load: {load_count} instructions");
    eprintln!("  ck_types: {} instructions", types_count - load_count);
    eprintln!(
        "  ck_all (types + every function): {} instructions",
        all_count - load_count
    );
    eprintln!(
        "  ck_fn on the largest function ({} blocks, {operations} operations): {} instructions (incl. index)",
        largest.0,
        one_count - load_count
    );
}

// ----- mutation corpus -----

fn fun_mut(p: &mut Program, fid: EntityId) -> &mut FunctionGraph {
    p.functions
        .iter_mut()
        .find(|f| f.entity_id == fid)
        .expect("function")
}
fn blk_mut(p: &mut Program, id: EntityId) -> &mut Block {
    p.blocks
        .iter_mut()
        .find(|b| b.entity_id == id)
        .expect("block")
}
fn op_mut(p: &mut Program, id: EntityId) -> &mut Operation {
    p.operations
        .iter_mut()
        .find(|o| o.entity_id == id)
        .expect("operation")
}
fn prm_mut(p: &mut Program, id: EntityId) -> &mut Parameter {
    p.parameters
        .iter_mut()
        .find(|x| x.entity_id == id)
        .expect("parameter")
}
fn fun(p: &Program, fid: EntityId) -> &FunctionGraph {
    p.functions
        .iter()
        .find(|f| f.entity_id == fid)
        .expect("function")
}
fn blk(p: &Program, id: EntityId) -> &Block {
    p.blocks.iter().find(|b| b.entity_id == id).expect("block")
}
/// The first declared block of `fid` matching `pred`.
fn find_block(p: &Program, fid: EntityId, pred: impl Fn(&Block) -> bool) -> EntityId {
    fun(p, fid)
        .blocks
        .iter()
        .copied()
        .find(|id| pred(blk(p, *id)))
        .expect("a matching block")
}
fn entry(p: &Program, fid: EntityId) -> EntityId {
    fun(p, fid).entry_block
}
fn pv(id: EntityId) -> ValueRef {
    ValueRef::Parameter(id)
}
fn ov(op: EntityId, index: u32) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation: op,
        result_index: index,
    })
}
fn br(target: EntityId, arguments: Vec<ValueRef>) -> Terminator {
    Terminator::Branch(BranchTerminator {
        edge: TargetEdge { target, arguments },
    })
}
fn ret(value: ValueRef) -> Terminator {
    Terminator::Return(ReturnTerminator { value })
}
fn trap0() -> Terminator {
    Terminator::Trap(TrapTerminator {
        code: TrapCode::Unreachable,
        payload: None,
    })
}

/// The seed-built mutation base.
fn base() -> (Program, HashMap<String, EntityId>) {
    let mut m = Module::new("ck-mut");
    m.record("Pt", &[("x", t::u64()), ("y", t::u64())]);
    m.variant(
        "Shape",
        &[
            ("Dot", None),
            ("Line", Some(t::u64())),
            ("Box", Some(m.ty("Pt"))),
        ],
    );
    m.func(
        "area",
        &[("s", m.ty("Shape")), ("n", t::u64())],
        t::res(t::u64(), t::u32()),
        |f| {
            f.var("out", u64c(0));
            f.match_var(
                v("s"),
                vec![
                    (
                        "Dot",
                        "_",
                        Box::new(|f: &mut FnB<'_>| f.set("out", u64c(1))),
                    ),
                    (
                        "Line",
                        "len",
                        Box::new(|f: &mut FnB<'_>| f.set("out", v("len"))),
                    ),
                    (
                        "Box",
                        "p",
                        Box::new(|f: &mut FnB<'_>| {
                            f.set("out", mul(field(v("p"), "x"), field(v("p"), "y")));
                        }),
                    ),
                ],
            );
            f.for_u64("i", u64c(0), v("n"), |f| {
                f.if_(gt(v("i"), u64c(10)), |f| f.ret_err(u32c(7)));
                f.set("out", add(v("out"), v("i")));
            });
            f.match_opt(
                some(v("out")),
                "o",
                |f| f.set("out", add(v("o"), u64c(1))),
                |_| {},
            );
            f.ret_ok(v("out"));
        },
    );
    m.func("spin", &[("x", t::u64())], t::u64(), |f| {
        f.var("y", v("x"));
        f.loop_(|f| {
            f.if_(gt(v("y"), u64c(100)), |f| f.ret(v("y")));
            f.set("y", add(v("y"), u64c(1)));
        });
    });
    m.func(
        "tryit",
        &[("r", t::res(t::u64(), t::u32()))],
        t::res(t::u64(), t::u32()),
        |f| {
            f.let_("x", tri(v("r")));
            f.ret_ok(add(v("x"), u64c(1)));
        },
    );
    m.finish()
}

type Mutation = Box<dyn Fn(&mut Program, &HashMap<String, EntityId>)>;

#[allow(clippy::too_many_lines)]
fn function_mutations() -> Vec<(&'static str, Option<u32>, Mutation)> {
    let area = |n: &HashMap<String, EntityId>| n["area"];
    let spin = |n: &HashMap<String, EntityId>| n["spin"];
    let first_op_block = move |p: &Program, n: &HashMap<String, EntityId>| {
        find_block(p, area(n), |b| !b.operations.is_empty())
    };
    let some_param_block = move |p: &Program, n: &HashMap<String, EntityId>| {
        find_block(p, area(n), |b| !b.parameters.is_empty())
    };
    let trap_payload_op = move |p: &Program, n: &HashMap<String, EntityId>| {
        let b = find_block(p, area(n), |b| {
            matches!(
                b.terminator,
                Terminator::Trap(TrapTerminator {
                    payload: Some(_),
                    ..
                })
            )
        });
        let Terminator::Trap(TrapTerminator {
            payload: Some(ValueRef::OperationResult(r)),
            ..
        }) = blk(p, b).terminator
        else {
            panic!("trap payload")
        };
        r.operation
    };
    let set_param_type = move |ty: TypeExpr| -> Mutation {
        Box::new(move |p, n| {
            let id = fun(p, area(n)).parameters[1];
            prm_mut(p, id).value_type = ty.clone();
        })
    };
    let pt = named(seed::type_id("ck-mut", "Pt"), vec![]);
    let fnref = TypeExpr::FunctionRef(FunctionType {
        parameters: vec![t::u64()],
        result: Box::new(t::u64()),
        effects: vec![],
    });
    vec![
        (
            "baseline",
            None,
            Box::new(|_: &mut Program, _: &HashMap<String, EntityId>| {}) as Mutation,
        ),
        (
            "blocks-dup",
            Some(22_000),
            Box::new(move |p, n| {
                let f = fun_mut(p, area(n));
                let b = f.blocks[1];
                f.blocks.push(b);
            }),
        ),
        (
            "blocks-missing",
            Some(22_001),
            Box::new(move |p, n| {
                fun_mut(p, area(n)).blocks.pop();
            }),
        ),
        (
            "block-extra",
            Some(22_001),
            Box::new(move |p, n| {
                p.blocks.push(Block {
                    entity_id: fresh(1),
                    function: area(n),
                    parameters: vec![],
                    operations: vec![],
                    terminator: trap0(),
                    reachability: Reachability::ExplicitlyUnreachable,
                });
            }),
        ),
        (
            "entry-foreign",
            Some(22_005),
            Box::new(move |p, n| {
                let e = entry(p, spin(n));
                fun_mut(p, area(n)).entry_block = e;
            }),
        ),
        (
            "entry-unreachable",
            Some(22_005),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                blk_mut(p, e).reachability = Reachability::ExplicitlyUnreachable;
            }),
        ),
        (
            "fparam-ordinal",
            Some(22_003),
            Box::new(move |p, n| {
                let id = fun(p, area(n)).parameters[1];
                prm_mut(p, id).ordinal = 0;
            }),
        ),
        (
            "fparam-foreign",
            Some(22_004),
            Box::new(move |p, n| {
                let id = fun(p, area(n)).parameters[0];
                prm_mut(p, id).owner = spin(n);
            }),
        ),
        (
            "fparam-role",
            Some(22_002),
            Box::new(move |p, n| {
                let id = fun(p, area(n)).parameters[0];
                prm_mut(p, id).role = ParameterRole::Block;
            }),
        ),
        (
            "fparam-missing",
            Some(22_004),
            Box::new(move |p, n| {
                fun_mut(p, area(n)).parameters.push(fresh(2));
            }),
        ),
        (
            "fparam-dup",
            Some(22_000),
            Box::new(move |p, n| {
                let f = fun_mut(p, area(n));
                let id = f.parameters[0];
                f.parameters.push(id);
            }),
        ),
        (
            "fparam-extra",
            Some(22_001),
            Box::new(move |p, n| {
                p.parameters.push(Parameter {
                    entity_id: fresh(3),
                    owner: area(n),
                    role: ParameterRole::Function,
                    ordinal: 2,
                    value_type: t::u64(),
                });
            }),
        ),
        (
            "fparam-two-failures-min-id",
            None,
            Box::new(move |p, n| {
                // Both parameters fail differently: the smaller identity wins.
                let ids = fun(p, area(n)).parameters.clone();
                prm_mut(p, ids[0]).ordinal = 7;
                prm_mut(p, ids[1]).value_type = width(7);
            }),
        ),
        (
            "bparam-ordinal",
            Some(22_003),
            Box::new(move |p, n| {
                let b = some_param_block(p, n);
                let id = blk(p, b).parameters[0];
                prm_mut(p, id).ordinal = 9;
            }),
        ),
        (
            "bparam-moved",
            Some(22_002),
            Box::new(move |p, n| {
                let b = some_param_block(p, n);
                let id = blk(p, b).parameters[0];
                let e = entry(p, area(n));
                blk_mut(p, e).parameters.push(id);
            }),
        ),
        (
            "bparam-role",
            Some(22_002),
            Box::new(move |p, n| {
                let b = some_param_block(p, n);
                let id = blk(p, b).parameters[0];
                prm_mut(p, id).role = ParameterRole::Function;
            }),
        ),
        (
            "bparam-dup",
            Some(22_000),
            Box::new(move |p, n| {
                let b = some_param_block(p, n);
                let id = blk(p, b).parameters[0];
                blk_mut(p, b).parameters.push(id);
            }),
        ),
        (
            "op-ordinal",
            Some(22_003),
            Box::new(move |p, n| {
                let b = first_op_block(p, n);
                let id = blk(p, b).operations[0];
                op_mut(p, id).ordinal = 5;
            }),
        ),
        (
            "op-block",
            Some(22_002),
            Box::new(move |p, n| {
                let b = first_op_block(p, n);
                let id = blk(p, b).operations[0];
                let other = find_block(p, area(n), |x| x.entity_id != b);
                op_mut(p, id).block = other;
            }),
        ),
        (
            "op-undeclared",
            Some(22_001),
            Box::new(move |p, n| {
                let b = first_op_block(p, n);
                blk_mut(p, b).operations.pop();
            }),
        ),
        (
            "op-missing",
            Some(22_004),
            Box::new(move |p, n| {
                let b = first_op_block(p, n);
                blk_mut(p, b).operations.push(fresh(4));
            }),
        ),
        (
            "op-dup",
            Some(22_000),
            Box::new(move |p, n| {
                let b = first_op_block(p, n);
                let id = blk(p, b).operations[0];
                blk_mut(p, b).operations.push(id);
            }),
        ),
        (
            "op-foreign",
            Some(22_004),
            Box::new(move |p, n| {
                let sb = find_block(p, spin(n), |b| !b.operations.is_empty());
                let id = blk(p, sb).operations[0];
                let b = first_op_block(p, n);
                blk_mut(p, b).operations.push(id);
            }),
        ),
        (
            "op-result-width",
            Some(21_001),
            Box::new(move |p, n| {
                let b = first_op_block(p, n);
                let id = blk(p, b).operations[0];
                op_mut(p, id).result_types[0] = width(12);
            }),
        ),
        (
            "tparam-ordinal",
            Some(22_003),
            Box::new(move |p, n| {
                fun_mut(p, area(n)).type_parameters = vec![TypeParameterDef { ordinal: 1 }];
            }),
        ),
        ("ptype-width", Some(21_001), set_param_type(width(7))),
        (
            "ptype-unknown",
            Some(21_005),
            set_param_type(named(fresh(5), vec![])),
        ),
        (
            "ptype-tparam",
            Some(21_002),
            set_param_type(TypeExpr::TypeParameter(0)),
        ),
        (
            "ptype-map-vector-key",
            Some(21_011),
            set_param_type(t::map(t::vec(t::u64()), t::u64())),
        ),
        (
            "ptype-map-f64-key",
            Some(21_011),
            set_param_type(t::map(TypeExpr::F64, t::u64())),
        ),
        (
            "ptype-map-fnref-key",
            Some(21_011),
            set_param_type(t::map(fnref.clone(), t::u64())),
        ),
        (
            "ptype-map-cell-key",
            Some(21_011),
            set_param_type(t::map(TypeExpr::LocalCell(Box::new(t::u64())), t::u64())),
        ),
        ("ptype-map-tparam-key", Some(21_011), {
            let ty = t::map(TypeExpr::TypeParameter(0), t::u64());
            Box::new(move |p, n| {
                fun_mut(p, area(n)).type_parameters = vec![TypeParameterDef { ordinal: 0 }];
                let id = fun(p, area(n)).parameters[1];
                prm_mut(p, id).value_type = ty.clone();
            })
        }),
        (
            "ptype-map-record-key-ok",
            None,
            set_param_type(t::map(pt.clone(), t::u64())),
        ),
        (
            "ptype-map-nested-bad",
            Some(21_011),
            set_param_type(t::vec(t::opt(t::map(
                t::tuple(vec![t::u64(), TypeExpr::F32]),
                t::u64(),
            )))),
        ),
        (
            "ptype-arity",
            Some(21_004),
            set_param_type(named(seed::type_id("ck-mut", "Pt"), vec![t::u64()])),
        ),
        (
            "ptype-argument-limit",
            Some(21_003),
            set_param_type(named(seed::type_id("ck-mut", "Pt"), vec![t::u64(); 1025])),
        ),
        (
            "result-type",
            Some(22_008),
            Box::new(move |p, n| {
                fun_mut(p, area(n)).result_type = t::u64();
            }),
        ),
        (
            "result-type-bad",
            Some(21_005),
            Box::new(move |p, n| {
                fun_mut(p, area(n)).result_type = named(fresh(6), vec![]);
            }),
        ),
        (
            "cond-non-bool",
            Some(22_009),
            Box::new(move |p, n| {
                let b = find_block(p, area(n), |b| {
                    matches!(b.terminator, Terminator::CondBranch(_))
                });
                let np = fun(p, area(n)).parameters[1];
                if let Terminator::CondBranch(c) = &mut blk_mut(p, b).terminator {
                    c.condition = pv(np);
                }
            }),
        ),
        (
            "branch-arg-count",
            Some(22_007),
            Box::new(move |p, n| {
                let b = find_block(
                    p,
                    area(n),
                    |b| matches!(&b.terminator, Terminator::Branch(x) if !x.edge.arguments.is_empty()),
                );
                if let Terminator::Branch(x) = &mut blk_mut(p, b).terminator {
                    x.edge.arguments.pop();
                }
            }),
        ),
        (
            "branch-arg-type",
            Some(22_007),
            Box::new(move |p, n| {
                let b = find_block(
                    p,
                    area(n),
                    |b| matches!(&b.terminator, Terminator::Branch(x) if !x.edge.arguments.is_empty()),
                );
                let s = fun(p, area(n)).parameters[0];
                if let Terminator::Branch(x) = &mut blk_mut(p, b).terminator {
                    x.edge.arguments[0] = pv(s);
                }
            }),
        ),
        (
            "branch-target-invalid",
            Some(22_006),
            Box::new(move |p, n| {
                let b = find_block(p, area(n), |b| {
                    matches!(b.terminator, Terminator::Branch(_))
                });
                if let Terminator::Branch(x) = &mut blk_mut(p, b).terminator {
                    x.edge.target = fresh(7);
                }
            }),
        ),
        (
            "branch-target-foreign",
            Some(22_006),
            Box::new(move |p, n| {
                let e = entry(p, spin(n));
                let b = find_block(p, area(n), |b| {
                    matches!(b.terminator, Terminator::Branch(_))
                });
                if let Terminator::Branch(x) = &mut blk_mut(p, b).terminator {
                    x.edge.target = e;
                }
            }),
        ),
        (
            "self-use",
            Some(22_015),
            Box::new(move |p, n| {
                let b = first_op_block(p, n);
                let id = blk(p, b).operations[0];
                op_mut(p, id).operands.push(ov(id, 0));
            }),
        ),
        (
            "dominance-op",
            Some(22_016),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                let e_op = blk(p, e).operations[0];
                let other = find_block(p, area(n), |b| {
                    b.entity_id != e
                        && !b.operations.is_empty()
                        && b.reachability == Reachability::Required
                });
                let o = blk(p, other).operations[0];
                op_mut(p, e_op).operands.push(ov(o, 0));
            }),
        ),
        (
            "dominance-block-param",
            Some(22_016),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                let e_op = blk(p, e).operations[0];
                let b = some_param_block(p, n);
                let bp = blk(p, b).parameters[0];
                op_mut(p, e_op).operands.push(pv(bp));
            }),
        ),
        (
            "value-unresolved-param",
            Some(22_013),
            Box::new(move |p, n| {
                let b = first_op_block(p, n);
                let id = blk(p, b).operations[0];
                op_mut(p, id).operands.push(pv(fresh(8)));
            }),
        ),
        (
            "value-unresolved-op",
            Some(22_013),
            Box::new(move |p, n| {
                let b = first_op_block(p, n);
                let id = blk(p, b).operations[0];
                op_mut(p, id).operands.push(ov(fresh(9), 0));
            }),
        ),
        (
            "value-foreign-param",
            Some(22_013),
            Box::new(move |p, n| {
                let sp = fun(p, spin(n)).parameters[0];
                let b = first_op_block(p, n);
                let id = blk(p, b).operations[0];
                op_mut(p, id).operands.push(pv(sp));
            }),
        ),
        (
            "result-index",
            Some(22_014),
            Box::new(move |p, n| {
                let b = find_block(p, area(n), |b| {
                    matches!(b.terminator, Terminator::Return(_)) && !b.operations.is_empty()
                });
                let o = blk(p, b).operations[0];
                blk_mut(p, b).terminator = ret(ov(o, 3));
            }),
        ),
        (
            "unreachable-value",
            Some(22_018),
            Box::new(move |p, n| {
                let dead = find_block(p, spin(n), |b| {
                    b.reachability == Reachability::ExplicitlyUnreachable
                });
                let live = find_block(p, spin(n), |b| {
                    b.reachability == Reachability::Required && !b.operations.is_empty()
                });
                let o = blk(p, live).operations[0];
                blk_mut(p, dead).terminator = Terminator::Trap(TrapTerminator {
                    code: TrapCode::Unreachable,
                    payload: Some(ov(o, 0)),
                });
            }),
        ),
        (
            "unreachable-block-param-use",
            Some(22_018),
            Box::new(move |p, n| {
                let dead = find_block(p, spin(n), |b| {
                    b.reachability == Reachability::ExplicitlyUnreachable
                });
                let live = find_block(p, spin(n), |b| !b.parameters.is_empty());
                let bp = blk(p, live).parameters[0];
                blk_mut(p, dead).terminator = Terminator::Trap(TrapTerminator {
                    code: TrapCode::Unreachable,
                    payload: Some(pv(bp)),
                });
            }),
        ),
        (
            "unreachable-function-param-ok",
            None,
            Box::new(move |p, n| {
                let dead = find_block(p, spin(n), |b| {
                    b.reachability == Reachability::ExplicitlyUnreachable
                });
                let x = fun(p, spin(n)).parameters[0];
                blk_mut(p, dead).terminator = Terminator::Trap(TrapTerminator {
                    code: TrapCode::Unreachable,
                    payload: Some(pv(x)),
                });
            }),
        ),
        (
            "reachable-marked-unreachable",
            Some(22_017),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                let b = find_block(p, area(n), |b| b.entity_id != e);
                blk_mut(p, b).reachability = Reachability::ExplicitlyUnreachable;
            }),
        ),
        (
            "unreachable-marked-required",
            Some(22_017),
            Box::new(move |p, n| {
                let dead = find_block(p, spin(n), |b| {
                    b.reachability == Reachability::ExplicitlyUnreachable
                });
                blk_mut(p, dead).reachability = Reachability::Required;
            }),
        ),
        (
            "switch-drop-case-unreachable-target",
            Some(22_017),
            Box::new(move |p, n| {
                // The dropped case's target becomes unreachable (still Required).
                let e = entry(p, area(n));
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, e).terminator {
                    s.cases.pop();
                }
            }),
        ),
        (
            "switch-duplicate-key",
            Some(22_011),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, e).terminator {
                    s.cases[2].case_key = s.cases[1].case_key;
                }
            }),
        ),
        (
            "switch-extra-case",
            Some(22_011),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, e).terminator {
                    let mut extra = s.cases[0].clone();
                    extra.case_key = CaseKey::Member(MemberId::from_bytes([0xff; 32]));
                    s.cases.push(extra);
                }
            }),
        ),
        (
            "switch-swap-cases",
            Some(22_011),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, e).terminator {
                    s.cases.swap(0, 1);
                }
            }),
        ),
        (
            "switch-builtin-key",
            Some(22_011),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, e).terminator {
                    s.cases[0].case_key = CaseKey::Builtin(BuiltinCase::None);
                }
            }),
        ),
        (
            "switch-payload-on-none",
            Some(22_012),
            Box::new(move |p, n| {
                let b = find_block(p, area(n), |b| match &b.terminator {
                    Terminator::VariantSwitch(s) => s.cases.iter().any(|c| {
                        c.case_key == CaseKey::Builtin(BuiltinCase::None)
                            && c.edge
                                .arguments
                                .iter()
                                .any(|a| matches!(a, SwitchArgument::Value(_)))
                    }),
                    _ => false,
                });
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, b).terminator {
                    let c = s
                        .cases
                        .iter_mut()
                        .find(|c| c.case_key == CaseKey::Builtin(BuiltinCase::None))
                        .unwrap();
                    let k = c
                        .edge
                        .arguments
                        .iter()
                        .position(|a| matches!(a, SwitchArgument::Value(_)))
                        .unwrap();
                    c.edge.arguments[k] = SwitchArgument::CasePayload;
                }
            }),
        ),
        (
            "switch-on-u64",
            Some(22_010),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                let np = fun(p, area(n)).parameters[1];
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, e).terminator {
                    s.value = pv(np);
                }
            }),
        ),
        ("switch-on-record", Some(22_010), {
            let pt = pt.clone();
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                let np = fun(p, area(n)).parameters[1];
                prm_mut(p, np).value_type = pt.clone();
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, e).terminator {
                    s.value = pv(np);
                }
            })
        }),
        (
            "switch-payload-type",
            Some(22_007),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                let Terminator::VariantSwitch(s) = &blk(p, e).terminator else {
                    panic!()
                };
                let c = s
                    .cases
                    .iter()
                    .find(|c| c.edge.arguments.first() == Some(&SwitchArgument::CasePayload))
                    .unwrap();
                let target = c.edge.target;
                let bp = blk(p, target).parameters[0];
                prm_mut(p, bp).value_type = t::u32();
            }),
        ),
        (
            "switch-arg-count",
            Some(22_007),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, e).terminator {
                    s.cases[0].edge.arguments.push(SwitchArgument::CasePayload);
                }
            }),
        ),
        (
            "switch-target-invalid",
            Some(22_006),
            Box::new(move |p, n| {
                let e = entry(p, area(n));
                if let Terminator::VariantSwitch(s) = &mut blk_mut(p, e).terminator {
                    s.cases[1].edge.target = fresh(10);
                }
            }),
        ),
        (
            "trap-payload-cell",
            Some(22_019),
            Box::new(move |p, n| {
                let o = trap_payload_op(p, n);
                op_mut(p, o).result_types = vec![TypeExpr::LocalCell(Box::new(t::u64()))];
            }),
        ),
        (
            "trap-payload-adapter",
            Some(22_019),
            Box::new(move |p, n| {
                let o = trap_payload_op(p, n);
                op_mut(p, o).result_types = vec![TypeExpr::AdapterHandle(fresh(11))];
            }),
        ),
        (
            "trap-payload-tparam",
            Some(22_019),
            Box::new(move |p, n| {
                fun_mut(p, area(n)).type_parameters = vec![TypeParameterDef { ordinal: 0 }];
                let o = trap_payload_op(p, n);
                op_mut(p, o).result_types = vec![TypeExpr::TypeParameter(0)];
            }),
        ),
        (
            "trap-payload-map-ok",
            None,
            Box::new(move |p, n| {
                let o = trap_payload_op(p, n);
                op_mut(p, o).result_types = vec![t::map(t::bytes(), t::vec(t::u64()))];
            }),
        ),
        (
            "declared-order-shuffle",
            None,
            Box::new(move |p, n| {
                let f = fun_mut(p, area(n));
                f.blocks.reverse();
            }),
        ),
    ]
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn function_mutations_match_native() {
    let (program, names) = base();
    let mut corpus = Corpus::default();
    for (name, expected, mutate) in function_mutations() {
        let mut p = program.clone();
        mutate(&mut p, &names);
        match expected {
            Some(code) => corpus.expect(name, &p, code),
            None => corpus.check(name, &p),
        }
    }
    eprintln!(
        "function mutations: {} comparisons, native codes {:?}",
        corpus.cases, corpus.codes
    );
    assert!(corpus.mismatches.is_empty(), "{:#?}", corpus.mismatches);
}

// ----- raw graphs -----

/// A hand-built program extension.
struct Raw {
    p: Program,
    n: u32,
}

impl Raw {
    fn new(p: Program) -> Self {
        Self { p, n: 0 }
    }
    fn id(&mut self) -> EntityId {
        self.n += 1;
        fresh(1_000_000 + self.n)
    }
    fn func(&mut self, params: &[TypeExpr], result: TypeExpr) -> (EntityId, Vec<EntityId>) {
        let fid = self.id();
        let mut ids = Vec::new();
        for (ordinal, ty) in params.iter().enumerate() {
            let id = self.id();
            self.p.parameters.push(Parameter {
                entity_id: id,
                owner: fid,
                role: ParameterRole::Function,
                ordinal: u32::try_from(ordinal).unwrap(),
                value_type: ty.clone(),
            });
            ids.push(id);
        }
        self.p.functions.push(FunctionGraph {
            entity_id: fid,
            type_parameters: vec![],
            parameters: ids.clone(),
            result_type: result,
            effects: vec![],
            entry_block: fid,
            blocks: vec![],
            contracts: vec![],
            visibility: Visibility::Private,
        });
        (fid, ids)
    }
    fn block(
        &mut self,
        fid: EntityId,
        params: &[TypeExpr],
        reach: Reachability,
    ) -> (EntityId, Vec<EntityId>) {
        let bid = self.id();
        let mut ids = Vec::new();
        for (ordinal, ty) in params.iter().enumerate() {
            let id = self.id();
            self.p.parameters.push(Parameter {
                entity_id: id,
                owner: bid,
                role: ParameterRole::Block,
                ordinal: u32::try_from(ordinal).unwrap(),
                value_type: ty.clone(),
            });
            ids.push(id);
        }
        self.p.blocks.push(Block {
            entity_id: bid,
            function: fid,
            parameters: ids.clone(),
            operations: vec![],
            terminator: trap0(),
            reachability: reach,
        });
        let f = fun_mut(&mut self.p, fid);
        if f.blocks.is_empty() {
            f.entry_block = bid;
        }
        f.blocks.push(bid);
        (bid, ids)
    }
    fn op(&mut self, bid: EntityId, operands: Vec<ValueRef>, results: Vec<TypeExpr>) -> EntityId {
        let id = self.id();
        let b = blk_mut(&mut self.p, bid);
        let ordinal = u32::try_from(b.operations.len()).unwrap();
        b.operations.push(id);
        self.p.operations.push(Operation {
            entity_id: id,
            block: bid,
            ordinal,
            opcode: Opcode::TupleNew,
            operands,
            result_types: results,
            immediate: Immediate::None,
        });
        id
    }
    fn term(&mut self, bid: EntityId, terminator: Terminator) {
        blk_mut(&mut self.p, bid).terminator = terminator;
    }
    fn typedef(&mut self, tparams: u32, form: TypeDefForm) -> EntityId {
        let id = self.id();
        self.p.type_definitions.push(TypeDefinition {
            entity_id: id,
            type_parameters: (0..tparams)
                .map(|ordinal| TypeParameterDef { ordinal })
                .collect(),
            form,
            invariants: vec![],
            visibility: Visibility::Private,
        });
        id
    }
    fn member(&mut self) -> MemberId {
        MemberId::from_bytes(*self.id().as_bytes())
    }
}

/// A minimal program with one trivial function.
fn tiny() -> Program {
    let mut m = Module::new("ck-tiny");
    m.func("main", &[], t::u64(), |f| f.ret(u64c(1)));
    m.finish().0
}

fn record(fields: Vec<(MemberId, TypeExpr)>) -> TypeDefForm {
    TypeDefForm::Record(
        fields
            .into_iter()
            .map(|(member_id, value_type)| RecordField {
                member_id,
                value_type,
                visibility: Visibility::Private,
            })
            .collect(),
    )
}

fn variant(cases: Vec<(MemberId, Option<TypeExpr>)>) -> TypeDefForm {
    TypeDefForm::Variant(
        cases
            .into_iter()
            .map(|(member_id, payload_type)| VariantCase {
                member_id,
                payload_type,
            })
            .collect(),
    )
}

/// `entry -> x(n-1) -> ... -> x(0) -> return`, declared `[entry, x0 ..
/// x(n-1)]` so every chain edge retreats, plus `pad` unreachable blocks.
fn chain(n: usize, pad: usize) -> Program {
    let mut r = Raw::new(tiny());
    let (fid, ps) = r.func(&[TypeExpr::Unit], TypeExpr::Unit);
    let (entry, _) = r.block(fid, &[], Reachability::Required);
    let xs: Vec<EntityId> = (0..n)
        .map(|_| r.block(fid, &[], Reachability::Required).0)
        .collect();
    r.term(entry, br(xs[n - 1], vec![]));
    for k in (1..n).rev() {
        r.term(xs[k], br(xs[k - 1], vec![]));
    }
    r.term(xs[0], ret(pv(ps[0])));
    for _ in 0..pad {
        r.block(fid, &[], Reachability::ExplicitlyUnreachable);
    }
    r.p
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn dominator_budget_matches_native() {
    let mut corpus = Corpus::default();
    // Under budget through the cheap bound; exactly at the 50M budget
    // (1000 rounds x 50,000 words, exact fallback); one word over.
    corpus.check("chain-1000", &chain(1000, 0));
    corpus.check("chain-1000-pad-550", &chain(1000, 550));
    corpus.expect("chain-1000-pad-600", &chain(1000, 600), 22_020);
    corpus.expect("chain-2000", &chain(2000, 0), 22_020);
    // Too many blocks.
    corpus.expect("blocks-4097", &chain(1, 4096), 22_020);
    eprintln!(
        "budget: {} comparisons, native codes {:?}",
        corpus.cases, corpus.codes
    );
    assert!(corpus.mismatches.is_empty(), "{:#?}", corpus.mismatches);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn edge_budget_matches_native() {
    let mut corpus = Corpus::default();
    for (blocks, code) in [(1600, None), (1700, Some(22_020))] {
        let mut r = Raw::new(tiny());
        let cases: Vec<MemberId> = (0..10).map(|_| r.member()).collect();
        let v10 = r.typedef(0, variant(cases.iter().map(|c| (*c, None)).collect()));
        let (fid, ps) = r.func(&[named(v10, vec![])], t::u64());
        let bs: Vec<EntityId> = (0..=blocks)
            .map(|_| r.block(fid, &[], Reachability::Required).0)
            .collect();
        let mut sorted = cases.clone();
        sorted.sort();
        for k in 0..blocks {
            r.term(
                bs[k],
                Terminator::VariantSwitch(VariantSwitchTerminator {
                    value: pv(ps[0]),
                    cases: sorted
                        .iter()
                        .map(|c| SwitchCase {
                            case_key: CaseKey::Member(*c),
                            edge: SwitchEdge {
                                target: bs[k + 1],
                                arguments: vec![],
                            },
                        })
                        .collect(),
                }),
            );
        }
        let last = bs[blocks];
        let o = r.op(last, vec![], vec![t::u64()]);
        r.term(last, ret(ov(o, 0)));
        let name = format!("switch-chain-{blocks}");
        match code {
            Some(code) => corpus.expect(&name, &r.p, code),
            None => corpus.check(&name, &r.p),
        }
    }
    eprintln!(
        "edges: {} comparisons, native codes {:?}",
        corpus.cases, corpus.codes
    );
    assert!(corpus.mismatches.is_empty(), "{:#?}", corpus.mismatches);
}

/// Generic definitions and a switch on an instantiated variant.
#[allow(clippy::too_many_lines)]
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn generic_switches_match_native() {
    let mut corpus = Corpus::default();
    struct G {
        r: Raw,
        boxed: EntityId,
        maybe: EntityId,
        no: MemberId,
        yes: MemberId,
    }
    let setup = || {
        let mut r = Raw::new(tiny());
        let field = r.member();
        let boxed = r.typedef(1, record(vec![(field, TypeExpr::TypeParameter(0))]));
        let no = r.member();
        let yes = r.member();
        let maybe = r.typedef(
            1,
            variant(vec![(no, None), (yes, Some(TypeExpr::TypeParameter(0)))]),
        );
        G {
            r,
            boxed,
            maybe,
            no,
            yes,
        }
    };
    // switch x: Maybe<arg> { No -> return d, Yes(p: payload) -> return conv }
    let build =
        |g: &mut G, arg: TypeExpr, payload_param: TypeExpr, tparams: u32, result: TypeExpr| {
            let (fid, ps) =
                g.r.func(&[named(g.maybe, vec![arg]), result.clone()], result);
            fun_mut(&mut g.r.p, fid).type_parameters = (0..tparams)
                .map(|ordinal| TypeParameterDef { ordinal })
                .collect();
            let (e, _) = g.r.block(fid, &[], Reachability::Required);
            let (b_no, _) = g.r.block(fid, &[], Reachability::Required);
            let (b_yes, yp) = g.r.block(fid, &[payload_param], Reachability::Required);
            let mut cases = vec![
                SwitchCase {
                    case_key: CaseKey::Member(g.no),
                    edge: SwitchEdge {
                        target: b_no,
                        arguments: vec![],
                    },
                },
                SwitchCase {
                    case_key: CaseKey::Member(g.yes),
                    edge: SwitchEdge {
                        target: b_yes,
                        arguments: vec![SwitchArgument::CasePayload],
                    },
                },
            ];
            cases.sort_by_key(|c| c.case_key);
            g.r.term(
                e,
                Terminator::VariantSwitch(VariantSwitchTerminator {
                    value: pv(ps[0]),
                    cases,
                }),
            );
            g.r.term(b_no, ret(pv(ps[1])));
            g.r.term(
                b_yes,
                Terminator::Trap(TrapTerminator {
                    code: TrapCode::Unreachable,
                    payload: Some(pv(yp[0])),
                }),
            );
            fid
        };
    {
        let mut g = setup();
        build(&mut g, t::u64(), t::u64(), 0, t::u64());
        corpus.check("maybe-u64", &g.r.p);
    }
    {
        let mut g = setup();
        build(&mut g, t::u64(), t::u32(), 0, t::u64());
        corpus.expect("maybe-u64-payload-u32", &g.r.p, 22_007);
    }
    {
        let mut g = setup();
        let bx = named(g.boxed, vec![t::u64()]);
        build(&mut g, bx.clone(), bx, 0, t::u64());
        corpus.check("maybe-box-u64", &g.r.p);
    }
    {
        let mut g = setup();
        let bad = t::map(named(g.boxed, vec![TypeExpr::F64]), t::u64());
        build(&mut g, bad.clone(), bad, 0, t::u64());
        corpus.expect("maybe-map-box-f64-key", &g.r.p, 21_011);
    }
    {
        let mut g = setup();
        let good = t::map(named(g.boxed, vec![t::u64()]), t::u64());
        build(&mut g, good.clone(), good, 0, t::u64());
        corpus.check("maybe-map-box-u64-key", &g.r.p);
    }
    {
        let mut g = setup();
        build(
            &mut g,
            TypeExpr::TypeParameter(0),
            TypeExpr::TypeParameter(0),
            1,
            t::u64(),
        );
        // The trap payload contains a type parameter.
        corpus.expect("maybe-generic-trap", &g.r.p, 22_019);
    }
    {
        let mut g = setup();
        let fid = build(&mut g, t::u64(), t::u64(), 0, t::u64());
        let maybe = g.maybe;
        let x = fun(&g.r.p, fid).parameters[0];
        prm_mut(&mut g.r.p, x).value_type = named(maybe, vec![]);
        corpus.expect("maybe-arity", &g.r.p, 21_004);
    }
    {
        let mut g = setup();
        let nested = named(g.maybe, vec![named(g.boxed, vec![t::vec(t::u64())])]);
        build(&mut g, nested.clone(), nested, 0, t::u64());
        corpus.check("maybe-nested", &g.r.p);
    }
    {
        // A generic record whose instantiated field is a non-orderable key.
        let mut g = setup();
        let m2 = g.r.member();
        let keyed = g.r.typedef(
            1,
            record(vec![(m2, t::map(TypeExpr::TypeParameter(0), t::u64()))]),
        );
        build(
            &mut g,
            named(keyed, vec![t::vec(t::u64())]),
            t::u64(),
            0,
            t::u64(),
        );
        corpus.expect("keyed-vector-key", &g.r.p, 21_011);
    }
    eprintln!(
        "generics: {} comparisons, native codes {:?}",
        corpus.cases, corpus.codes
    );
    assert!(corpus.mismatches.is_empty(), "{:#?}", corpus.mismatches);
}

#[allow(clippy::too_many_lines)]
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn type_environment_mutations_match_native() {
    let mut corpus = Corpus::default();
    let with = |f: &dyn Fn(&mut Raw)| {
        let mut r = Raw::new(tiny());
        f(&mut r);
        r.p
    };
    corpus.check("tiny", &tiny());
    corpus.expect(
        "member-duplicate",
        &with(&|r| {
            let m = r.member();
            r.typedef(0, record(vec![(m, t::u64()), (m, t::u32())]));
        }),
        21_008,
    );
    corpus.expect(
        "tparam-ordinal",
        &with(&|r| {
            let m = r.member();
            let id = r.typedef(2, record(vec![(m, t::u64())]));
            r.p.type_definitions
                .iter_mut()
                .find(|d| d.entity_id == id)
                .unwrap()
                .type_parameters[1]
                .ordinal = 5;
        }),
        21_002,
    );
    corpus.expect(
        "member-tparam-scope",
        &with(&|r| {
            let m = r.member();
            r.typedef(1, record(vec![(m, TypeExpr::TypeParameter(1))]));
        }),
        21_002,
    );
    corpus.expect(
        "member-unknown",
        &with(&|r| {
            let m = r.member();
            r.typedef(0, variant(vec![(m, Some(named(fresh(77), vec![])))]));
        }),
        21_005,
    );
    corpus.expect(
        "member-arity",
        &with(&|r| {
            let m = r.member();
            let a = r.typedef(0, record(vec![(m, t::u64())]));
            let m2 = r.member();
            r.typedef(0, record(vec![(m2, named(a, vec![t::u64()]))]));
        }),
        21_004,
    );
    corpus.expect(
        "member-width",
        &with(&|r| {
            let m = r.member();
            r.typedef(0, record(vec![(m, width(24))]));
        }),
        21_001,
    );
    corpus.expect(
        "self-cycle",
        &with(&|r| {
            let m = r.member();
            let id = r.id();
            r.p.type_definitions.push(TypeDefinition {
                entity_id: id,
                type_parameters: vec![],
                form: record(vec![(m, t::opt(named(id, vec![])))]),
                invariants: vec![],
                visibility: Visibility::Private,
            });
        }),
        21_007,
    );
    corpus.expect(
        "mutual-cycle",
        &with(&|r| {
            let (a, b) = (r.id(), r.id());
            let (ma, mb) = (r.member(), r.member());
            for (id, m, other) in [(a, ma, b), (b, mb, a)] {
                r.p.type_definitions.push(TypeDefinition {
                    entity_id: id,
                    type_parameters: vec![],
                    form: variant(vec![(m, Some(t::vec(named(other, vec![]))))]),
                    invariants: vec![],
                    visibility: Visibility::Private,
                });
            }
        }),
        21_007,
    );
    // Chains of nested definitions: deep ones exceed the walk depth.
    for (len, code) in [(40, None), (63, None), (64, None), (70, Some(21_000))] {
        let p = with(&|r| {
            let mut prev = t::u64();
            for _ in 0..len {
                let m = r.member();
                let id = r.typedef(0, record(vec![(m, prev.clone())]));
                prev = named(id, vec![]);
            }
        });
        match code {
            Some(code) => corpus.expect(&format!("chain-{len}"), &p, code),
            None => corpus.check(&format!("chain-{len}"), &p),
        }
    }
    corpus.expect(
        "member-map-vector-key",
        &with(&|r| {
            let m = r.member();
            r.typedef(0, record(vec![(m, t::map(t::vec(t::u64()), t::u64()))]));
        }),
        21_011,
    );
    corpus.expect(
        "member-map-generic-key",
        &with(&|r| {
            let m = r.member();
            let bx = r.typedef(1, record(vec![(m, TypeExpr::TypeParameter(0))]));
            let m2 = r.member();
            r.typedef(
                0,
                record(vec![(m2, t::map(named(bx, vec![TypeExpr::F32]), t::u64()))]),
            );
        }),
        21_011,
    );
    corpus.check(
        "member-map-generic-key-ok",
        &with(&|r| {
            let m = r.member();
            let bx = r.typedef(1, record(vec![(m, TypeExpr::TypeParameter(0))]));
            let m2 = r.member();
            r.typedef(
                0,
                record(vec![(m2, t::map(named(bx, vec![t::u64()]), t::u64()))]),
            );
        }),
    );
    corpus.check(
        "member-map-parameter-key-skipped",
        &with(&|r| {
            // A map keyed by a parameter inside a generic definition is judged
            // only when instantiated; a Named argument containing a parameter
            // is skipped by the map-key walk.
            let m = r.member();
            let bx = r.typedef(1, record(vec![(m, TypeExpr::TypeParameter(0))]));
            let m2 = r.member();
            r.typedef(
                1,
                record(vec![(
                    m2,
                    named(bx, vec![t::map(TypeExpr::TypeParameter(0), t::u64())]),
                )]),
            );
        }),
    );
    corpus.expect(
        "member-map-direct-parameter-key",
        &with(&|r| {
            let m = r.member();
            r.typedef(
                1,
                record(vec![(m, t::map(TypeExpr::TypeParameter(0), t::u64()))]),
            );
        }),
        21_011,
    );
    corpus.expect(
        "argument-limit",
        &with(&|r| {
            let m = r.member();
            let a = r.typedef(0, record(vec![(m, t::u64())]));
            let m2 = r.member();
            r.typedef(0, record(vec![(m2, named(a, vec![t::u64(); 1025]))]));
        }),
        21_003,
    );
    eprintln!(
        "types: {} comparisons, native codes {:?}",
        corpus.cases, corpus.codes
    );
    assert!(corpus.mismatches.is_empty(), "{:#?}", corpus.mismatches);
}
