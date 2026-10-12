//! `sr_*`: the `STATE_ROOT_V1` standalone record — strict decoding, the
//! `StateRoot` digest, and the facts the build driver binds.
//!
//! A root binds every entity of S, so it can exceed the 1 MiB conversion
//! bridge. The reader therefore works through a sliding window: a
//! `Vector<UInt(8)>` copy of at most 4 KiB of the root at a time, refilled
//! by `sr_ensure` before each read. Positions stay absolute.

use super::codes::{
    SCB_CONTRACT_UNKNOWN, SCB_DIGEST_MISMATCH, SCB_EPOCH_MISMATCH, SCB_FIELD_DUPLICATE,
    SCB_FIELD_MISSING, SCB_FIELD_ORDER, SCB_FIELD_UNKNOWN, SCB_LENGTH_OVERFLOW,
    SCB_MAGIC_INVALID, SCB_MAP_DUPLICATE, SCB_MAP_ORDER, SCB_RESOURCE_LIMIT, SCB_TRAILING_BYTES,
    SCB_VERSION_UNSUPPORTED, TOOL_ROOT_BINDING_MISMATCH,
};
use super::rt::{MAX_STANDALONE, code, r};
use crate::seed::*;

/// Root facts record.
pub const ROOT: &str = "Root";
/// Window size.
const WINDOW: u64 = 4096;
/// The largest single read the parser needs inside a window.
const NEED: u64 = 80;

fn win_ty() -> Ty {
    t::tuple(vec![t::vec(t::u8()), t::u64()])
}

/// Refills the window when fewer than `NEED` bytes remain past `p`.
fn ensure(f: &mut FnB<'_>) {
    f.let_(
        "#w",
        call("sr_ensure", vec![v("bs"), v("total"), v("win"), v("ws"), v("p")]),
    );
    f.set("win", tget(v("#w"), 0));
    f.set("ws", tget(v("#w"), 1));
}

/// Reads with a window-relative reader: binds `out`, advances `p`.
fn wrd(f: &mut FnB<'_>, out: &str, reader: &str, extra: Vec<E>) {
    ensure(f);
    let mut args = vec![v("win"), sub(v("p"), v("ws")), sub(v("e"), v("ws"))];
    args.extend(extra);
    f.let_("#r", tri(call(reader, args)));
    f.let_(out, tget(v("#r"), 0));
    f.set("p", add(v("ws"), tget(v("#r"), 1)));
}

