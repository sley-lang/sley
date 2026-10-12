//! `drv_*`: the build driver — `BuildRequest(S, P)` to an `EXEC_PACKAGE_V3`
//! toolchain package, entirely in Sley.
//!
//! The entry `drv_build(objects, root, epoch, limits)`:
//!
//! 1. decodes and digests the stored state root (`sr_root`);
//! 2. decodes every entity object (`drv_load`) and binds the root to them:
//!    the root's entity bindings must equal the decoded (entity, object)
//!    identities exactly, in order;
//! 3. resolves the root's single entry point to its function;
//! 4. checks the type environment (`ck_types`), then every function of the
//!    entry's call closure in the native lowering order (entry first, then
//!    callees discovered last-in first-out): graph/type/CFG validation
//!    (`ck_function`), the lowering profile refusal (type parameters,
//!    effects, contracts), and extended operation judgment
//!    (`jg_function`);
//! 5. lowers the closure to the `SLEYBC02` image (`lw_image`);
//! 6. runs the bootstrap-profile gate (`jg_gate`) and fingerprints every
//!    reached function (`fp_function`);
//! 7. assembles the package envelope (`pk_envelope`).
//!
//! `limits` is `(instructions, fuel, value units, output units,
//! has_cancel, cancel_at_fuel)`: the build profile's admitted limits.

use super::codes::TOOL_ROOT_BINDING_MISMATCH;
use super::model::{FUN, GATE, LIM};
use super::rt::{code, r};
use crate::seed::*;

/// `VM_LOWER_PROFILE_UNSUPPORTED`.
pub const VM_LOWER_PROFILE_UNSUPPORTED: u32 = 26_000;
/// The entry function of the toolchain program.
pub const ENTRY: &str = "drv_build";

fn limits_ty() -> Ty {
    t::tuple(vec![t::u64(), t::u64(), t::u64(), t::u64(), t::bool(), t::u64()])
}

