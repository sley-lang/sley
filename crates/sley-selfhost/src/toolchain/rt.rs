//! Toolchain runtime library: byte/integer conversion, strict SCB1 readers,
//! and byte writers, written as Sley functions.
//!
//! Every internal integer is `UInt(64)` except shift amounts (`UInt(32)`)
//! and bytes (`UInt(8)`); SSMC1 has no width conversion, so `UInt(8)` and
//! `UInt(64)` meet through two 256-entry constant maps. Readers take the
//! decoded buffer `b: Vector<UInt(8)>` (and, where they slice, the same
//! bytes `bs: Bytes`), a position `p`, and an exclusive end `e`; they return
//! `Result<(value, new_position), UInt(32)>`.

use sley_ssmc::{ConstData, ConstValue, MapEntryConst};

use super::codes::{SCB_INTEGER_OVERFLOW, SCB_LENGTH_OVERFLOW, SCB_RESOURCE_LIMIT, SCB_VARINT_NON_MINIMAL};
use crate::seed::*;

/// SCB1 standalone byte ceiling.
pub const MAX_STANDALONE: u64 = 67_108_864;
/// SCB1 byte-payload ceiling.
pub const MAX_BYTE_PAYLOAD: u64 = 16_777_216;
/// SCB1 collection-element ceiling.
pub const MAX_COLLECTION: u64 = 1_000_000;
/// SCB1 record-field ceiling.
pub const MAX_RECORD_FIELDS: u64 = 65_535;

/// The toolchain error type.
#[must_use]
pub fn err_ty() -> Ty {
    t::u32()
}

/// `Result<(T, UInt(64)), UInt(32)>`.
#[must_use]
pub fn rd_ty(value: Ty) -> Ty {
    t::res(t::tuple(vec![value, t::u64()]), err_ty())
}

/// `Result<T, UInt(32)>`.
#[must_use]
pub fn r(value: Ty) -> Ty {
    t::res(value, err_ty())
}

/// Error-code literal.
#[must_use]
pub fn code(value: u32) -> E {
    u32c(value)
}

fn sorted_map(key: Ty, value: Ty, mut entries: Vec<(ConstValue, ConstValue)>) -> E {
    entries.sort_by_key(|(k, _)| sley_mutate::encode_const_value(k).expect("canonical key"));
    E::Lit(ConstValue {
        value_type: t::map(key, value),
        data: ConstData::Map(
            entries
                .into_iter()
                .map(|(key, value)| MapEntryConst { key, value })
                .collect(),
        ),
    })
}

fn uint(value: u128, ty: Ty) -> ConstValue {
    ConstValue {
        value_type: ty,
        data: ConstData::UInt(value),
    }
}

/// The `UInt(8) -> UInt(64)` table.
#[must_use]
pub fn u8_u64_table() -> E {
    sorted_map(
        t::u8(),
        t::u64(),
        (0..=255_u128).map(|i| (uint(i, t::u8()), uint(i, t::u64()))).collect(),
    )
}

/// The `UInt(64) -> UInt(8)` table (keys 0..=255).
#[must_use]
pub fn u64_u8_table() -> E {
    sorted_map(
        t::u64(),
        t::u8(),
        (0..=255_u128).map(|i| (uint(i, t::u64()), uint(i, t::u8()))).collect(),
    )
}

/// Binds `out` to the value and sets `pos` from a reader call returning
/// `Result<(T, UInt(64)), UInt(32)>` (propagating `Err`).
pub fn rd(f: &mut FnB<'_>, out: &str, pos: &str, call_expr: E) {
    f.let_("#rd", tri(call_expr));
    f.let_(out, tget(v("#rd"), 0));
    f.set(pos, tget(v("#rd"), 1));
}

/// Reader call with the standard `(b, p, e)` arguments plus extras.
#[must_use]
pub fn rcall(name: &str, extra: Vec<E>) -> E {
    let mut args = vec![v("b"), v("p"), v("e")];
    args.extend(extra);
    call(name, args)
}