/// Defines the `sr_*` functions.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    m.record(
        ROOT,
        &[
            ("root", t::bytes()),
            ("workspace", t::bytes()),
            ("keys", t::vec(t::bytes())),
            ("vals", t::vec(t::bytes())),
            ("entries", t::vec(t::bytes())),
        ],
    );
    // Window refill: the window covers [ws, ws + len) of `bs`.
    m.func(
        "sr_ensure",
        &[
            ("bs", t::bytes()),
            ("total", t::u64()),
            ("win", t::vec(t::u8())),
            ("ws", t::u64()),
            ("p", t::u64()),
        ],
        win_ty(),
        |f| {
            f.if_(
                and(
                    ge(v("p"), v("ws")),
                    ge(add(v("ws"), vlen(v("win"))), call("sr_min", vec![add(v("p"), u64c(NEED)), v("total")])),
                ),
                |f| f.ret(tup(vec![v("win"), v("ws")])),
            );
            f.let_("len", call("sr_min", vec![u64c(WINDOW), sub(v("total"), v("p"))]));
            f.ret(tup(vec![b2v(slice(v("bs"), v("p"), v("len"))), v("p")]));
        },
    );
    m.func("sr_min", &[("a", t::u64()), ("b", t::u64())], t::u64(), |f| {
        f.ret(select(lt(v("a"), v("b")), v("a"), v("b")));
    });
    // Fixed 32 bytes at [p, p + 32) of an exact 32-byte region.
    // Decodes a stored root under `epoch`.
    m.func(
        "sr_root",
        &[("bs", t::bytes()), ("epoch", t::bytes())],
        r(m.ty(ROOT)),
        |f| {
            f.let_("total", call("rt_blen", vec![v("bs")]));
            f.if_(gt(v("total"), u64c(MAX_STANDALONE)), |f| {
                f.ret_err(code(SCB_RESOURCE_LIMIT));
            });
            f.var("win", vnew(t::u8(), vec![]));
            f.var("ws", u64c(0));
            f.var("p", u64c(0));
            f.var("e", v("total"));
            f.if_(lt(v("total"), u64c(8)), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
            f.if_(ne(slice(v("bs"), u64c(0), u64c(8)), bytesc(b"SLEYSCB1")), |f| {
                f.ret_err(code(SCB_MAGIC_INVALID));
            });
            f.set("p", u64c(8));
            wrd(f, "version", "rt_rd_uvar", vec![u32c(64)]);
            f.if_(ne(v("version"), u64c(1)), |f| f.ret_err(code(SCB_VERSION_UNSUPPORTED)));
            wrd(f, "contract", "rt_rd_uvar", vec![u32c(32)]);
            f.if_(ne(v("contract"), u64c(160)), |f| f.ret_err(code(SCB_CONTRACT_UNKNOWN)));
            f.if_(lt(sub(v("total"), v("p")), u64c(32)), |f| {
                f.ret_err(code(SCB_LENGTH_OVERFLOW));
            });
            f.let_("env_epoch", slice(v("bs"), v("p"), u64c(32)));
            f.set("p", add(v("p"), u64c(32)));
            wrd(f, "plen", "rt_rd_len", vec![u64c(MAX_STANDALONE)]);
            f.let_("ps", v("p"));
            f.if_(gt(add(v("p"), v("plen")), v("total")), |f| {
                f.ret_err(code(SCB_LENGTH_OVERFLOW));
            });
            f.let_("pe", add(v("p"), v("plen")));
            f.if_(lt(sub(v("total"), v("pe")), u64c(32)), |f| {
                f.ret_err(code(SCB_LENGTH_OVERFLOW));
            });
            f.if_(gt(sub(v("total"), v("pe")), u64c(32)), |f| {
                f.ret_err(code(SCB_TRAILING_BYTES));
            });
            f.let_(
                "digest",
                blake3_large(concat(vnew(
                    t::bytes(),
                    vec![bytesc(b"sley2.state-root.v1"), slice(v("bs"), u64c(0), v("pe"))],
                ))),
            );
            f.if_(ne(slice(v("bs"), v("pe"), u64c(32)), v("digest")), |f| {
                f.ret_err(code(SCB_DIGEST_MISMATCH));
            });
            f.if_(ne(v("env_epoch"), v("epoch")), |f| f.ret_err(code(SCB_EPOCH_MISMATCH)));
            // Payload: nine required fields.
            f.set("p", v("ps"));
            f.set("e", v("pe"));
            wrd(f, "count", "rt_rd_fields", vec![]);
            f.if_(lt(v("count"), u64c(9)), |f| f.ret_err(code(SCB_FIELD_MISSING)));
            f.if_(gt(v("count"), u64c(9)), |f| f.ret_err(code(SCB_FIELD_UNKNOWN)));
            f.var("workspace", bytesc(&[]));
            f.var("keys", vnew(t::bytes(), vec![]));
            f.var("vals", vnew(t::bytes(), vec![]));
            f.var("entries", vnew(t::bytes(), vec![]));
            f.var("prev", u64c(0));
            f.for_u64("i", u64c(1), u64c(10), |f| {
                wrd(f, "tag", "rt_rd_uvar", vec![u32c(32)]);
                f.if_(eq(v("tag"), v("prev")), |f| f.ret_err(code(SCB_FIELD_DUPLICATE)));
                f.if_(lt(v("tag"), v("prev")), |f| f.ret_err(code(SCB_FIELD_ORDER)));
                f.set("prev", v("tag"));
                f.if_(ne(v("tag"), v("i")), |f| {
                    f.if_(le(v("tag"), u64c(9)), |f| f.ret_err(code(SCB_FIELD_ORDER)));
                    f.ret_err(code(SCB_FIELD_UNKNOWN));
                });
                wrd(f, "flen", "rt_rd_len", vec![u64c(MAX_STANDALONE)]);
                f.let_("fs", v("p"));
                f.if_(gt(add(v("p"), v("flen")), v("e")), |f| {
                    f.ret_err(code(SCB_LENGTH_OVERFLOW));
                });
                f.let_("fe", add(v("p"), v("flen")));
                // Fixed fields: 1 workspace, 2 epoch, 6..8 roots.
                f.if_(
                    or(
                        or(eq(v("i"), u64c(1)), eq(v("i"), u64c(2))),
                        and(ge(v("i"), u64c(6)), le(v("i"), u64c(8))),
                    ),
                    |f| {
                        f.if_(lt(v("flen"), u64c(32)), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
                        f.if_(gt(v("flen"), u64c(32)), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
                        f.let_("x", slice(v("bs"), v("fs"), u64c(32)));
                        f.if_(eq(v("i"), u64c(1)), |f| f.set("workspace", v("x")));
                        f.if_(eq(v("i"), u64c(2)), |f| {
                            f.if_(ne(v("x"), v("epoch")), |f| f.ret_err(code(SCB_EPOCH_MISMATCH)));
                        });
                    },
                );
                // Bindings map.
                f.if_(eq(v("i"), u64c(3)), |f| {
                    f.let_("outer_e", v("e"));
                    f.set("e", v("fe"));
                    wrd(f, "n", "rt_rd_count", vec![]);
                    f.for_u64("j", u64c(0), v("n"), |f| {
                        wrd(f, "klen", "rt_rd_len", vec![u64c(MAX_STANDALONE)]);
                        f.if_(ne(v("klen"), u64c(32)), |f| {
                            f.ret_err(select(
                                lt(v("klen"), u64c(32)),
                                u32c(SCB_LENGTH_OVERFLOW),
                                u32c(SCB_TRAILING_BYTES),
                            ));
                        });
                        f.if_(gt(add(v("p"), u64c(32)), v("e")), |f| {
                            f.ret_err(code(SCB_LENGTH_OVERFLOW));
                        });
                        f.let_("key", slice(v("bs"), v("p"), u64c(32)));
                        f.set("p", add(v("p"), u64c(32)));
                        wrd(f, "vlen_", "rt_rd_len", vec![u64c(MAX_STANDALONE)]);
                        f.if_(ne(v("vlen_"), u64c(32)), |f| {
                            f.ret_err(select(
                                lt(v("vlen_"), u64c(32)),
                                u32c(SCB_LENGTH_OVERFLOW),
                                u32c(SCB_TRAILING_BYTES),
                            ));
                        });
                        f.if_(gt(add(v("p"), u64c(32)), v("e")), |f| {
                            f.ret_err(code(SCB_LENGTH_OVERFLOW));
                        });
                        f.let_("val", slice(v("bs"), v("p"), u64c(32)));
                        f.set("p", add(v("p"), u64c(32)));
                        f.if_(gt(v("j"), u64c(0)), |f| {
                            f.let_("last", vget(v("keys"), sub(v("j"), u64c(1))));
                            f.if_(eq(v("last"), v("key")), |f| f.ret_err(code(SCB_MAP_DUPLICATE)));
                            f.if_(gt(v("last"), v("key")), |f| f.ret_err(code(SCB_MAP_ORDER)));
                        });
                        f.set("keys", push(v("keys"), v("key")));
                        f.set("vals", push(v("vals"), v("val")));
                    });
                    f.if_(ne(v("p"), v("fe")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
                    f.set("e", v("outer_e"));
                });
                // Sets of 32-byte identities (4 entries, 5 dependency roots).
                f.if_(or(eq(v("i"), u64c(4)), eq(v("i"), u64c(5))), |f| {
                    f.let_("outer_e", v("e"));
                    f.set("e", v("fe"));
                    wrd(f, "n", "rt_rd_count", vec![]);
                    f.var("set", vnew(t::bytes(), vec![]));
                    f.for_u64("j", u64c(0), v("n"), |f| {
                        wrd(f, "elen", "rt_rd_len", vec![u64c(MAX_STANDALONE)]);
                        f.if_(ne(v("elen"), u64c(32)), |f| {
                            f.ret_err(select(
                                lt(v("elen"), u64c(32)),
                                u32c(SCB_LENGTH_OVERFLOW),
                                u32c(SCB_TRAILING_BYTES),
                            ));
                        });
                        f.if_(gt(add(v("p"), u64c(32)), v("e")), |f| {
                            f.ret_err(code(SCB_LENGTH_OVERFLOW));
                        });
                        f.let_("x", slice(v("bs"), v("p"), u64c(32)));
                        f.set("p", add(v("p"), u64c(32)));
                        f.if_(gt(v("j"), u64c(0)), |f| {
                            f.let_("last", vget(v("set"), sub(v("j"), u64c(1))));
                            f.if_(eq(v("last"), v("x")), |f| f.ret_err(code(SCB_MAP_DUPLICATE)));
                            f.if_(gt(v("last"), v("x")), |f| f.ret_err(code(SCB_MAP_ORDER)));
                        });
                        f.set("set", push(v("set"), v("x")));
                    });
                    f.if_(ne(v("p"), v("fe")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
                    f.set("e", v("outer_e"));
                    f.if_(eq(v("i"), u64c(4)), |f| f.set("entries", v("set")));
                    // The toolchain closure is self-contained: no external
                    // dependency roots.
                    f.if_(and(eq(v("i"), u64c(5)), gt(vlen(v("set")), u64c(0))), |f| {
                        f.ret_err(code(TOOL_ROOT_BINDING_MISMATCH));
                    });
                });
                // Interpretation flags: epoch 1 admits only the empty set.
                f.if_(eq(v("i"), u64c(9)), |f| {
                    f.let_("outer_e", v("e"));
                    f.set("e", v("fe"));
                    wrd(f, "n", "rt_rd_count", vec![]);
                    f.if_(gt(v("n"), u64c(0)), |f| f.ret_err(code(SCB_FIELD_UNKNOWN)));
                    f.if_(ne(v("p"), v("fe")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
                    f.set("e", v("outer_e"));
                });
                f.set("p", v("fe"));
            });
            f.if_(ne(v("p"), v("e")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
            f.ret_ok(rec(
                ROOT,
                vec![v("digest"), v("workspace"), v("keys"), v("vals"), v("entries")],
            ));
        },
    );
}
