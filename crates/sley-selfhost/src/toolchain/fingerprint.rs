//! `fp_*`: semantic fingerprints of judged functions.
//!
//! `fp_function(inv, fid, epoch)` reproduces
//! `sley_ssmc::fingerprint::fingerprint_function` exactly as the bootstrap
//! gate calls it (`judged_closure_fingerprints`): the function body is
//! encoded with canonical local slots (function parameters `(1, 0, i)`,
//! block parameters `(2, block, i)`, operation results `(block, op, i)`),
//! types in the fingerprint `TypeExpr` encoding (`u32` tag, `u16` widths
//! and failure kinds, `u64` list lengths, entity references `u32(1) || id`
//! or `u32(2)` for the enclosing function), and the preimage
//! `"SLEYSFP1" || u32(1) || epoch || SSMC1_FIELD_SCHEMA_HASH || u32(5) ||
//! u64(len) || body` hashed under the `sley2.semantic-fingerprint.v1`
//! domain with the raw BLAKE3 bridge.
//!
//! Types arrive as canonical SCB1 bytes and are converted once per distinct
//! type (the memo maps type bytes to `(encoding, length, entity refs)`; a
//! type that names the enclosing function is re-encoded for it).
//!
//! The gate's narrowing also requires that no unlisted entity claims the
//! function (a parameter owned by it, a block of it, an operation of one of
//! its blocks). `fp_closure` checks that through one reverse-ownership pass
//! (`fp_owned`); `fp_function` alone checks every listed entity but cannot
//! see unlisted claimants without that pass.
//!
//! The same structural type encoding is the execution-package `TypeExpr`
//! encoding (with no enclosing function), so `pk_*` reuses `fp_ty`.

use super::inv::{INV, KIND_BLOCK, KIND_FUNCTION, KIND_OPERATION, KIND_PARAMETER};
use super::model::*;
use super::rt::{code, r};
use crate::seed::*;

/// `FINGERPRINT_INVENTORY_INVALID` (frozen numeric value).
pub const FP_INVENTORY_INVALID: u32 = 25_001;
/// `FINGERPRINT_LOCAL_REFERENCE_INVALID` (frozen numeric value).
pub const FP_LOCAL_REFERENCE_INVALID: u32 = 25_002;
/// `FINGERPRINT_RESOURCE_LIMIT` (frozen numeric value).
pub const FP_RESOURCE_LIMIT: u32 = 25_005;
/// The fingerprinted identity is not a function of the inventory
/// (`VM_LOWER_IMMEDIATE_MISMATCH`, the gate's refusal for that case).
pub const FP_FUNCTION_MISSING: u32 = 26_003;

/// Fingerprint digest domain (`sley_id::Domain::SemanticFingerprint`).
pub const FP_DOMAIN: &[u8] = b"sley2.semantic-fingerprint.v1";
/// Charged-work ceiling (`MAX_FINGERPRINT_WORK`).
const MAX_WORK: u64 = 100_000_000;
/// Operation ceiling (`MAX_FINGERPRINT_OPERATIONS`).
const MAX_OPS: u64 = 1_000_000;
/// Graph-entity ceiling (`MAX_FINGERPRINT_GRAPH_ENTITIES`).
const MAX_ENTITIES: u64 = 2_000_000;
/// Largest preimage hashed through `RHW1` (larger ones use `RHW2`).
const SMALL_HASH: u64 = 1_048_576;

/// Big-endian `u32` literal bytes.
#[must_use]
pub fn lit32(x: u32) -> E {
    bytesc(&x.to_be_bytes())
}

/// Memo entry: `(encoding, length, entity references)`.
#[must_use]
pub fn ent_ty() -> Ty {
    t::tuple(vec![t::bytes(), t::u64(), t::vec(t::bytes())])
}

/// Type memo: canonical SCB type bytes -> memo entry.
#[must_use]
pub fn memo_ty() -> Ty {
    t::map(t::bytes(), ent_ty())
}

/// Reverse ownership counts: (function -> owned parameters, function ->
/// blocks claiming it, block -> operations claiming it).
fn own_ty() -> Ty {
    let counts = t::map(t::bytes(), t::u64());
    t::tuple(vec![counts.clone(), counts.clone(), counts])
}

fn empty_own() -> E {
    tup(vec![
        mnew(t::bytes(), t::u64(), vec![]),
        mnew(t::bytes(), t::u64(), vec![]),
        mnew(t::bytes(), t::u64(), vec![]),
    ])
}

/// Appends one piece of `len` bytes to the `out`/`n` accumulators.
pub fn put(f: &mut FnB<'_>, piece: E, len: E) {
    f.set("out", push(v("out"), piece));
    f.set("n", add(v("n"), len));
}

/// Appends the encoding of type `ty` through the `memo` variable, with
/// `me` the enclosing-function identity (empty bytes for none).
///
/// The memo is projected last: projecting a dead register's part copies
/// the whole value while another view shares its root, so the encoding
/// and length go first and the memo then moves out unshared.
pub fn tyc(f: &mut FnB<'_>, ty: E, me: E) {
    f.let_("#tc", call("fp_tyc", vec![v("memo"), ty, me]));
    put(f, tget(v("#tc"), 1), tget(v("#tc"), 2));
    f.set("memo", tget(v("#tc"), 0));
}

