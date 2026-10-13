//! `lw_image` against the native `sley_vm::lower_function` (`EXTENDED_V1)`:
//! byte-for-byte images and refusal codes.
#![allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
mod common;
use common::*;
use sley_id::EntityId;
use sley_selfhost::canonical::{self, CanonicalS};
use sley_selfhost::native;
use sley_selfhost::seed::*;
use sley_selfhost::toolchain::types::scb_type_bytes;
use sley_ssmc::{
    BuiltinFailureKind, FunctionType, Immediate, IntegerWidth, NamedType, Opcode, TypeExpr,
};
use sley_vm::LoweringError;
use std::time::{Duration, Instant};

/// Runs `t_lw_image` over `s` for `entry`.
fn sley_image(s: &CanonicalS, entry: EntityId) -> (Result<Vec<u8>, u32>, u64, Duration) {
    let start = Instant::now();
    let (value, instructions) = run_counted(
        "t_lw_image",
        vec![
            objects_input(s),
            bytes(s.epoch.as_bytes()),
            bytes(entry.as_bytes()),
        ],
    );
    let elapsed = start.elapsed();
    (result(value).map(|v| as_bytes(&v)), instructions, elapsed)
}

/// The native image (or numeric lowering refusal).
fn native_image(program: &Program, entry: EntityId) -> Result<Vec<u8>, u32> {
    match native::lower(program, entry) {
        Ok(lowered) => Ok(lowered.bytes),
        Err(LoweringError::Lower(error)) => Err(error.code().numeric()),
        Err(LoweringError::Cfg(error)) => panic!("native CFG refusal: {error}"),
    }
}

fn compare(label: &str, program: &Program, s: &CanonicalS, entry: EntityId) -> u64 {
    let start = Instant::now();
    let expected = native_image(program, entry);
    let native_elapsed = start.elapsed();
    // LW_MEASURE=1 also times `drv_load` alone, so the lowering share of the
    // instruction count and wall time can be read off.
    if std::env::var_os("LW_MEASURE").is_some() {
        let start = Instant::now();
        let (_, load_instructions) = run_counted(
            "t_load_counts",
            vec![objects_input(s), bytes(s.epoch.as_bytes())],
        );
        let load_elapsed = start.elapsed();
        eprintln!(
            "{label}: native {native_elapsed:?}; load-only {load_instructions} instructions, {load_elapsed:?}"
        );
    }
    let (got, instructions, elapsed) = sley_image(s, entry);
    if let (Ok(e), Ok(g)) = (&expected, &got) {
        if e != g {
            let at = e
                .iter()
                .zip(g)
                .position(|(a, b)| a != b)
                .unwrap_or(e.len().min(g.len()));
            panic!(
                "{label}: image differs at byte {at} (native {} bytes, sley {} bytes)",
                e.len(),
                g.len()
            );
        }
        eprintln!(
            "{label}: {} bytes, {instructions} instructions, {elapsed:?}",
            g.len()
        );
    } else {
        assert_eq!(got, expected, "{label}: outcome");
        eprintln!("{label}: refusal {got:?}, {instructions} instructions");
    }
    instructions
}

/// Canonical S of the toolchain narrowed to `entry`'s call closure (the
/// driver's shape; lowering reads only the closure, and loading the whole
/// toolchain S costs ~190M VM instructions in `drv_load`).
fn closure_s(entry: EntityId) -> CanonicalS {
    let (program, _) = program();
    canonical::build(
        &canonical::prune_to_closure(program, entry),
        entry,
        "toolchain-test",
    )
}

#[test]
fn small_toolchain_closures_match_native() {
    let (program, names) = program();
    for entry in ["rt_b", "be32", "rt_rd_uvar", "scb_walk"] {
        compare(entry, program, &closure_s(names[entry]), names[entry]);
    }
}

#[test]
fn large_toolchain_closures_match_native() {
    let (program, names) = program();
    for entry in ["cd_object", "drv_load", "t_lw_image"] {
        compare(entry, program, &closure_s(names[entry]), names[entry]);
    }
}