/// Defines the runtime library.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    let rdu = rd_ty(t::u64());
    m.func("rt_b", &[("x", t::u8())], t::u64(), |f| {
        f.ret(mget(u8_u64_table(), v("x")));
    });
    m.func("rt_byte", &[("x", t::u64())], t::u8(), |f| {
        f.ret(mget(u64_u8_table(), v("x")));
    });
    // One byte as UInt(64).
    m.func(
        "rt_rd_u8",
        &[("b", t::vec(t::u8())), ("p", t::u64()), ("e", t::u64())],
        rdu.clone(),
        |f| {
            f.if_(ge(v("p"), v("e")), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
            f.ret_ok(tup(vec![
                call("rt_b", vec![vget(v("b"), v("p"))]),
                add(v("p"), u64c(1)),
            ]));
        },
    );
    // Canonical LE7 unsigned varint within `width` bits (1..=64), exactly the
    // `sley-scb1` reader: minimal encoding, shift-63 and shift-64 overflow
    // rules, then the width check.
    m.func(
        "rt_rd_uvar",
        &[
            ("b", t::vec(t::u8())),
            ("p", t::u64()),
            ("e", t::u64()),
            ("width", t::u32()),
        ],
        rdu.clone(),
        |f| {
            f.var("value", u64c(0));
            f.var("shift", u32c(0));
            f.var("count", u64c(0));
            f.var("p", v("p"));
            f.loop_(|f| {
                f.if_(ge(v("p"), v("e")), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
                f.let_("byte", call("rt_b", vec![vget(v("b"), v("p"))]));
                f.set("p", add(v("p"), u64c(1)));
                f.set("count", add(v("count"), u64c(1)));
                f.let_("payload", rem(v("byte"), u64c(128)));
                f.if_(
                    and(ge(v("shift"), u32c(64)), ne(v("payload"), u64c(0))),
                    |f| f.ret_err(code(SCB_INTEGER_OVERFLOW)),
                );
                f.if_(lt(v("shift"), u32c(64)), |f| {
                    f.if_(
                        and(eq(v("shift"), u32c(63)), gt(v("payload"), u64c(1))),
                        |f| f.ret_err(code(SCB_INTEGER_OVERFLOW)),
                    );
                    f.set("value", add(v("value"), shl(v("payload"), v("shift"))));
                });
                f.if_(lt(v("byte"), u64c(128)), |f| {
                    f.if_(
                        and(gt(v("count"), u64c(1)), eq(v("payload"), u64c(0))),
                        |f| f.ret_err(code(SCB_VARINT_NON_MINIMAL)),
                    );
                    f.if_(lt(v("width"), u32c(64)), |f| {
                        f.if_(
                            ge(v("value"), shl(u64c(1), v("width"))),
                            |f| f.ret_err(code(SCB_INTEGER_OVERFLOW)),
                        );
                    });
                    f.ret_ok(tup(vec![v("value"), v("p")]));
                });
                f.set("shift", add(v("shift"), u32c(7)));
                f.if_(ge(v("shift"), u32c(71)), |f| {
                    f.ret_err(code(SCB_INTEGER_OVERFLOW));
                });
            });
        },
    );
    // A 64-bit length bounded by `max` (exceeding it is a resource limit).
    m.func(
        "rt_rd_len",
        &[
            ("b", t::vec(t::u8())),
            ("p", t::u64()),
            ("e", t::u64()),
            ("max", t::u64()),
        ],
        rdu.clone(),
        |f| {
            f.var("p", v("p"));
            rd(f, "len", "p", rcall("rt_rd_uvar", vec![u32c(64)]));
            f.if_(gt(v("len"), v("max")), |f| f.ret_err(code(SCB_RESOURCE_LIMIT)));
            f.ret_ok(tup(vec![v("len"), v("p")]));
        },
    );
    // A sized payload: `(start, end)`; the new position is `end`.
    m.func(
        "rt_rd_payload",
        &[("b", t::vec(t::u8())), ("p", t::u64()), ("e", t::u64())],
        rd_ty(t::u64()),
        |f| {
            f.var("p", v("p"));
            rd(
                f,
                "len",
                "p",
                rcall("rt_rd_len", vec![u64c(MAX_STANDALONE)]),
            );
            f.match_res(
                try_add(v("p"), v("len")),
                "end",
                |f| {
                    f.if_(gt(v("end"), v("e")), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
                    // Returned as (start, end): the caller's position is end.
                    f.ret_ok(tup(vec![v("p"), v("end")]));
                },
                "_",
                |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)),
            );
        },
    );
    // A list/set/map count (at most one million).
    m.func(
        "rt_rd_count",
        &[("b", t::vec(t::u8())), ("p", t::u64()), ("e", t::u64())],
        rdu.clone(),
        |f| {
            f.var("p", v("p"));
            rd(f, "count", "p", rcall("rt_rd_uvar", vec![u32c(64)]));
            f.if_(gt(v("count"), u64c(MAX_COLLECTION)), |f| {
                f.ret_err(code(SCB_RESOURCE_LIMIT));
            });
            f.ret_ok(tup(vec![v("count"), v("p")]));
        },
    );
    // A record field count (at most 65,535).
    m.func(
        "rt_rd_fields",
        &[("b", t::vec(t::u8())), ("p", t::u64()), ("e", t::u64())],
        rdu,
        |f| {
            f.var("p", v("p"));
            rd(f, "count", "p", rcall("rt_rd_uvar", vec![u32c(64)]));
            f.if_(gt(v("count"), u64c(MAX_RECORD_FIELDS)), |f| {
                f.ret_err(code(SCB_RESOURCE_LIMIT));
            });
            f.ret_ok(tup(vec![v("count"), v("p")]));
        },
    );

    // ----- writers over Vector<UInt(8)> -----
    m.func(
        "wr_u8",
        &[("o", t::vec(t::u8())), ("x", t::u64())],
        t::vec(t::u8()),
        |f| f.ret(push(v("o"), call("rt_byte", vec![v("x")]))),
    );
    // Big-endian fixed width: `n` bytes of `x`.
    m.func(
        "wr_be",
        &[("o", t::vec(t::u8())), ("x", t::u64()), ("n", t::u64())],
        t::vec(t::u8()),
        |f| {
            f.var("o", v("o"));
            f.var("k", v("n"));
            f.while_(gt(v("k"), u64c(0)), |f| {
                f.set("k", sub(v("k"), u64c(1)));
                f.let_(
                    "shift",
                    call("rt_shift8", vec![v("k")]),
                );
                f.set(
                    "o",
                    push(
                        v("o"),
                        call("rt_byte", vec![rem(shr(v("x"), v("shift")), u64c(256))]),
                    ),
                );
            });
            f.ret(v("o"));
        },
    );
    // 8 * k as a shift amount (k in 0..=7).
    m.func("rt_shift8", &[("k", t::u64())], t::u32(), |f| {
        f.ret(mget(
            sorted_map(
                t::u64(),
                t::u32(),
                (0..8_u128)
                    .map(|k| (uint(k, t::u64()), uint(k * 8, t::u32())))
                    .collect(),
            ),
            v("k"),
        ));
    });
    m.func(
        "wr_u32",
        &[("o", t::vec(t::u8())), ("x", t::u64())],
        t::vec(t::u8()),
        |f| f.ret(call("wr_be", vec![v("o"), v("x"), u64c(4)])),
    );
    m.func(
        "wr_u64",
        &[("o", t::vec(t::u8())), ("x", t::u64())],
        t::vec(t::u8()),
        |f| f.ret(call("wr_be", vec![v("o"), v("x"), u64c(8)])),
    );
    // Canonical LE7 unsigned varint.
    m.func(
        "wr_uvar",
        &[("o", t::vec(t::u8())), ("x", t::u64())],
        t::vec(t::u8()),
        |f| {
            f.var("o", v("o"));
            f.var("x", v("x"));
            f.loop_(|f| {
                f.if_(lt(v("x"), u64c(128)), |f| {
                    f.set("o", push(v("o"), call("rt_byte", vec![v("x")])));
                    f.break_();
                });
                f.set(
                    "o",
                    push(
                        v("o"),
                        call("rt_byte", vec![add(rem(v("x"), u64c(128)), u64c(128))]),
                    ),
                );
                f.set("x", div(v("x"), u64c(128)));
            });
            f.ret(v("o"));
        },
    );
    // Appends all of `bytes` (converted through the bridge).
    m.func(
        "wr_bytes",
        &[("o", t::vec(t::u8())), ("x", t::bytes())],
        t::vec(t::u8()),
        |f| {
            f.var("o", v("o"));
            f.let_("src", b2v(v("x")));
            f.for_u64("i", u64c(0), vlen(v("src")), |f| {
                f.set("o", push(v("o"), vget(v("src"), v("i"))));
            });
            f.ret(v("o"));
        },
    );
    // `Bytes` length.
    m.func("rt_blen", &[("x", t::bytes())], t::u64(), |f| {
        f.ret(blen(v("x")));
    });
    // Big-endian u32/u64 as standalone `Bytes`.
    m.func("be32", &[("x", t::u64())], t::bytes(), |f| {
        f.ret(v2b(call("wr_u32", vec![vnew(t::u8(), vec![]), v("x")])));
    });
    m.func("be64", &[("x", t::u64())], t::bytes(), |f| {
        f.ret(v2b(call("wr_u64", vec![vnew(t::u8(), vec![]), v("x")])));
    });
    m.func("uvarb", &[("x", t::u64())], t::bytes(), |f| {
        f.ret(v2b(call("wr_uvar", vec![vnew(t::u8(), vec![]), v("x")])));
    });
}