/// Keeps `inv` (and other shared inputs) live to this point.
///
/// `field`/`tget`/`vget` on a register that is dead after the projection
/// take the ownership path, which deep-copies the whole value when its
/// root is shared (here: with the caller). Ending a function with this
/// call keeps the shared inputs live, so every earlier projection shares.
pub fn keep(f: &mut FnB<'_>, names: &[&str]) {
    for name in names {
        f.expr(call(&format!("fp_keep_{name}"), vec![v(name)]));
    }
}

/// Appends `u64(len(x))` for a vector `x`.
fn put_len(f: &mut FnB<'_>, x: E) {
    put(f, call("fp_be64", vec![vlen(x)]), u64c(8));
}

/// Appends one entity reference relative to the `fid` variable.
fn eref(f: &mut FnB<'_>, x: E) {
    f.let_("#er", x);
    f.if_else(
        eq(v("#er"), v("fid")),
        |f| put(f, lit32(2), u64c(4)),
        |f| {
            put(f, lit32(1), u64c(4));
            put(f, v("#er"), u64c(32));
        },
    );
}

/// Appends one value reference (16 bytes) through `pmap`/`omap`.
fn vref(f: &mut FnB<'_>, x: E) {
    f.let_("#vr", tri(call("fp_vref", vec![v("pmap"), v("omap"), x])));
    put(f, v("#vr"), u64c(16));
}

/// Appends the slot of block `target` (local-reference failure if absent).
fn slot(f: &mut FnB<'_>, target: E) {
    f.let_("#ts", mget_opt(v("bslot"), target));
    f.if_(not(is_some(v("#ts"))), |f| {
        f.ret_err(code(FP_LOCAL_REFERENCE_INVALID));
    });
    put(f, call("fp_be32", vec![unwrap(v("#ts"))]), u64c(4));
}

/// Appends one target edge.
fn edge(f: &mut FnB<'_>, e: E) {
    f.let_("#ed", e);
    slot(f, field(v("#ed"), "target"));
    f.let_("#ea", field(v("#ed"), "args"));
    put_len(f, v("#ea"));
    f.for_u64("#ek", u64c(0), vlen(v("#ea")), |f| {
        vref(f, vget(v("#ea"), v("#ek")));
    });
}

/// Appends one terminator.
fn term(f: &mut FnB<'_>, x: E) {
    f.let_("tm", x);
    f.let_("tk", field(v("tm"), "kind"));
    put(f, call("fp_be32", vec![v("tk")]), u64c(4));
    f.if_(eq(v("tk"), u64c(1)), |f| vref(f, field(v("tm"), "val")));
    f.if_(eq(v("tk"), u64c(2)), |f| {
        edge(f, vget(field(v("tm"), "edges"), u64c(0)));
    });
    f.if_(eq(v("tk"), u64c(3)), |f| {
        vref(f, field(v("tm"), "val"));
        edge(f, vget(field(v("tm"), "edges"), u64c(0)));
        edge(f, vget(field(v("tm"), "edges"), u64c(1)));
    });
    f.if_(eq(v("tk"), u64c(4)), |f| {
        vref(f, field(v("tm"), "val"));
        f.let_("cs", field(v("tm"), "cases"));
        put_len(f, v("cs"));
        f.for_u64("c", u64c(0), vlen(v("cs")), |f| {
            f.let_("cc", vget(v("cs"), v("c")));
            f.if_else(
                field(v("cc"), "member"),
                |f| {
                    put(f, lit32(1), u64c(4));
                    put(f, field(v("cc"), "key"), u64c(32));
                },
                |f| {
                    put(f, lit32(2), u64c(4));
                    put(f, call("fp_be32", vec![field(v("cc"), "builtin")]), u64c(4));
                },
            );
            slot(f, field(v("cc"), "target"));
            f.let_("sa", field(v("cc"), "args"));
            put_len(f, v("sa"));
            f.for_u64("a", u64c(0), vlen(v("sa")), |f| {
                f.let_("aa", vget(v("sa"), v("a")));
                f.if_else(
                    field(v("aa"), "pay"),
                    |f| put(f, lit32(2), u64c(4)),
                    |f| {
                        put(f, lit32(1), u64c(4));
                        vref(f, field(v("aa"), "val"));
                    },
                );
            });
        });
    });
    f.if_(eq(v("tk"), u64c(5)), |f| {
        put(f, call("fp_be32", vec![field(v("tm"), "code")]), u64c(4));
        f.if_else(
            field(v("tm"), "has_val"),
            |f| {
                put(f, lit32(2), u64c(4));
                vref(f, field(v("tm"), "val"));
            },
            |f| put(f, lit32(1), u64c(4)),
        );
    });
}