/// A seed program exercising every terminator kind, switches with payloads,
/// records/variants, calls (including self-reference), and bridges.
fn seed_program() -> (Program, std::collections::HashMap<String, EntityId>) {
    let mut m = Module::new("lw-seed");
    m.record("Pt", &[("x", t::u64()), ("y", t::u64())]);
    m.variant(
        "Shape",
        &[
            ("Dot", None),
            ("Line", Some(t::u64())),
            ("Box", Some(m.ty("Pt"))),
        ],
    );
    m.func("inc", &[("x", t::u64())], t::u64(), |f| {
        f.ret(add(v("x"), u64c(1)));
    });
    m.func("area", &[("s", m.ty("Shape"))], t::u64(), |f| {
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
                    Box::new(|f: &mut FnB<'_>| f.set("out", call("inc", vec![v("len")]))),
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
        f.ret(v("out"));
    });
    m.func(
        "opt_res",
        &[("o", t::opt(t::u64())), ("r", t::res(t::bytes(), t::u32()))],
        t::res(t::tuple(vec![t::u64(), t::bytes()]), t::u32()),
        |f| {
            f.var("a", u64c(0));
            f.match_opt(
                v("o"),
                "x",
                |f| f.set("a", v("x")),
                sley_selfhost::seed::FnB::trap,
            );
            f.let_("b", tri(v("r")));
            f.var("m", mnew(t::bytes(), t::u64(), vec![]));
            f.set("m", mins(v("m"), v("b"), v("a")));
            f.ret_ok(tup(vec![
                mget(v("m"), v("b")),
                concat(vnew(t::bytes(), vec![v("b"), v("b")])),
            ]));
        },
    );
    m.func(
        "main",
        &[("b", t::bytes()), ("n", t::u64())],
        t::bytes(),
        |f| {
            f.let_("src", b2v(v("b")));
            f.var("out", vnew(t::u8(), vec![]));
            f.var("i", u64c(0));
            f.loop_(|f| {
                f.if_(ge(v("i"), vlen(v("src"))), sley_selfhost::seed::FnB::break_);
                f.let_("x", vget(v("src"), v("i")));
                f.set("out", push(v("out"), add(v("x"), u8c(1))));
                f.set("i", add(v("i"), u64c(1)));
            });
            f.let_(
                "sh",
                var_new("Shape", "Box", Some(rec("Pt", vec![v("n"), u64c(7)]))),
            );
            f.let_("a", call("area", vec![v("sh")]));
            f.if_else(
                is_case(v("sh"), "Shape", "Line"),
                sley_selfhost::seed::FnB::trap,
                |_| {},
            );
            f.let_(
                "r",
                call("opt_res", vec![some(v("a")), ok(v2b(v("out")), t::u32())]),
            );
            f.ret(select(
                is_ok(v("r")),
                tget(unwrap(v("r")), 1),
                blake3(v("b")),
            ));
        },
    );
    // An infinite loop (trap-without-payload exit) and a dead path.
    m.func("spin", &[("n", t::u64())], t::u64(), |f| {
        f.var("k", v("n"));
        f.loop_(|f| {
            f.if_(eq(v("k"), u64c(0)), |f| f.ret(u64c(5)));
            f.set("k", sub(v("k"), u64c(1)));
        });
    });
    m.func("pong", &[("n", t::u64())], t::u64(), |f| {
        f.ret(call("inc", vec![v("n")]));
    });
    m.func("ping", &[("n", t::u64())], t::u64(), |f| {
        f.ret(add(call("pong", vec![v("n")]), u64c(1)));
    });
    m.func("top", &[("n", t::u64())], t::u64(), |f| {
        f.let_("s", call("spin", vec![v("n")]));
        f.let_("p", call("ping", vec![v("s")]));
        f.ret(add(
            v("p"),
            call("area", vec![var_new("Shape", "Line", Some(v("n")))]),
        ));
    });
    m.finish()
}

#[test]
fn seed_programs_match_native() {
    let (program, names) = seed_program();
    for entry in ["inc", "area", "opt_res", "main", "spin", "ping", "top"] {
        let s = canonical::build(&program, names[entry], "lw-seed");
        compare(entry, &program, &s, names[entry]);
    }
}

#[test]
fn self_and_mutual_calls_match_native() {
    // Lowering ignores the profile's call-cycle rule: a self-call refers to
    // the entry, and a cycle among callees lowers each callee once.
    let (mut program, names) = seed_program();
    let top = names["top"];
    let inc = names["inc"];
    let ops_of = |program: &Program, function: EntityId| -> Vec<usize> {
        let blocks: Vec<EntityId> = program
            .blocks
            .iter()
            .filter(|b| b.function == function)
            .map(|b| b.entity_id)
            .collect();
        program
            .operations
            .iter()
            .enumerate()
            .filter(|(_, op)| blocks.contains(&op.block) && op.opcode == Opcode::CallDirect)
            .map(|(i, _)| i)
            .collect()
    };
    // `top` calls `spin` then `area`: retarget `spin` -> `top` (self-call).
    let top_calls = ops_of(&program, top);
    if let Immediate::Function(reference) = &mut program.operations[top_calls[0]].immediate {
        assert_eq!(reference.function, names["spin"]);
        reference.function = top;
        program.operations[top_calls[0]].result_types = vec![t::u64()];
    }
    // `pong` calls `inc`: retarget it to `ping` (a callee cycle).
    let pong_calls = ops_of(&program, names["pong"]);
    if let Immediate::Function(reference) = &mut program.operations[pong_calls[0]].immediate {
        assert_eq!(reference.function, inc);
        reference.function = names["ping"];
    }
    let s = canonical::build(&program, top, "lw-seed");
    compare("top (self-call)", &program, &s, top);
}

#[test]
fn profile_and_callee_refusals_match_native() {
    let (base, names) = seed_program();
    // A callee with effects (a generic callee is refused earlier, by the
    // caller's call_direct judgment, which belongs to the jg_ module).
    let mut program = base.clone();
    let callee = names["inc"];
    for function in &mut program.functions {
        if function.entity_id == callee {
            function.effects = vec![EntityId::from_bytes([0x5e; 32])];
        }
    }
    let s = canonical::build(&program, names["area"], "lw-seed");
    compare("callee with effects", &program, &s, names["area"]);
    // The entry itself.
    let s = canonical::build(&program, callee, "lw-seed");
    compare("entry with effects", &program, &s, callee);
}

#[test]
fn large_functions_match_native() {
    let mut m = Module::new("lw-blocks");
    m.func("many", &[("n", t::u64())], t::u64(), |f| {
        f.var("k", v("n"));
        for i in 0..300_u64 {
            f.if_(eq(v("k"), u64c(i)), |f| f.set("k", add(v("k"), u64c(1))));
        }
        f.ret(v("k"));
    });
    m.func("calls_many", &[("n", t::u64())], t::u64(), |f| {
        f.ret(call("many", vec![v("n")]));
    });
    let (program, names) = m.finish();
    let entry = names["many"];
    let s = canonical::build(&program, entry, "lw-blocks");
    compare("~900 blocks (entry)", &program, &s, entry);
    let entry = names["calls_many"];
    let s = canonical::build(&program, entry, "lw-blocks");
    compare("~900 blocks (callee)", &program, &s, entry);
}

// ----- type conversion -----

/// Reference Appendix-A encoding (mirrors `sley_vm::lower::Encoder::type_expr`).
fn appendix_a(ty: &TypeExpr, depth: usize, out: &mut Vec<u8>) -> Result<(), ()> {
    if depth > 64 {
        return Err(());
    }
    out.extend_from_slice(&ty.tag().to_be_bytes());
    let entity = |out: &mut Vec<u8>, id: &EntityId| {
        out.extend_from_slice(&1_u32.to_be_bytes());
        out.extend_from_slice(id.as_bytes());
    };
    match ty {
        TypeExpr::Unit
        | TypeExpr::Bool
        | TypeExpr::F32
        | TypeExpr::F64
        | TypeExpr::Bytes
        | TypeExpr::Text => {}
        TypeExpr::SInt(w) | TypeExpr::UInt(w) => out.extend_from_slice(&w.bits().to_be_bytes()),
        TypeExpr::Tuple(items) => {
            out.extend_from_slice(&(items.len() as u64).to_be_bytes());
            for item in items {
                appendix_a(item, depth + 1, out)?;
            }
        }
        TypeExpr::Named(named) => {
            entity(out, &named.definition);
            out.extend_from_slice(&(named.arguments.len() as u64).to_be_bytes());
            for item in &named.arguments {
                appendix_a(item, depth + 1, out)?;
            }
        }
        TypeExpr::Vector(x) | TypeExpr::Option(x) | TypeExpr::LocalCell(x) => {
            appendix_a(x, depth + 1, out)?;
        }
        TypeExpr::OrderedMap { key, value } => {
            appendix_a(key, depth + 1, out)?;
            appendix_a(value, depth + 1, out)?;
        }
        TypeExpr::Result { ok, error } => {
            appendix_a(ok, depth + 1, out)?;
            appendix_a(error, depth + 1, out)?;
        }
        TypeExpr::FunctionRef(fun) => {
            out.extend_from_slice(&(fun.parameters.len() as u64).to_be_bytes());
            for item in &fun.parameters {
                appendix_a(item, depth + 1, out)?;
            }
            appendix_a(&fun.result, depth + 1, out)?;
            out.extend_from_slice(&(fun.effects.len() as u64).to_be_bytes());
            for effect in &fun.effects {
                entity(out, effect);
            }
        }
        TypeExpr::AdapterHandle(id) | TypeExpr::CapabilityToken(id) => entity(out, id),
        TypeExpr::TypeParameter(index) => out.extend_from_slice(&index.to_be_bytes()),
        TypeExpr::BuiltinFailure(kind) => out.extend_from_slice(&kind.tag().to_be_bytes()),
    }
    Ok(())
}

fn sley_ty(scb: &[u8]) -> Result<Vec<u8>, u32> {
    result(run("t_lw_ty", vec![bytes(scb)])).map(|value| {
        let items = tuple_items(&value);
        let encoded = as_bytes(&items[0]);
        assert_eq!(as_u64(&items[1]), encoded.len() as u64, "reported length");
        encoded
    })
}

#[test]
fn every_type_kind_converts() {
    let id = |b: u8| EntityId::from_bytes([b; 32]);
    let w = IntegerWidth::from_bits;
    let types = vec![
        TypeExpr::Unit,
        TypeExpr::Bool,
        TypeExpr::SInt(w(8)),
        TypeExpr::UInt(w(24)),
        TypeExpr::UInt(w(128)),
        TypeExpr::F32,
        TypeExpr::F64,
        TypeExpr::Bytes,
        TypeExpr::Text,
        TypeExpr::Tuple(vec![]),
        TypeExpr::Tuple(vec![TypeExpr::Bool, TypeExpr::Bytes, t::vec(t::u64())]),
        TypeExpr::Named(NamedType {
            definition: id(3),
            arguments: vec![TypeExpr::Text, t::opt(TypeExpr::Unit)],
        }),
        t::map(
            TypeExpr::Text,
            t::res(
                t::u64(),
                TypeExpr::BuiltinFailure(BuiltinFailureKind::Capability),
            ),
        ),
        TypeExpr::LocalCell(Box::new(TypeExpr::Text)),
        TypeExpr::FunctionRef(FunctionType {
            parameters: vec![TypeExpr::Bool, t::vec(TypeExpr::Text)],
            result: Box::new(t::tuple(vec![TypeExpr::Unit])),
            effects: vec![id(1), id(2)],
        }),
        TypeExpr::FunctionRef(FunctionType {
            parameters: vec![],
            result: Box::new(TypeExpr::Unit),
            effects: vec![],
        }),
        TypeExpr::AdapterHandle(id(16)),
        TypeExpr::CapabilityToken(id(17)),
        TypeExpr::TypeParameter(300),
        TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey),
    ];
    for ty in &types {
        let mut expected = Vec::new();
        appendix_a(ty, 1, &mut expected).unwrap();
        assert_eq!(sley_ty(&scb_type_bytes(ty)), Ok(expected), "{ty:?}");
    }
}

