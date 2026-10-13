//! End to end: the Sley toolchain (`drv_build`) builds canonical S into an
//! `EXEC_PACKAGE_V3` envelope byte-identical to the native C0 build.
#![allow(clippy::missing_panics_doc)]
mod common;
use common::*;
use sley_selfhost::canonical;
use sley_selfhost::native;
use sley_selfhost::seed::*;
use sley_ssmc::{ConstData, ConstValue, TypeExpr};

fn limits_value(l: sley_vm::ExecutionLimits) -> ConstValue {
    let u = |x: u64| u64v(x);
    ConstValue {
        value_type: t::tuple(vec![
            t::u64(),
            t::u64(),
            t::u64(),
            t::u64(),
            t::bool(),
            t::u64(),
        ]),
        data: ConstData::Sequence(vec![
            u(l.max_instructions),
            u(l.max_fuel),
            u(l.max_value_units),
            u(l.max_output_units),
            ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(l.cancel_at_fuel.is_some()),
            },
            u(l.cancel_at_fuel.unwrap_or(0)),
        ]),
    }
}

/// Builds S with the Sley toolchain and with C0; returns (sley, native).
pub fn both(s: &canonical::CanonicalS) -> (Result<Vec<u8>, u32>, Vec<u8>, u64) {
    let limits = native::toolchain_limits();
    let native = native::c0_build(s, limits).expect("C0 builds").envelope;
    let t = std::time::Instant::now();
    let (value, n) = run_counted(
        "drv_build",
        vec![
            objects_input(s),
            bytes(&s.root.stored_bytes),
            bytes(s.epoch.as_bytes()),
            limits_value(limits),
        ],
    );
    eprintln!("drv_build: {n} instructions in {:?}", t.elapsed());
    (result(value).map(|v| as_bytes(&v)), native, n)
}

fn small_program() -> (Program, sley_id::EntityId) {
    let mut m = Module::new("e2e-small");
    m.record("Pt", &[("x", t::u64()), ("y", t::u64())]);
    m.variant("Shape", &[("Dot", None), ("Box", Some(m.ty("Pt")))]);
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
        "main",
        &[("n", t::u64()), ("b", t::bytes())],
        t::bytes(),
        |f| {
            f.var("acc", u64c(0));
            f.for_u64("i", u64c(0), v("n"), |f| {
                f.set(
                    "acc",
                    add(
                        v("acc"),
                        call(
                            "area",
                            vec![var_new(
                                "Shape",
                                "Box",
                                Some(rec("Pt", vec![v("i"), u64c(3)])),
                            )],
                        ),
                    ),
                );
            });
            f.var("out", b2v(v("b")));
            f.set("out", push(v("out"), u8c(7)));
            f.ret(concat(vnew(
                t::bytes(),
                vec![v2b(v("out")), blake3(v("b"))],
            )));
        },
    );
    let (program, names) = m.finish();
    (program, names["main"])
}

#[test]
fn builds_a_small_program_like_c0() {
    let (program, entry) = small_program();
    let s = canonical::build(&program, entry, "e2e-small");
    let (sley, native, _) = both(&s);
    assert_eq!(sley.expect("Sley build succeeds"), native);
}

#[test]
fn builds_toolchain_closures_like_c0() {
    let (program, names) = program();
    for entry in ["rt_rd_uvar", "sr_root", "cd_object"] {
        let pruned = canonical::prune_to_closure(program, names[entry]);
        let s = canonical::build(&pruned, names[entry], "e2e-closure");
        let (sley, native, n) = both(&s);
        assert_eq!(sley.expect("Sley build succeeds"), native, "{entry}");
        eprintln!(
            "{entry}: {} objects, {} byte package, {n} instructions",
            s.objects.len(),
            native.len()
        );
    }
}