/// Appends one immediate.
fn imm(f: &mut FnB<'_>, x: E) {
    f.let_("im", x);
    f.let_("ig", field(v("im"), "tag"));
    put(f, call("fp_be32", vec![v("ig")]), u64c(4));
    f.if_(eq(v("ig"), u64c(2)), |f| eref(f, field(v("im"), "id")));
    f.if_(eq(v("ig"), u64c(3)), |f| {
        put(f, call("fp_be32", vec![field(v("im"), "index")]), u64c(4));
    });
    f.if_(or(eq(v("ig"), u64c(4)), eq(v("ig"), u64c(6))), |f| {
        put(f, field(v("im"), "id"), u64c(32));
    });
    f.if_(eq(v("ig"), u64c(5)), |f| {
        eref(f, field(v("im"), "id"));
        put(f, field(v("im"), "member"), u64c(32));
    });
    f.if_(eq(v("ig"), u64c(7)), |f| {
        eref(f, field(v("im"), "id"));
        f.let_("ta", field(v("im"), "targs"));
        put_len(f, v("ta"));
        f.for_u64("k", u64c(0), vlen(v("ta")), |f| {
            tyc(f, vget(v("ta"), v("k")), v("fid"));
        });
    });
}

/// Binds `name` to the inventory entity `id` of `kind` from the live
/// bindings `idx` (the inventory index) and `all` (that kind's vector), or
/// returns `err`.
///
/// `inv_fun`/`inv_blk`/... index a temporary `field(inv, ...)` view, which
/// the VM's ownership arm materializes (a deep copy of the whole kind
/// vector) on every lookup because the view is dead after `vget`; indexing a
/// live binding takes the borrowed path and copies only the element.
fn need(f: &mut FnB<'_>, name: &str, all: &str, kind: u64, id: E, err: u32) {
    let pos = format!("#{name}");
    f.let_(&pos, call("inv_find", vec![v("idx"), id, u64c(kind)]));
    f.if_(not(is_some(v(&pos))), |f| f.ret_err(code(err)));
    f.let_(name, vget(v(all), unwrap(v(&pos))));
}

fn fail(f: &mut FnB<'_>, condition: E, err: u32) {
    f.if_(condition, |f| f.ret_err(code(err)));
}

