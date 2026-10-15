//! The decoded program inventory (`inv_*`) and the loader (`drv_load`).
//!
//! `Inv` holds one vector per entity kind plus `index`, a map from entity
//! identity to `kind * 2^32 + position` in that kind's vector. Objects
//! arrive in strictly ascending entity-identity order (state-root binding
//! order), so every index insertion appends.

use super::codes::TOOL_ROOT_BINDING_MISMATCH;
use super::model::{BLK, CST, EPT, FUN, IMP, OP, PRM, TDEF};
use super::rt::{code, r};
use crate::seed::{
    FnB, Module, Ty, add, call, div, eq, field, gt, le, mget_opt, mins, mnew, mul, none, push, rec,
    rem, some, sub, t, tri, u64c, v, vget, vlen, vnew,
};

/// The inventory record.
pub const INV: &str = "Inv";

/// SSMC1 kind tags used as index kinds.
pub const KIND_TYPEDEF: u64 = 4;
/// Function.
pub const KIND_FUNCTION: u64 = 5;
/// Parameter.
pub const KIND_PARAMETER: u64 = 6;
/// Block.
pub const KIND_BLOCK: u64 = 7;
/// Operation.
pub const KIND_OPERATION: u64 = 8;
/// Constant.
pub const KIND_CONSTANT: u64 = 9;
/// Adapter import.
pub const KIND_IMPORT: u64 = 15;
/// Entry point.
pub const KIND_ENTRY: u64 = 16;

const SHIFT: u64 = 1 << 32;