/// Defines the driver.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    m.func(
        ENTRY,
        &[
            ("objects", t::vec(t::bytes())),
            ("root_bytes", t::bytes()),
            ("epoch", t::bytes()),
            ("limits", limits_ty()),
        ],
        r(t::bytes()),
        |f| {
            f.let_(
                "lim",
                rec(
                    LIM,
                    vec![
                        tget(v("limits"), 0),
                        tget(v("limits"), 1),
                        tget(v("limits"), 2),
                        tget(v("limits"), 3),
                        tget(v("limits"), 4),
                        tget(v("limits"), 5),
                    ],
                ),
            );
            f.let_("root", tri(call("sr_root", vec![v("root_bytes"), v("epoch")])));
            f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
            f.expr(tri(call("drv_bind", vec![v("root"), v("inv")])));
            f.let_("entries", field(v("root"), "entries"));
            f.if_(ne(vlen(v("entries")), u64c(1)), |f| {
                f.ret_err(code(TOOL_ROOT_BINDING_MISMATCH));
            });
            f.let_(
                "ept",
                call("inv_ept", vec![v("inv"), vget(v("entries"), u64c(0))]),
            );
            f.var("fid", bytesc(&[]));
            f.match_opt(
                v("ept"),
                "ep",
                |f| f.set("fid", field(v("ep"), "function")),
                |f| f.ret_err(code(TOOL_ROOT_BINDING_MISMATCH)),
            );
            f.expr(tri(call("ck_types", vec![v("inv")])));
            f.expr(tri(call("drv_check_closure", vec![v("inv"), v("fid")])));
            f.let_("image", tri(call("lw_image", vec![v("inv"), v("fid")])));
            f.let_("gate0", tri(call("jg_gate", vec![v("inv"), v("fid"), v("image")])));
            // Fingerprint every reached function, in gate order.
            f.let_("reached", field(v("gate0"), "functions"));
            f.let_(
                "fps",
                tri(call("fp_closure", vec![v("inv"), v("reached"), v("epoch")])),
            );
            f.let_(
                "gate",
                rec(
                    GATE,
                    vec![
                        v("reached"),
                        field(v("gate0"), "ops"),
                        field(v("gate0"), "bridges"),
                        field(v("gate0"), "imports"),
                        v("fps"),
                    ],
                ),
            );
            f.ret(call(
                "pk_envelope",
                vec![
                    v("inv"),
                    v("image"),
                    v("fid"),
                    v("epoch"),
                    field(v("root"), "root"),
                    v("lim"),
                    v("gate"),
                ],
            ));
        },
    );
    // The root binds exactly the decoded objects, in order.
    m.func(
        "drv_bind",
        &[("root", m.ty(super::root::ROOT)), ("inv", m.ty(super::inv::INV))],
        r(t::unit()),
        |f| {
            if fault("driver-binding") {
                f.ret_ok(unitc());
                return;
            }
            f.let_("keys", field(v("root"), "keys"));
            f.let_("vals", field(v("root"), "vals"));
            f.let_("ids", field(v("inv"), "ids"));
            f.let_("oids", field(v("inv"), "oids"));
            f.if_(ne(vlen(v("keys")), vlen(v("ids"))), |f| {
                f.ret_err(code(TOOL_ROOT_BINDING_MISMATCH));
            });
            f.for_u64("i", u64c(0), vlen(v("keys")), |f| {
                f.if_(
                    or(
                        ne(vget(v("keys"), v("i")), vget(v("ids"), v("i"))),
                        ne(vget(v("vals"), v("i")), vget(v("oids"), v("i"))),
                    ),
                    |f| f.ret_err(code(TOOL_ROOT_BINDING_MISMATCH)),
                );
            });
            f.ret_ok(unitc());
        },
    );
    // Checks the entry's call closure in the native lowering order.
    m.func(
        "drv_check_closure",
        &[("inv", m.ty(super::inv::INV)), ("fid", t::bytes())],
        r(t::unit()),
        |f| {
            f.expr(tri(call("drv_check_one", vec![v("inv"), v("fid")])));
            // An explicit stack: `pending[..top]` is live.
            f.var("pending", call("drv_callees", vec![v("inv"), v("fid")]));
            f.var("top", vlen(v("pending")));
            f.var("done", mnew(t::bytes(), t::bool(), vec![]));
            f.while_(gt(v("top"), u64c(0)), |f| {
                f.set("top", sub(v("top"), u64c(1)));
                f.let_("callee", vget(v("pending"), v("top")));
                f.if_(
                    or(eq(v("callee"), v("fid")), mhas(v("done"), v("callee"))),
                    |f| f.continue_(),
                );
                f.expr(tri(call("drv_check_one", vec![v("inv"), v("callee")])));
                f.set("done", mins(v("done"), v("callee"), boolc(true)));
                f.let_("more", call("drv_callees", vec![v("inv"), v("callee")]));
                f.for_u64("j", u64c(0), vlen(v("more")), |f| {
                    f.set(
                        "pending",
                        call("drv_stack_push", vec![v("pending"), v("top"), vget(v("more"), v("j"))]),
                    );
                    f.set("top", add(v("top"), u64c(1)));
                });
            });
            f.ret_ok(unitc());
        },
    );
    // Grows-or-overwrites push for an explicit stack.
    m.func(
        "drv_stack_push",
        &[("xs", t::vec(t::bytes())), ("top", t::u64()), ("x", t::bytes())],
        t::vec(t::bytes()),
        |f| {
            f.if_(lt(v("top"), vlen(v("xs"))), |f| f.ret(vset(v("xs"), v("top"), v("x"))));
            f.ret(push(v("xs"), v("x")));
        },
    );
    // One function: graph validation, profile refusal, operation judgment.
    m.func(
        "drv_check_one",
        &[("inv", m.ty(super::inv::INV)), ("fid", t::bytes())],
        r(t::unit()),
        |f| {
            f.let_("fun", call("inv_fun", vec![v("inv"), v("fid")]));
            f.match_opt(
                v("fun"),
                "fn_",
                |f| {
                    f.expr(tri(call("ck_function", vec![v("inv"), v("fid")])));
                    f.if_(
                        or(
                            or(
                                gt(vlen(field(v("fn_"), "tparams")), u64c(0)),
                                gt(vlen(field(v("fn_"), "effects")), u64c(0)),
                            ),
                            gt(vlen(field(v("fn_"), "contracts")), u64c(0)),
                        ),
                        |f| f.ret_err(code(VM_LOWER_PROFILE_UNSUPPORTED)),
                    );
                    f.expr(tri(call("jg_function", vec![v("inv"), v("fid")])));
                },
                |f| f.ret_err(code(26_003)),
            );
            f.ret_ok(unitc());
        },
    );
    // Direct callees of one function, in block and operation order.
    m.func(
        "drv_callees",
        &[("inv", m.ty(super::inv::INV)), ("fid", t::bytes())],
        t::vec(t::bytes()),
        |f| {
            f.var("out", vnew(t::bytes(), vec![]));
            f.let_("fun", unwrap(call("inv_fun", vec![v("inv"), v("fid")])));
            f.let_("blocks", field(v("fun"), "blocks"));
            f.for_u64("i", u64c(0), vlen(v("blocks")), |f| {
                f.let_("blk", unwrap(call("inv_blk", vec![v("inv"), vget(v("blocks"), v("i"))])));
                f.let_("ops", field(v("blk"), "ops"));
                f.for_u64("j", u64c(0), vlen(v("ops")), |f| {
                    f.let_("op", unwrap(call("inv_op", vec![v("inv"), vget(v("ops"), v("j"))])));
                    f.if_(eq(field(v("op"), "opcode"), u64c(112)), |f| {
                        f.let_("imm", field(v("op"), "imm"));
                        f.if_(eq(field(v("imm"), "tag"), u64c(7)), |f| {
                            f.set("out", push(v("out"), field(v("imm"), "id")));
                        });
                    });
                });
            });
            f.ret(v("out"));
        },
    );
    let _ = FUN;
}
