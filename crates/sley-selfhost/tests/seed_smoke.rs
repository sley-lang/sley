//! Seed assembler smoke tests through the native reference VM.

use sley_selfhost::native::{big_limits, execute};
use sley_selfhost::seed::*;
use sley_ssmc::{ConstData, ConstValue};
use sley_vm::ExecutionTermination;

fn u64v(value: u64) -> ConstValue {
    ConstValue {
        value_type: t::u64(),
        data: ConstData::UInt(u128::from(value)),
    }
}

fn run(m: Module, entry: &str, inputs: Vec<ConstValue>) -> (ConstValue, u64) {
    let (program, names) = m.finish();
    let outcome = execute(&program, names[entry], inputs, big_limits()).expect("executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => (value, outcome.instruction_count),
        other => panic!("termination {other:?}"),
    }
}

#[test]
fn sum_loop_and_calls() {
    let mut m = Module::new("smoke");
    m.func("inc", &[("x", t::u64())], t::u64(), |f| {
        f.ret(add(v("x"), u64c(1)));
    });
    m.func("sum_to", &[("n", t::u64())], t::u64(), |f| {
        f.var("acc", u64c(0));
        f.for_u64("i", u64c(0), v("n"), |f| {
            f.set("acc", add(v("acc"), call("inc", vec![v("i")])));
        });
        f.ret(v("acc"));
    });
    let (value, _) = run(m, "sum_to", vec![u64v(10)]);
    assert_eq!(value, u64v(55));
}

#[test]
fn records_variants_matches() {
    let mut m = Module::new("smoke2");
    m.record("Pt", &[("x", t::u64()), ("y", t::u64())]);
    m.variant(
        "Shape",
        &[("Dot", None), ("Line", Some(t::u64())), ("Box", Some(m.ty("Pt")))],
    );
    m.func("area", &[("s", m.ty("Shape"))], t::u64(), |f| {
        f.var("out", u64c(0));
        f.match_var(
            v("s"),
            vec![
                ("Dot", "_", Box::new(|f: &mut FnB<'_>| f.set("out", u64c(1)))),
                ("Line", "len", Box::new(|f: &mut FnB<'_>| f.set("out", v("len")))),
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
    m.func("main", &[], t::u64(), |f| {
        f.let_("b", var_new("Shape", "Box", Some(rec("Pt", vec![u64c(6), u64c(7)]))));
        f.let_("l", var_new("Shape", "Line", Some(u64c(5))));
        f.let_("d", var_new("Shape", "Dot", None));
        f.var("total", call("area", vec![v("b")]));
        f.set("total", add(v("total"), call("area", vec![v("l")])));
        f.set("total", add(v("total"), call("area", vec![v("d")])));
        f.if_else(
            is_case(v("l"), "Shape", "Line"),
            |f| f.set("total", add(v("total"), u64c(100))),
            |f| f.trap(),
        );
        f.ret(v("total"));
    });
    let (value, _) = run(m, "main", vec![]);
    assert_eq!(value, u64v(42 + 5 + 1 + 100));
}

#[test]
fn vectors_push_bytes_and_break() {
    let mut m = Module::new("smoke3");
    m.func("main", &[("b", t::bytes())], t::bytes(), |f| {
        f.let_("src", b2v(v("b")));
        f.var("out", vnew(t::u8(), vec![]));
        f.var("i", u64c(0));
        f.loop_(|f| {
            f.if_(ge(v("i"), vlen(v("src"))), |f| f.break_());
            f.let_("x", vget(v("src"), v("i")));
            f.match_opt(
                some(v("x")),
                "y",
                |f| f.set("out", push(v("out"), add(v("y"), u8c(1)))),
                |f| f.trap(),
            );
            f.set("i", add(v("i"), u64c(1)));
        });
        f.ret(v2b(v("out")));
    });
    let input = ConstValue {
        value_type: t::bytes(),
        data: ConstData::Bytes(vec![1, 2, 3]),
    };
    let (value, _) = run(m, "main", vec![input]);
    assert_eq!(value.data, ConstData::Bytes(vec![2, 3, 4]));
}

#[test]
fn unwrap_failure_traps_with_site() {
    let mut m = Module::new("smoke4");
    m.func("main", &[], t::u8(), |f| {
        f.ret(add(u8c(255), u8c(1)));
    });
    let (program, names) = m.finish();
    let outcome = execute(&program, names["main"], vec![], big_limits()).expect("executes");
    assert!(matches!(outcome.termination, ExecutionTermination::Trap { .. }));
}

#[test]
#[ignore = "throughput measurement"]
fn throughput() {
    let mut m = Module::new("bench");
    m.func("sum_to", &[("n", t::u64())], t::u64(), |f| {
        f.var("acc", u64c(0));
        f.for_u64("i", u64c(0), v("n"), |f| {
            f.set("acc", add(v("acc"), v("i")));
        });
        f.ret(v("acc"));
    });
    m.func("vec_fill", &[("n", t::u64())], t::u64(), |f| {
        f.var("xs", vnew(t::u8(), vec![]));
        f.for_u64("i", u64c(0), v("n"), |f| {
            f.set("xs", push(v("xs"), u8c(7)));
        });
        f.var("acc", u64c(0));
        f.for_u64("i", u64c(0), v("n"), |f| {
            f.set("xs", vset(v("xs"), v("i"), u8c(9)));
        });
        f.ret(vlen(v("xs")));
    });
    let (program, names) = m.finish();
    for (entry, n) in [("sum_to", 1_000_000_u64), ("vec_fill", 20_000)] {
        let start = std::time::Instant::now();
        let outcome = execute(&program, names[entry], vec![u64v(n)], big_limits()).unwrap();
        let elapsed = start.elapsed();
        eprintln!(
            "{entry}({n}): {:?} instructions={} fuel={} elapsed={elapsed:?} ({:.0} instr/s)",
            matches!(outcome.termination, ExecutionTermination::Success(_)),
            outcome.instruction_count,
            outcome.fuel_used,
            outcome.instruction_count as f64 / elapsed.as_secs_f64()
        );
    }
}