#[test]
fn type_depth_limit() {
    // Hand-built canonical SCB bytes: Option^k(Unit) (tag 13 wraps its
    // element as a sized payload).
    fn uvar(mut x: u64, out: &mut Vec<u8>) {
        while x >= 128 {
            out.push((x % 128) as u8 | 128);
            x /= 128;
        }
        out.push(x as u8);
    }
    let nest = |k: usize| {
        let mut ty = vec![1_u8, 0];
        let mut native = TypeExpr::Unit;
        for _ in 0..k {
            let mut outer = Vec::new();
            uvar(13, &mut outer);
            uvar(ty.len() as u64, &mut outer);
            outer.extend_from_slice(&ty);
            ty = outer;
            native = TypeExpr::Option(Box::new(native));
        }
        (ty, native)
    };
    let (ok_bytes, ok_native) = nest(63);
    let mut expected = Vec::new();
    appendix_a(&ok_native, 1, &mut expected).unwrap();
    assert_eq!(sley_ty(&ok_bytes), Ok(expected));
    let (deep, deep_native) = nest(64);
    assert!(appendix_a(&deep_native, 1, &mut Vec::new()).is_err());
    assert_eq!(sley_ty(&deep), Err(26_006));
}

/// Lowering alone: `drv_load` once, then `lw_image` over the decoded
/// inventory (instructions and wall time exclude the load).
#[test]
#[ignore = "diagnostic"]
fn lw_lowering_only() {
    let (program, names) = program();
    let entry = std::env::var("LW_ENTRY").unwrap_or_else(|_| "t_lw_image".to_owned());
    let s = closure_s(names[entry.as_str()]);
    let inv = result(run(
        "drv_load",
        vec![objects_input(&s), bytes(s.epoch.as_bytes())],
    ))
    .unwrap();
    eprintln!("loaded");
    let start = Instant::now();
    let (value, instructions) = run_counted(
        "lw_image",
        vec![inv, bytes(names[entry.as_str()].as_bytes())],
    );
    let elapsed = start.elapsed();
    let image = as_bytes(&result(value).unwrap());
    assert_eq!(
        Ok(image.clone()),
        native_image(program, names[entry.as_str()])
    );
    eprintln!(
        "{entry}: lowering only: {} bytes, {instructions} instructions, {elapsed:?}",
        image.len()
    );
}

