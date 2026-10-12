//! The SH2 toolchain program, constructed by the C0 seed assembler.
//!
//! Module map (every Sley function name carries its module prefix):
//!
//! - `rt_*` / `wr_*`: runtime library ([`rt`]);
//! - `scb_*`: schema-directed SCB1 validation of nested values;
//! - `cd_*`: entity-object and state-root decoding;
//! - `ty_*`: canonical `TypeExpr` bytes ([`types`]);
//! - `sr_*`: the state-root record ([`root`]);
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
pub mod driver;
pub mod fingerprint;
pub mod inv;
pub mod judge;
pub mod lower;
pub mod model;
pub mod package;
pub mod root;
pub mod rt;
pub mod scb;
pub mod types;

use crate::seed::Module;

/// Seed for every toolchain identity.
pub const TOOLCHAIN_SEED: &str = "sley.sh2.toolchain.v1";
/// Seed for the workspace/package/namespace/entry-point identities of the
/// toolchain's canonical state.
pub const S_SEED: &str = "sley.sh2.toolchain.s.v1";

/// Builds the complete toolchain module.
#[must_use]
pub fn module() -> Module {
    let mut m = Module::new(TOOLCHAIN_SEED);
    model::declare(&mut m);
    rt::define(&mut m);
    codec::define(&mut m);
    types::define(&mut m);
    inv::define(&mut m);
    root::define(&mut m);
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
    // Builds two composite types from parts (ty_* constructor test).
    m.func("t_ty_build", &[("def", t::bytes())], t::vec(t::bytes()), |f| {
        f.let_("u64", types::ty_u64());
        f.let_("idx", types::ty_failure(sley_ssmc::BuiltinFailureKind::Index));
        f.let_("a", call("ty_res", vec![call("ty_vec", vec![v("u64")]), v("idx")]));
        f.let_(
            "b",
            call(
                "ty_tuple",
                vec![vnew(t::bytes(), vec![types::ty_bool(), call("ty_named0", vec![v("def")])])],
            ),
        );
        f.let_("c", call("ty_map", vec![v("u64"), call("ty_opt", vec![v("u64")])]));
        // Round trips through the accessors.
        f.let_("a_ok", call("ty_first", vec![v("a")]));
        f.let_("b_items", call("ty_tuple_items", vec![v("b")]));
        f.let_("named", call("ty_named_parts", vec![vget(v("b_items"), u64c(1))]));
        f.ret(vnew(
            t::bytes(),
            vec![
                v("a"),
                v("b"),
                v("c"),
                v("a_ok"),
                tget(v("named"), 0),
                call("uvarb", vec![call("ty_width", vec![v("u64")])]),
                call("uvarb", vec![call("ty_tag", vec![v("c")])]),
            ],
        ));
    });
    // Decodes a stored root: (digest, binding count, entry count).
    m.func(
        "t_sr_root",
        &[("bs", t::bytes()), ("epoch", t::bytes())],
        rt::r(t::tuple(vec![t::bytes(), t::u64(), t::u64(), t::bytes()])),
        |f| {
            f.let_("root", tri(call("sr_root", vec![v("bs"), v("epoch")])));
            f.ret_ok(tup(vec![
                field(v("root"), "root"),
                vlen(field(v("root"), "keys")),
                vlen(field(v("root"), "entries")),
                vget(field(v("root"), "vals"), u64c(0)),
            ]));
        },
    );
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
