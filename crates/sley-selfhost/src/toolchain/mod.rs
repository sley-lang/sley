//! The SH2 toolchain program, constructed by the C0 seed assembler.
//!
//! Module map (every Sley function name carries its module prefix):
//!
//! - `rt_*` / `wr_*`: runtime library ([`rt`]);
//! - `scb_*`: schema-directed SCB1 validation of nested values;
//! - `cd_*`: entity-object and state-root decoding;
//! - `ck_*`: semantic checking and the bootstrap-profile gate;
//! - `lw_*`: SSMC-to-`SLEYBC02` lowering;
//! - `fp_*`: semantic fingerprints;
//! - `pk_*`: execution-package sections and envelope;
//! - `drv_*`: the build driver.

pub mod codec;
pub mod codes;
pub mod model;
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
    scb::define(&mut m);
    scb::define_entry_key(&mut m);
    define_entries(&mut m);
    m
}

/// Test and driver entry points over the library functions.
fn define_entries(m: &mut Module) {
    use crate::seed::*;
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