/// Declares `Inv` and defines the loader and lookups.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    let index_ty = t::map(t::bytes(), t::u64());
    m.record(
        INV,
        &[
            ("fns", t::vec(m.ty(FUN))),
            ("prms", t::vec(m.ty(PRM))),
            ("blks", t::vec(m.ty(BLK))),
            ("ops", t::vec(m.ty(OP))),
            ("csts", t::vec(m.ty(CST))),
            ("tdefs", t::vec(m.ty(TDEF))),
            ("imps", t::vec(m.ty(IMP))),
            ("eps", t::vec(m.ty(EPT))),
            // Entity identity -> kind * 2^32 + position.
            ("index", index_ty.clone()),
            // Every decoded (entity id, object id), in order.
            ("ids", t::vec(t::bytes())),
            ("oids", t::vec(t::bytes())),
        ],
    );
    // Decodes every object; identities must strictly ascend.
    m.func(
        "drv_load",
        &[("objects", t::vec(t::bytes())), ("epoch", t::bytes())],
        r(m.ty(INV)),
        |f| {
            f.var("fns", vnew(named_ty(FUN), vec![]));
            f.var("prms", vnew(named_ty(PRM), vec![]));
            f.var("blks", vnew(named_ty(BLK), vec![]));
            f.var("ops", vnew(named_ty(OP), vec![]));
            f.var("csts", vnew(named_ty(CST), vec![]));
            f.var("tdefs", vnew(named_ty(TDEF), vec![]));
            f.var("imps", vnew(named_ty(IMP), vec![]));
            f.var("eps", vnew(named_ty(EPT), vec![]));
            f.var("index", mnew(t::bytes(), t::u64(), vec![]));
            f.var("ids", vnew(t::bytes(), vec![]));
            f.var("oids", vnew(t::bytes(), vec![]));
            f.for_u64("i", u64c(0), vlen(v("objects")), |f| {
                f.let_(
                    "obj",
                    tri(call(
                        "cd_object",
                        vec![vget(v("objects"), v("i")), v("epoch")],
                    )),
                );
                f.let_("id", field(v("obj"), "id"));
                f.if_(gt(v("i"), u64c(0)), |f| {
                    f.if_(le(v("id"), vget(v("ids"), sub(v("i"), u64c(1)))), |f| {
                        f.ret_err(code(TOOL_ROOT_BINDING_MISMATCH));
                    });
                });
                f.set("ids", push(v("ids"), v("id")));
                f.set("oids", push(v("oids"), field(v("obj"), "oid")));
                f.let_("kind", field(v("obj"), "kind"));
                let slot = |f: &mut FnB<'_>, vector: &str, payload: &str| {
                    f.set(
                        "index",
                        mins(
                            v("index"),
                            v("id"),
                            add(mul(v("kind"), u64c(SHIFT)), vlen(v(vector))),
                        ),
                    );
                    f.set(vector, push(v(vector), v(payload)));
                };
                f.match_var(
                    field(v("obj"), "ent"),
                    vec![
                        ("W", "_", Box::new(|_: &mut FnB<'_>| {})),
                        ("K", "_", Box::new(|_: &mut FnB<'_>| {})),
                        ("N", "_", Box::new(|_: &mut FnB<'_>| {})),
                        (
                            "T",
                            "x",
                            Box::new(move |f: &mut FnB<'_>| slot(f, "tdefs", "x")),
                        ),
                        (
                            "F",
                            "x",
                            Box::new(move |f: &mut FnB<'_>| slot(f, "fns", "x")),
                        ),
                        (
                            "P",
                            "x",
                            Box::new(move |f: &mut FnB<'_>| slot(f, "prms", "x")),
                        ),
                        (
                            "B",
                            "x",
                            Box::new(move |f: &mut FnB<'_>| slot(f, "blks", "x")),
                        ),
                        (
                            "O",
                            "x",
                            Box::new(move |f: &mut FnB<'_>| slot(f, "ops", "x")),
                        ),
                        (
                            "C",
                            "x",
                            Box::new(move |f: &mut FnB<'_>| slot(f, "csts", "x")),
                        ),
                        (
                            "I",
                            "x",
                            Box::new(move |f: &mut FnB<'_>| slot(f, "imps", "x")),
                        ),
                        (
                            "E",
                            "x",
                            Box::new(move |f: &mut FnB<'_>| slot(f, "eps", "x")),
                        ),
                    ],
                );
            });
            f.ret_ok(rec(
                INV,
                vec![
                    v("fns"),
                    v("prms"),
                    v("blks"),
                    v("ops"),
                    v("csts"),
                    v("tdefs"),
                    v("imps"),
                    v("eps"),
                    v("index"),
                    v("ids"),
                    v("oids"),
                ],
            ));
        },
    );
    // Position of `id` when it is an entity of `kind`.
    m.func(
        "inv_find",
        &[("index", index_ty), ("id", t::bytes()), ("kind", t::u64())],
        t::opt(t::u64()),
        |f| {
            f.match_opt(
                mget_opt(v("index"), v("id")),
                "code",
                |f| {
                    f.if_(eq(div(v("code"), u64c(SHIFT)), v("kind")), |f| {
                        f.ret(some(rem(v("code"), u64c(SHIFT))));
                    });
                },
                |_| {},
            );
            f.ret(none(t::u64()));
        },
    );
    // Typed lookups: `None` when absent or of another kind.
    for (name, kind, vector, type_name) in [
        ("inv_fun", KIND_FUNCTION, "fns", FUN),
        ("inv_prm", KIND_PARAMETER, "prms", PRM),
        ("inv_blk", KIND_BLOCK, "blks", BLK),
        ("inv_op", KIND_OPERATION, "ops", OP),
        ("inv_cst", KIND_CONSTANT, "csts", CST),
        ("inv_tdef", KIND_TYPEDEF, "tdefs", TDEF),
        ("inv_imp", KIND_IMPORT, "imps", IMP),
        ("inv_ept", KIND_ENTRY, "eps", EPT),
    ] {
        let ty = m.ty(type_name);
        m.func(
            name,
            &[("inv", m.ty(INV)), ("id", t::bytes())],
            t::opt(ty.clone()),
            move |f| {
                f.match_opt(
                    call(
                        "inv_find",
                        vec![field(v("inv"), "index"), v("id"), u64c(kind)],
                    ),
                    "pos",
                    |f| f.ret(some(vget(field(v("inv"), vector), v("pos")))),
                    |_| {},
                );
                f.ret(none(ty));
            },
        );
    }
}

/// A model type by name.
fn named_ty(name: &str) -> Ty {
    Module::new(super::TOOLCHAIN_SEED).ty(name)
}