/// Defines the `fp_*` functions.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    let inv_ty = m.ty(INV);
    let vref_ty = m.ty(VREF);
    let core_ty = t::tuple(vec![t::bytes(), t::u64(), t::u64(), memo_ty()]);

    // ----- liveness anchors (see `keep`) -----
    for (name, ty) in [
        ("inv", inv_ty.clone()),
        ("own", own_ty()),
        ("all_fns", t::vec(m.ty(FUN))),
        ("all_prms", t::vec(m.ty(PRM))),
        ("all_blks", t::vec(m.ty(BLK))),
        ("all_ops", t::vec(m.ty(OP))),
    ] {
        m.func(&format!("fp_keep_{name}"), &[("x", ty)], t::unit(), |f| {
            f.ret(unitc());
        });
    }

    // ----- small fixed-width writers -----
    m.func("fp_be32", &[("x", t::u64())], t::bytes(), |f| {
        f.if_(lt(v("x"), u64c(256)), |f| {
            f.ret(v2b(vnew(
                t::u8(),
                vec![u8c(0), u8c(0), u8c(0), call("rt_byte", vec![v("x")])],
            )));
        });
        f.ret(call("be32", vec![v("x")]));
    });
    m.func("fp_be64", &[("x", t::u64())], t::bytes(), |f| {
        f.if_(lt(v("x"), u64c(256)), |f| {
            let mut bytes = vec![u8c(0); 7];
            bytes.push(call("rt_byte", vec![v("x")]));
            f.ret(v2b(vnew(t::u8(), bytes)));
        });
        f.ret(call("be64", vec![v("x")]));
    });
    // `x` below 2^16.
    m.func("fp_be16", &[("x", t::u64())], t::bytes(), |f| {
        f.ret(v2b(vnew(
            t::u8(),
            vec![
                call("rt_byte", vec![div(v("x"), u64c(256))]),
                call("rt_byte", vec![rem(v("x"), u64c(256))]),
            ],
        )));
    });

    // ----- TypeExpr: canonical SCB1 bytes -> fingerprint/package encoding -----
    // Explicit stack of (kind, bytes, len): kind 0 a type to encode, kind 1
    // literal bytes of length `len` (the FunctionRef effect tail).
    m.func(
        "fp_ty",
        &[("ty", t::bytes()), ("me", t::bytes())],
        ent_ty(),
        |f| {
            let item = t::tuple(vec![t::u64(), t::bytes(), t::u64()]);
            f.var("stack", vnew(item, vec![tup(vec![u64c(0), v("ty"), u64c(0)])]));
            f.var("sp", u64c(1));
            f.var("out", vnew(t::bytes(), vec![]));
            f.var("n", u64c(0));
            f.var("refs", vnew(t::bytes(), vec![]));
            // Pushes one stack item (reusing popped slots).
            let spush = |f: &mut FnB<'_>, x: E| {
                f.if_else(
                    lt(v("sp"), vlen(v("stack"))),
                    |f| f.set("stack", vset(v("stack"), v("sp"), x.clone())),
                    |f| f.set("stack", push(v("stack"), x.clone())),
                );
                f.set("sp", add(v("sp"), u64c(1)));
            };
            // Pushes the types of vector `xs` in reverse (first pops first).
            let spush_rev = move |f: &mut FnB<'_>, xs: &str| {
                f.var("j", vlen(v(xs)));
                f.while_(gt(v("j"), u64c(0)), |f| {
                    f.set("j", sub(v("j"), u64c(1)));
                    spush(f, tup(vec![u64c(0), vget(v(xs), v("j")), u64c(0)]));
                });
            };
            let eref_me = |f: &mut FnB<'_>, x: E| {
                f.let_("#er", x);
                f.if_else(
                    eq(v("#er"), v("me")),
                    |f| put(f, lit32(2), u64c(4)),
                    |f| {
                        put(f, lit32(1), u64c(4));
                        put(f, v("#er"), u64c(32));
                    },
                );
                f.set("refs", push(v("refs"), v("#er")));
            };
            f.while_(gt(v("sp"), u64c(0)), move |f| {
                f.set("sp", sub(v("sp"), u64c(1)));
                f.let_("it", vget(v("stack"), v("sp")));
                f.let_("x", tget(v("it"), 1));
                f.if_else(
                    eq(tget(v("it"), 0), u64c(1)),
                    |f| put(f, v("x"), tget(v("it"), 2)),
                    move |f| {
                        f.let_("pt", call("ty_parts", vec![v("x")]));
                        f.let_("tag", tget(v("pt"), 0));
                        f.let_("pl", tget(v("pt"), 1));
                        put(f, call("fp_be32", vec![v("tag")]), u64c(4));
                        // SInt/UInt width, BuiltinFailure kind: u16.
                        f.if_(
                            or(
                                or(eq(v("tag"), u64c(3)), eq(v("tag"), u64c(4))),
                                eq(v("tag"), u64c(20)),
                            ),
                            |f| {
                                put(
                                    f,
                                    call(
                                        "fp_be16",
                                        vec![call("ty_uvar_payload", vec![v("pl")])],
                                    ),
                                    u64c(2),
                                );
                            },
                        );
                        // TypeParameter: u32.
                        f.if_(eq(v("tag"), u64c(19)), |f| {
                            put(
                                f,
                                call("fp_be32", vec![call("ty_uvar_payload", vec![v("pl")])]),
                                u64c(4),
                            );
                        });
                        // Tuple.
                        f.if_(eq(v("tag"), u64c(9)), move |f| {
                            f.let_("items", call("ty_list", vec![v("pl")]));
                            put_len(f, v("items"));
                            spush_rev(f, "items");
                        });
                        // Named.
                        f.if_(eq(v("tag"), u64c(10)), move |f| {
                            f.let_("fs", call("ty_fields", vec![v("pl")]));
                            eref_me(f, vget(v("fs"), u64c(0)));
                            f.let_("args", call("ty_list", vec![vget(v("fs"), u64c(1))]));
                            put_len(f, v("args"));
                            spush_rev(f, "args");
                        });
                        // Vector, Option, LocalCell.
                        f.if_(
                            or(
                                or(eq(v("tag"), u64c(11)), eq(v("tag"), u64c(13))),
                                eq(v("tag"), u64c(18)),
                            ),
                            move |f| spush(f, tup(vec![u64c(0), v("pl"), u64c(0)])),
                        );
                        // OrderedMap, Result.
                        f.if_(
                            or(eq(v("tag"), u64c(12)), eq(v("tag"), u64c(14))),
                            move |f| {
                                f.let_("fs", call("ty_fields", vec![v("pl")]));
                                spush(f, tup(vec![u64c(0), vget(v("fs"), u64c(1)), u64c(0)]));
                                spush(f, tup(vec![u64c(0), vget(v("fs"), u64c(0)), u64c(0)]));
                            },
                        );
                        // FunctionRef: params, result, then the effect tail.
                        f.if_(eq(v("tag"), u64c(15)), move |f| {
                            f.let_("fs", call("ty_fields", vec![v("pl")]));
                            f.let_("ps", call("ty_list", vec![vget(v("fs"), u64c(0))]));
                            f.let_("es", call("ty_list", vec![vget(v("fs"), u64c(2))]));
                            f.var(
                                "tail",
                                vnew(t::bytes(), vec![call("fp_be64", vec![vlen(v("es"))])]),
                            );
                            f.var("tl", u64c(8));
                            f.for_u64("k", u64c(0), vlen(v("es")), |f| {
                                f.let_("e", vget(v("es"), v("k")));
                                f.set("refs", push(v("refs"), v("e")));
                                f.if_else(
                                    eq(v("e"), v("me")),
                                    |f| {
                                        f.set("tail", push(v("tail"), lit32(2)));
                                        f.set("tl", add(v("tl"), u64c(4)));
                                    },
                                    |f| {
                                        f.set("tail", push(v("tail"), lit32(1)));
                                        f.set("tail", push(v("tail"), v("e")));
                                        f.set("tl", add(v("tl"), u64c(36)));
                                    },
                                );
                            });
                            spush(f, tup(vec![u64c(1), concat(v("tail")), v("tl")]));
                            spush(f, tup(vec![u64c(0), vget(v("fs"), u64c(1)), u64c(0)]));
                            spush_rev(f, "ps");
                            put_len(f, v("ps"));
                        });
                        // AdapterHandle, CapabilityToken.
                        f.if_(
                            or(eq(v("tag"), u64c(16)), eq(v("tag"), u64c(17))),
                            move |f| eref_me(f, v("pl")),
                        );
                    },
                );
            });
            f.ret(tup(vec![concat(v("out")), v("n"), v("refs")]));
        },
    );
    // Memoized type encoding relative to `me`: (memo, encoding, length).
    m.func(
        "fp_tyc",
        &[("memo", memo_ty()), ("ty", t::bytes()), ("me", t::bytes())],
        t::tuple(vec![memo_ty(), t::bytes(), t::u64()]),
        |f| {
            f.var("memo", v("memo"));
            f.var("ent", tup(vec![bytesc(&[]), u64c(0), vnew(t::bytes(), vec![])]));
            f.match_opt(
                mget_opt(v("memo"), v("ty")),
                "hit",
                |f| f.set("ent", v("hit")),
                |f| {
                    f.set("ent", call("fp_ty", vec![v("ty"), bytesc(&[])]));
                    f.set("memo", mins(v("memo"), v("ty"), v("ent")));
                },
            );
            f.let_("refs", tget(v("ent"), 2));
            f.for_u64("k", u64c(0), vlen(v("refs")), |f| {
                f.if_(eq(vget(v("refs"), v("k")), v("me")), |f| {
                    f.let_("s", call("fp_ty", vec![v("ty"), v("me")]));
                    f.ret(tup(vec![v("memo"), tget(v("s"), 0), tget(v("s"), 1)]));
                });
            });
            f.ret(tup(vec![v("memo"), tget(v("ent"), 0), tget(v("ent"), 1)]));
        },
    );
    // One value reference as its 16-byte slot encoding.
    m.func(
        "fp_vref",
        &[
            ("pmap", t::map(t::bytes(), t::bytes())),
            ("omap", t::map(t::bytes(), t::tuple(vec![t::bytes(), t::u64()]))),
            ("x", vref_ty),
        ],
        r(t::bytes()),
        |f| {
            f.if_(eq(field(v("x"), "kind"), u64c(1)), |f| {
                f.let_("p", mget_opt(v("pmap"), field(v("x"), "id")));
                fail(f, not(is_some(v("p"))), FP_LOCAL_REFERENCE_INVALID);
                f.ret_ok(unwrap(v("p")));
            });
            f.let_("o", mget_opt(v("omap"), field(v("x"), "id")));
            fail(f, not(is_some(v("o"))), FP_LOCAL_REFERENCE_INVALID);
            f.let_("s", unwrap(v("o")));
            fail(f, ge(field(v("x"), "idx"), tget(v("s"), 1)), FP_LOCAL_REFERENCE_INVALID);
            f.ret_ok(concat(vnew(
                t::bytes(),
                vec![tget(v("s"), 0), call("fp_be32", vec![field(v("x"), "idx")])],
            )));
        },
    );
    // Reverse ownership counts over the whole inventory (one pass each).
    m.func("fp_owned", &[("inv", inv_ty.clone())], own_ty(), |f| {
        f.var("pown", mnew(t::bytes(), t::u64(), vec![]));
        f.var("bown", mnew(t::bytes(), t::u64(), vec![]));
        f.var("oown", mnew(t::bytes(), t::u64(), vec![]));
        f.let_("fns", field(v("inv"), "fns"));
        f.let_("blks", field(v("inv"), "blks"));
        f.let_("prms", field(v("inv"), "prms"));
        f.let_("ops", field(v("inv"), "ops"));
        // Pre-populate in ascending identity order (appends only).
        f.for_u64("i", u64c(0), vlen(v("fns")), |f| {
            f.let_("id", field(vget(v("fns"), v("i")), "id"));
            f.set("pown", mins(v("pown"), v("id"), u64c(0)));
            f.set("bown", mins(v("bown"), v("id"), u64c(0)));
        });
        f.for_u64("i", u64c(0), vlen(v("blks")), |f| {
            f.let_("b", vget(v("blks"), v("i")));
            f.set("oown", mins(v("oown"), field(v("b"), "id"), u64c(0)));
            f.let_("fo", field(v("b"), "function"));
            f.match_opt(
                mget_opt(v("bown"), v("fo")),
                "c",
                |f| f.set("bown", mins(v("bown"), v("fo"), add(v("c"), u64c(1)))),
                |_| {},
            );
        });
        f.for_u64("i", u64c(0), vlen(v("prms")), |f| {
            f.let_("ow", field(vget(v("prms"), v("i")), "owner"));
            f.match_opt(
                mget_opt(v("pown"), v("ow")),
                "c",
                |f| f.set("pown", mins(v("pown"), v("ow"), add(v("c"), u64c(1)))),
                |_| {},
            );
        });
        f.for_u64("i", u64c(0), vlen(v("ops")), |f| {
            f.let_("ob", field(vget(v("ops"), v("i")), "block"));
            f.match_opt(
                mget_opt(v("oown"), v("ob")),
                "c",
                |f| f.set("oown", mins(v("oown"), v("ob"), add(v("c"), u64c(1)))),
                |_| {},
            );
        });
        keep(f, &["inv"]);
        f.ret(tup(vec![v("pown"), v("bown"), v("oown")]));
    });
    // The fingerprint body of one function:
    // (body, body length, inventory work, memo).
    m.func(
        "fp_core",
        &[
            ("idx", t::map(t::bytes(), t::u64())),
            ("all_fns", t::vec(m.ty(FUN))),
            ("all_prms", t::vec(m.ty(PRM))),
            ("all_blks", t::vec(m.ty(BLK))),
            ("all_ops", t::vec(m.ty(OP))),
            ("fid", t::bytes()),
            ("chk", t::bool()),
            ("own", own_ty()),
            ("memo", memo_ty()),
        ],
        r(core_ty),
        |f| {
            f.var("memo", v("memo"));
            need(f, "fun", "all_fns", KIND_FUNCTION, v("fid"), FP_FUNCTION_MISSING);
            // Pass 1: slot maps over the listed entities.
            f.var("pmap", mnew(t::bytes(), t::bytes(), vec![]));
            f.var(
                "omap",
                mnew(t::bytes(), t::tuple(vec![t::bytes(), t::u64()]), vec![]),
            );
            f.var("bslot", mnew(t::bytes(), t::u64(), vec![]));
            f.var("ptys", vnew(t::bytes(), vec![]));
            f.var("btys", vnew(t::bytes(), vec![]));
            f.var("blks", vnew(m_ty(BLK), vec![]));
            f.var("opsv", vnew(m_ty(OP), vec![]));
            f.let_("fps", field(v("fun"), "params"));
            f.for_u64("i", u64c(0), vlen(v("fps")), |f| {
                f.let_("pid", vget(v("fps"), v("i")));
                need(f, "prm", "all_prms", KIND_PARAMETER, v("pid"), FP_INVENTORY_INVALID);
                fail(
                    f,
                    or(
                        or(
                            ne(field(v("prm"), "owner"), v("fid")),
                            ne(field(v("prm"), "role"), u64c(1)),
                        ),
                        ne(field(v("prm"), "ordinal"), v("i")),
                    ),
                    FP_INVENTORY_INVALID,
                );
                f.set(
                    "pmap",
                    mins(
                        v("pmap"),
                        v("pid"),
                        concat(vnew(
                            t::bytes(),
                            vec![lit32(1), lit32(1), lit32(0), call("fp_be32", vec![v("i")])],
                        )),
                    ),
                );
                f.set("ptys", push(v("ptys"), field(v("prm"), "ty")));
            });
            f.let_("fbs", field(v("fun"), "blocks"));
            f.for_u64("bi", u64c(0), vlen(v("fbs")), |f| {
                f.let_("bid", vget(v("fbs"), v("bi")));
                need(f, "blk", "all_blks", KIND_BLOCK, v("bid"), FP_INVENTORY_INVALID);
                fail(
                    f,
                    or(ne(field(v("blk"), "function"), v("fid")), mhas(v("bslot"), v("bid"))),
                    FP_INVENTORY_INVALID,
                );
                f.set("bslot", mins(v("bslot"), v("bid"), v("bi")));
                f.let_("bps", field(v("blk"), "params"));
                f.for_u64("pi", u64c(0), vlen(v("bps")), |f| {
                    f.let_("pid", vget(v("bps"), v("pi")));
                    need(f, "prm", "all_prms", KIND_PARAMETER, v("pid"), FP_INVENTORY_INVALID);
                    fail(
                        f,
                        or(
                            or(
                                ne(field(v("prm"), "owner"), v("bid")),
                                ne(field(v("prm"), "role"), u64c(2)),
                            ),
                            ne(field(v("prm"), "ordinal"), v("pi")),
                        ),
                        FP_INVENTORY_INVALID,
                    );
                    f.set(
                        "pmap",
                        mins(
                            v("pmap"),
                            v("pid"),
                            concat(vnew(
                                t::bytes(),
                                vec![
                                    lit32(1),
                                    lit32(2),
                                    call("fp_be32", vec![v("bi")]),
                                    call("fp_be32", vec![v("pi")]),
                                ],
                            )),
                        ),
                    );
                    f.set("btys", push(v("btys"), field(v("prm"), "ty")));
                });
                f.let_("bos", field(v("blk"), "ops"));
                f.for_u64("oi", u64c(0), vlen(v("bos")), |f| {
                    f.let_("oid", vget(v("bos"), v("oi")));
                    need(f, "op", "all_ops", KIND_OPERATION, v("oid"), FP_INVENTORY_INVALID);
                    fail(
                        f,
                        or(
                            ne(field(v("op"), "block"), v("bid")),
                            ne(field(v("op"), "ordinal"), v("oi")),
                        ),
                        FP_INVENTORY_INVALID,
                    );
                    f.set(
                        "omap",
                        mins(
                            v("omap"),
                            v("oid"),
                            tup(vec![
                                concat(vnew(
                                    t::bytes(),
                                    vec![
                                        lit32(2),
                                        call("fp_be32", vec![v("bi")]),
                                        call("fp_be32", vec![v("oi")]),
                                    ],
                                )),
                                vlen(field(v("op"), "results")),
                            ]),
                        ),
                    );
                    f.set("opsv", push(v("opsv"), v("op")));
                });
                f.if_(v("chk"), |f| {
                    fail(
                        f,
                        ne(mget(tget(v("own"), 2), v("bid")), vlen(v("bos"))),
                        FP_INVENTORY_INVALID,
                    );
                });
                f.set("blks", push(v("blks"), v("blk")));
            });
            // No unlisted parameter or block claims the function.
            f.if_(v("chk"), |f| {
                fail(
                    f,
                    or(
                        ne(mget(tget(v("own"), 0), v("fid")), vlen(v("fps"))),
                        ne(mget(tget(v("own"), 1), v("fid")), vlen(v("fbs"))),
                    ),
                    FP_INVENTORY_INVALID,
                );
            });
            f.let_("nops", vlen(v("opsv")));
            f.let_(
                "work",
                add(
                    add(vlen(v("fps")), vlen(v("btys"))),
                    add(vlen(v("fbs")), v("nops")),
                ),
            );
            fail(
                f,
                or(gt(v("nops"), u64c(MAX_OPS)), gt(v("work"), u64c(MAX_ENTITIES))),
                FP_RESOURCE_LIMIT,
            );

            // Pass 2: the body.
            f.var("out", vnew(t::bytes(), vec![]));
            f.var("n", u64c(0));
            f.let_("tps", field(v("fun"), "tparams"));
            put_len(f, v("tps"));
            f.for_u64("i", u64c(0), vlen(v("tps")), |f| {
                put(f, call("fp_be32", vec![vget(v("tps"), v("i"))]), u64c(4));
            });
            put_len(f, v("ptys"));
            f.for_u64("i", u64c(0), vlen(v("ptys")), |f| {
                tyc(f, vget(v("ptys"), v("i")), v("fid"));
            });
            tyc(f, field(v("fun"), "result"), v("fid"));
            f.let_("efs", field(v("fun"), "effects"));
            put_len(f, v("efs"));
            f.for_u64("i", u64c(0), vlen(v("efs")), |f| {
                eref(f, vget(v("efs"), v("i")));
            });
            slot(f, field(v("fun"), "entry"));
            put_len(f, v("blks"));
            f.var("bp", u64c(0));
            f.var("ok", u64c(0));
            f.for_u64("bi", u64c(0), vlen(v("blks")), |f| {
                f.let_("blk", vget(v("blks"), v("bi")));
                put(f, call("fp_be32", vec![field(v("blk"), "reach")]), u64c(4));
                f.let_("np", vlen(field(v("blk"), "params")));
                put(f, call("fp_be64", vec![v("np")]), u64c(8));
                f.for_u64("j", u64c(0), v("np"), |f| {
                    tyc(f, vget(v("btys"), v("bp")), v("fid"));
                    f.set("bp", add(v("bp"), u64c(1)));
                });
                f.let_("no", vlen(field(v("blk"), "ops")));
                put(f, call("fp_be64", vec![v("no")]), u64c(8));
                f.for_u64("j", u64c(0), v("no"), |f| {
                    f.let_("op", vget(v("opsv"), v("ok")));
                    f.set("ok", add(v("ok"), u64c(1)));
                    put(f, call("fp_be32", vec![field(v("op"), "opcode")]), u64c(4));
                    f.let_("ods", field(v("op"), "operands"));
                    put_len(f, v("ods"));
                    f.for_u64("k", u64c(0), vlen(v("ods")), |f| {
                        vref(f, vget(v("ods"), v("k")));
                    });
                    f.let_("rts", field(v("op"), "results"));
                    put_len(f, v("rts"));
                    f.for_u64("k", u64c(0), vlen(v("rts")), |f| {
                        tyc(f, vget(v("rts"), v("k")), v("fid"));
                    });
                    imm(f, field(v("op"), "imm"));
                });
                term(f, field(v("blk"), "term"));
            });
            f.let_("cts", field(v("fun"), "contracts"));
            put_len(f, v("cts"));
            f.for_u64("i", u64c(0), vlen(v("cts")), |f| {
                eref(f, vget(v("cts"), v("i")));
            });
            put(f, call("fp_be32", vec![field(v("fun"), "vis")]), u64c(4));
            keep(f, &["own", "all_fns", "all_prms", "all_blks", "all_ops"]);
            f.ret_ok(tup(vec![concat(v("out")), v("n"), v("work"), v("memo")]));
        },
    );
    // Preimage, work ceiling, and the domain-separated raw BLAKE3.
    m.func(
        "fp_hash",
        &[
            ("epoch", t::bytes()),
            ("body", t::bytes()),
            ("len", t::u64()),
            ("work", t::u64()),
        ],
        r(t::bytes()),
        |f| {
            fail(
                f,
                gt(
                    add(add(mul(v("len"), u64c(2)), u64c(88)), v("work")),
                    u64c(MAX_WORK),
                ),
                FP_RESOURCE_LIMIT,
            );
            f.let_(
                "pre",
                concat(vnew(
                    t::bytes(),
                    vec![
                        bytesc(FP_DOMAIN),
                        bytesc(b"SLEYSFP1"),
                        lit32(1),
                        v("epoch"),
                        bytesc(&sley_ssmc::fingerprint::SSMC1_FIELD_SCHEMA_HASH),
                        lit32(5),
                        call("fp_be64", vec![v("len")]),
                        v("body"),
                    ],
                )),
            );
            let fixed = FP_DOMAIN.len() as u64 + 8 + 4 + 32 + 32 + 4 + 8;
            f.if_(lt(add(v("len"), u64c(fixed)), u64c(SMALL_HASH)), |f| {
                f.ret_ok(blake3(v("pre")));
            });
            f.ret_ok(blake3_large(v("pre")));
        },
    );
    // The semantic fingerprint of one function (listed-entity checks).
    m.func(
        "fp_function",
        &[("inv", inv_ty.clone()), ("fid", t::bytes()), ("epoch", t::bytes())],
        r(t::bytes()),
        |f| {
            f.let_(
                "c",
                tri(call(
                    "fp_core",
                    vec![
                        field(v("inv"), "index"),
                        field(v("inv"), "fns"),
                        field(v("inv"), "prms"),
                        field(v("inv"), "blks"),
                        field(v("inv"), "ops"),
                        v("fid"),
                        boolc(false),
                        empty_own(),
                        mnew(t::bytes(), ent_ty(), vec![]),
                    ],
                )),
            );
            keep(f, &["inv"]);
            f.ret(call(
                "fp_hash",
                vec![v("epoch"), tget(v("c"), 0), tget(v("c"), 1), tget(v("c"), 2)],
            ));
        },
    );
    // Fingerprints of `fids` in order, with the exact gate narrowing (no
    // unlisted claimant) and one type memo shared by all functions.
    m.func(
        "fp_closure",
        &[("inv", inv_ty), ("fids", t::vec(t::bytes())), ("epoch", t::bytes())],
        r(t::vec(t::bytes())),
        |f| {
            f.let_("own", call("fp_owned", vec![v("inv")]));
            // Project the inventory once: each projection of a large part
            // walks it for value units.
            f.let_("idx", field(v("inv"), "index"));
            f.let_("all_fns", field(v("inv"), "fns"));
            f.let_("all_prms", field(v("inv"), "prms"));
            f.let_("all_blks", field(v("inv"), "blks"));
            f.let_("all_ops", field(v("inv"), "ops"));
            f.var("memo", mnew(t::bytes(), ent_ty(), vec![]));
            f.var("fps", vnew(t::bytes(), vec![]));
            f.for_u64("i", u64c(0), vlen(v("fids")), |f| {
                f.let_(
                    "c",
                    tri(call(
                        "fp_core",
                        vec![
                            v("idx"),
                            v("all_fns"),
                            v("all_prms"),
                            v("all_blks"),
                            v("all_ops"),
                            vget(v("fids"), v("i")),
                            boolc(true),
                            v("own"),
                            v("memo"),
                        ],
                    )),
                );
                f.set(
                    "fps",
                    push(
                        v("fps"),
                        tri(call(
                            "fp_hash",
                            vec![v("epoch"), tget(v("c"), 0), tget(v("c"), 1), tget(v("c"), 2)],
                        )),
                    ),
                );
                f.set("memo", tget(v("c"), 3));
            });
            keep(f, &["inv", "own", "all_fns", "all_prms", "all_blks", "all_ops"]);
            f.ret_ok(v("fps"));
        },
    );

    // ----- test entries -----
    // Loads S once; `closure` selects fp_closure, otherwise fp_function each.
    m.func(
        "t_fp_closure",
        &[
            ("objects", t::vec(t::bytes())),
            ("epoch", t::bytes()),
            ("fids", t::vec(t::bytes())),
            ("closure", t::bool()),
        ],
        r(t::vec(t::bytes())),
        |f| {
            f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
            f.if_(v("closure"), |f| {
                f.ret(call("fp_closure", vec![v("inv"), v("fids"), v("epoch")]));
            });
            f.var("fps", vnew(t::bytes(), vec![]));
            f.for_u64("i", u64c(0), vlen(v("fids")), |f| {
                f.set(
                    "fps",
                    push(
                        v("fps"),
                        tri(call(
                            "fp_function",
                            vec![v("inv"), vget(v("fids"), v("i")), v("epoch")],
                        )),
                    ),
                );
            });
            f.ret_ok(v("fps"));
        },
    );
}

/// A model type by name.
fn m_ty(name: &str) -> Ty {
    Module::new(super::TOOLCHAIN_SEED).ty(name)
}