/// No `lw_*` edge passes one vector/map register twice: two block
/// parameters would then share one value, and the next in-place update of
/// either deep-copies it (quadratic lowering). See `lower.rs` (`put`).
#[test]
fn lw_edges_never_duplicate_large_values() {
    use sley_vm::{BytecodeSwitchArgument, BytecodeTerminator};
    let (program, names) = program();
    let lowered = native::lower(program, names["t_lw_image"]).unwrap();
    let by_id: std::collections::HashMap<EntityId, &str> =
        names.iter().map(|(n, id)| (*id, n.as_str())).collect();
    let mut found = Vec::new();
    for f in std::iter::once(&lowered.bytecode).chain(&lowered.callees) {
        let name = by_id.get(&f.function).copied().unwrap_or("?");
        if !name.starts_with("lw_") {
            continue;
        }
        for (bi, b) in f.blocks.iter().enumerate() {
            let mut edges: Vec<Vec<u32>> = vec![];
            match &b.terminator {
                BytecodeTerminator::Branch(e) => edges.push(e.arguments.clone()),
                BytecodeTerminator::CondBranch {
                    if_true, if_false, ..
                } => {
                    edges.push(if_true.arguments.clone());
                    edges.push(if_false.arguments.clone());
                }
                BytecodeTerminator::VariantSwitch { cases, .. } => {
                    for c in cases {
                        edges.push(
                            c.edge
                                .arguments
                                .iter()
                                .filter_map(|a| match a {
                                    BytecodeSwitchArgument::Value(r) => Some(*r),
                                    BytecodeSwitchArgument::CasePayload => None,
                                })
                                .collect(),
                        );
                    }
                }
                _ => {}
            }
            for args in edges {
                for (i, r) in args.iter().enumerate() {
                    let large = matches!(
                        f.register_types[*r as usize],
                        TypeExpr::Vector(_) | TypeExpr::OrderedMap { .. } | TypeExpr::Named(_)
                    );
                    if large && args[i + 1..].contains(r) {
                        found.push(format!("{name}: block {bi} register {r}"));
                    }
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "duplicated large edge arguments: {found:?}"
    );
}
