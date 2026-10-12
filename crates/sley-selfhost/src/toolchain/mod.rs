//! The SH2 toolchain program, constructed by the C0 seed assembler.
//!
//! Module map (every Sley function name carries its module prefix):
//!
//! - `rt_*` / `wr_*`: runtime library ([`rt`]);
//! - `scb_*`: schema-directed SCB1 validation of nested values;
//! - `cd_*`: entity-object and state-root decoding;
//! - `inv_*` / `drv_load`: the decoded program inventory;
//! - `ck_*`: type-environment and function-graph validation;
//! - `jg_*`: extended operation judgment and the bootstrap-profile gate;
//! - `lw_*`: SSMC-to-`SLEYBC02` lowering;
//! - `fp_*`: semantic fingerprints;
//! - `pk_*`: execution-package sections and envelope;
//! - `drv_*`: the build driver.

pub mod check;
pub mod codec;
pub mod codes;
pub mod fingerprint;
pub mod inv;
pub mod judge;
pub mod lower;
pub mod model;
pub mod package;
pub mod rt;
pub mod scb;

use crate::seed::Module;

/// Seed for every toolchain identity.
pub const TOOLCHAIN_SEED: &str = "sley.sh2.toolchain.v1";

/// Builds the complete toolchain module.
#[must_use]
pub fn module() -> Module {
    let mut m = Module::new(TOOLCHAIN_SEED);
    model::declare(&mut m);
    rt::define(&mut m);
    codec::define(&mut m);
    inv::define(&mut m);
    check::define(&mut m);
    judge::define(&mut m);
    lower::define(&mut m);
    fingerprint::define(&mut m);
    package::define(&mut m);
    scb::define(&mut m);
    scb::define_entry_key(&mut m);
    define_entries(&mut m);
    m
}

/// Test and driver entry points over the library functions.
fn define_entries(m: &mut Module) {
    use crate::seed::*;
    // Loads S and reports (functions, operations, index size).
    m.func(
        "t_load_counts",
        &[("objects", t::vec(t::bytes())), ("epoch", t::bytes())],
        rt::r(t::tuple(vec![t::u64(), t::u64(), t::u64()])),
        |f| {
            f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
            f.ret_ok(tup(vec![
                vlen(field(v("inv"), "fns")),
                vlen(field(v("inv"), "ops")),
                vlen(field(v("inv"), "ids")),
            ]));
        },
    );
    // Validates one complete ConstValue encoding.
    m.func(
        "scb_const_exact",
        &[("bs", t::bytes())],
        rt::r(t::unit()),
        |f| {
            f.let_("b", b2v(v("bs")));
            f.ret(call(
                "scb_walk",
                vec![
                    v("b"),
                    v("bs"),
                    u64c(0),
                    vlen(v("b")),
                    u64c(scb::K_CONST),
                    u64c(0),
                ],
            ));
        },
    );
}
