//! Schema-directed strict validation of nested SCB1 values (`scb_*`):
//! `TypeExpr`, `ConstValue`, and their closed sub-schemas, exactly as the
//! native `sley-mutate` codec accepts them.
//!
//! Recursion is outside the bootstrap profile, so one walker drives an
//! explicit work stack of `(kind, start, end, depth)` regions. Every region
//! must decode as exactly one value of its kind (trailing bytes refuse), and
//! depth follows the native accounting: containers refuse at depth 64,
//! leaves above 64. Children are pushed in reverse so regions are visited
//! in byte order. One deliberate precedence difference: a list or record
//! reads all of its own framing before visiting its children, so when a
//! later sibling's framing and an earlier child's content are both
//! malformed, the framing error is the one reported (both refuse).

use super::codes::{
    SCB_BOOL_INVALID, SCB_FIELD_DUPLICATE, SCB_FIELD_MISSING, SCB_FIELD_ORDER, SCB_FIELD_UNKNOWN,
    SCB_FLOAT_NON_CANONICAL, SCB_INTEGER_OVERFLOW, SCB_LENGTH_OVERFLOW, SCB_MAP_DUPLICATE,
    SCB_MAP_ORDER, SCB_RESOURCE_LIMIT, SCB_TRAILING_BYTES, SCB_UNION_INVALID, SCB_UTF8_INVALID,
    SCB_VARINT_NON_MINIMAL,
};
use super::rt::{MAX_BYTE_PAYLOAD, code, r, rcall, rd, rd_ty};
use crate::seed::*;

/// `TypeExpr`.
pub const K_TYPE: u64 = 1;
/// `List<TypeExpr>`.
pub const K_TYPELIST: u64 = 2;
/// `NamedType` record.
pub const K_NAMED: u64 = 3;
/// `MapType` / `ResultType` record of two `TypeExpr`.
pub const K_PAIRTYPE: u64 = 4;
/// `FunctionType` record.
pub const K_FUNTYPE: u64 = 5;
/// `FixedBytes<32>`.
pub const K_ID: u64 = 6;
/// `UInt16`.
pub const K_U16: u64 = 7;
/// `UInt32`.
pub const K_U32: u64 = 8;
/// `BuiltinFailureKind` (`UInt16`, tags 1..=5).
pub const K_BFKIND: u64 = 9;
/// `Set<EntityId>`.
pub const K_IDSET: u64 = 10;
/// `ConstValue` record.
pub const K_CONST: u64 = 11;
/// `ConstData` union.
pub const K_DATA: u64 = 12;
/// `Bool`.
pub const K_BOOL: u64 = 13;
/// `SInt` (128-bit `ZigZag` varint).
pub const K_SINT: u64 = 14;
/// `UInt` (128-bit varint).
pub const K_UINT: u64 = 15;
/// `Bytes`.
pub const K_BYTES: u64 = 16;
/// `Text`.
pub const K_TEXT: u64 = 17;
/// `List<ConstValue>`.
pub const K_CONSTLIST: u64 = 18;
/// `RecordConst`.
pub const K_RECCONST: u64 = 19;
/// `List<FieldConst>`.
pub const K_FIELDLIST: u64 = 20;
/// `FieldConst`.
pub const K_FIELDCONST: u64 = 21;
/// `VariantConst`.
pub const K_VARCONST: u64 = 22;
/// `Option<ConstValue>`.
pub const K_OPTCONST: u64 = 23;
/// `List<MapEntryConst>` (strictly increasing canonical keys).
pub const K_MAPENTRIES: u64 = 24;
/// `MapEntryConst`.
pub const K_MAPENTRY: u64 = 25;
/// `ResultConst`.
pub const K_RESCONST: u64 = 26;
/// `FunctionRefValue`.
pub const K_FUNREF: u64 = 27;
/// `BuiltinFailureValue`.
pub const K_BFVALUE: u64 = 28;
/// `F32` bits.
pub const K_F32: u64 = 29;
/// `F64` bits.
pub const K_F64: u64 = 30;

fn item_ty() -> Ty {
    t::tuple(vec![t::u64(), t::u64(), t::u64(), t::u64()])
}

fn pair_ty() -> Ty {
    t::tuple(vec![t::u64(), t::u64()])
}

/// Pushes `(kind, start, end, depth)` onto the walker stack.
fn push_item(f: &mut FnB<'_>, kind: E, start: E, end: E, depth: E) {
    f.set(
        "stack",
        call(
            "scb_stack_push",
            vec![
                v("stack"),
                v("top"),
                tup(vec![kind, start, end, depth]),
            ],
        ),
    );
    f.set("top", add(v("top"), u64c(1)));
}

/// Pushes one child per field region, in reverse order, with kinds `kinds`.
fn push_fields(f: &mut FnB<'_>, kinds: &[u64]) {
    for (index, kind) in kinds.iter().enumerate().rev() {
        let index = u64::try_from(index).expect("small");
        push_item(
            f,
            u64c(*kind),
            tget(vget(v("fields"), u64c(index)), 0),
            tget(vget(v("fields"), u64c(index)), 1),
            add(v("d"), u64c(1)),
        );
    }
}

/// Reads an exact all-required record of `n` contiguous tags into `fields`.
fn read_record(f: &mut FnB<'_>, n: u64) {
    rd(f, "fields", "p", rcall("scb_fields", vec![u64c(n)]));
}

fn finished(f: &mut FnB<'_>) {
    f.if_(ne(v("p"), v("e")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
}

fn container_depth(f: &mut FnB<'_>) {
    f.if_(ge(v("d"), u64c(64)), |f| f.ret_err(code(SCB_RESOURCE_LIMIT)));
}

fn leaf_depth(f: &mut FnB<'_>) {
    f.if_(gt(v("d"), u64c(64)), |f| f.ret_err(code(SCB_RESOURCE_LIMIT)));
}

/// Defines the `scb_*` functions.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    let buf = || ("b", t::vec(t::u8()));
    // Grows-or-overwrites push for the walker stack.
    m.func(
        "scb_stack_push",
        &[
            ("stack", t::vec(item_ty())),
            ("top", t::u64()),
            ("item", item_ty()),
        ],
        t::vec(item_ty()),
        |f| {
            f.if_(lt(v("top"), vlen(v("stack"))), |f| {
                f.ret(vset(v("stack"), v("top"), v("item")));
            });
            f.ret(push(v("stack"), v("item")));
        },
    );
    // An all-required record with tags 1..=n: field payload regions.
    m.func(
        "scb_fields",
        &[buf(), ("p", t::u64()), ("e", t::u64()), ("n", t::u64())],
        rd_ty(t::vec(pair_ty())),
        |f| {
            f.var("p", v("p"));
            rd(f, "count", "p", rcall("rt_rd_fields", vec![]));
            f.if_(lt(v("count"), v("n")), |f| f.ret_err(code(SCB_FIELD_MISSING)));
            f.if_(gt(v("count"), v("n")), |f| f.ret_err(code(SCB_FIELD_UNKNOWN)));
            f.var("out", vnew(pair_ty(), vec![]));
            f.var("prev", u64c(0));
            f.for_u64("i", u64c(1), add(v("n"), u64c(1)), |f| {
                rd(f, "tag", "p", rcall("rt_rd_uvar", vec![u32c(32)]));
                f.if_(gt(v("prev"), u64c(0)), |f| {
                    f.if_(eq(v("tag"), v("prev")), |f| f.ret_err(code(SCB_FIELD_DUPLICATE)));
                    f.if_(lt(v("tag"), v("prev")), |f| f.ret_err(code(SCB_FIELD_ORDER)));
                });
                f.set("prev", v("tag"));
                f.if_(ne(v("tag"), v("i")), |f| {
                    f.if_(
                        and(ge(v("tag"), u64c(1)), le(v("tag"), v("n"))),
                        |f| f.ret_err(code(SCB_FIELD_ORDER)),
                    );
                    f.ret_err(code(SCB_FIELD_UNKNOWN));
                });
                rd(f, "start", "p", rcall("rt_rd_payload", vec![]));
                f.set("out", push(v("out"), tup(vec![v("start"), v("p")])));
            });
            f.ret_ok(tup(vec![v("out"), v("p")]));
        },
    );
    // Canonical LE7 varint at 128-bit width, as the native reader.
    m.func(
        "scb_rd_uvar128",
        &[buf(), ("p", t::u64()), ("e", t::u64())],
        rd_ty(t::u128()),
        |f| {
            f.var("value", u128c(0));
            f.var("shift", u32c(0));
            f.var("count", u64c(0));
            f.var("p", v("p"));
            f.loop_(|f| {
                f.if_(ge(v("p"), v("e")), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
                f.let_("byte", call("rt_b", vec![vget(v("b"), v("p"))]));
                f.set("p", add(v("p"), u64c(1)));
                f.set("count", add(v("count"), u64c(1)));
                f.let_("payload", call("scb_u64_u128", vec![rem(v("byte"), u64c(128))]));
                f.if_(
                    and(ge(v("shift"), u32c(128)), ne(v("payload"), u128c(0))),
                    |f| f.ret_err(code(SCB_INTEGER_OVERFLOW)),
                );
                f.if_(lt(v("shift"), u32c(128)), |f| {
                    f.if_(
                        and(eq(v("shift"), u32c(126)), gt(v("payload"), u128c(3))),
                        |f| f.ret_err(code(SCB_INTEGER_OVERFLOW)),
                    );
                    f.set("value", add(v("value"), shl(v("payload"), v("shift"))));
                });
                f.if_(lt(v("byte"), u64c(128)), |f| {
                    f.if_(
                        and(gt(v("count"), u64c(1)), eq(v("payload"), u128c(0))),
                        |f| f.ret_err(code(SCB_VARINT_NON_MINIMAL)),
                    );
                    f.ret_ok(tup(vec![v("value"), v("p")]));
                });
                f.set("shift", add(v("shift"), u32c(7)));
                f.if_(ge(v("shift"), u32c(135)), |f| {
                    f.ret_err(code(SCB_INTEGER_OVERFLOW));
                });
            });
        },
    );
    // u64 (< 128) as u128 through a 128-entry table.
    m.func("scb_u64_u128", &[("x", t::u64())], t::u128(), |f| {
        f.ret(mget(table_u64_u128(), v("x")));
    });
    // UTF-8 well-formedness (shortest form, no surrogates, <= U+10FFFF).
    m.func(
        "scb_utf8_ok",
        &[buf(), ("p", t::u64()), ("e", t::u64())],
        t::bool(),
        |f| {
            f.var("p", v("p"));
            f.while_(lt(v("p"), v("e")), |f| {
                f.let_("c", call("rt_b", vec![vget(v("b"), v("p"))]));
                f.if_(lt(v("c"), u64c(0x80)), |f| {
                    f.set("p", add(v("p"), u64c(1)));
                    f.continue_();
                });
                // Lead byte classes: C2..DF (2), E0..EF (3), F0..F4 (4).
                f.var("need", u64c(0));
                f.var("lo", u64c(0x80));
                f.var("hi", u64c(0xbf));
                f.if_else(
                    and(ge(v("c"), u64c(0xc2)), le(v("c"), u64c(0xdf))),
                    |f| f.set("need", u64c(1)),
                    |f| {
                        f.if_else(
                            and(ge(v("c"), u64c(0xe0)), le(v("c"), u64c(0xef))),
                            |f| {
                                f.set("need", u64c(2));
                                f.if_(eq(v("c"), u64c(0xe0)), |f| f.set("lo", u64c(0xa0)));
                                f.if_(eq(v("c"), u64c(0xed)), |f| f.set("hi", u64c(0x9f)));
                            },
                            |f| {
                                f.if_(
                                    and(ge(v("c"), u64c(0xf0)), le(v("c"), u64c(0xf4))),
                                    |f| {
                                        f.set("need", u64c(3));
                                        f.if_(eq(v("c"), u64c(0xf0)), |f| f.set("lo", u64c(0x90)));
                                        f.if_(eq(v("c"), u64c(0xf4)), |f| f.set("hi", u64c(0x8f)));
                                    },
                                );
                            },
                        );
                    },
                );
                f.if_(eq(v("need"), u64c(0)), |f| f.ret(boolc(false)));
                f.if_(gt(add(v("p"), v("need")), sub(v("e"), u64c(1))), |f| {
                    f.ret(boolc(false));
                });
                f.var("k", u64c(1));
                f.while_(le(v("k"), v("need")), |f| {
                    f.let_("x", call("rt_b", vec![vget(v("b"), add(v("p"), v("k")))]));
                    f.if_(eq(v("k"), u64c(1)), |f| {
                        f.if_(or(lt(v("x"), v("lo")), gt(v("x"), v("hi"))), |f| {
                            f.ret(boolc(false));
                        });
                    });
                    f.if_(gt(v("k"), u64c(1)), |f| {
                        f.if_(or(lt(v("x"), u64c(0x80)), gt(v("x"), u64c(0xbf))), |f| {
                            f.ret(boolc(false));
                        });
                    });
                    f.set("k", add(v("k"), u64c(1)));
                });
                f.set("p", add(add(v("p"), v("need")), u64c(1)));
            });
            f.ret(boolc(true));
        },
    );
    // Canonical float bits: `width` bytes big-endian at [p, p+width).
    m.func(
        "scb_float_ok",
        &[buf(), ("p", t::u64()), ("width", t::u64())],
        t::bool(),
        |f| {
            // Sign/exponent/mantissa tests on bytes: F32 exponent occupies
            // b0 & 0x7f and the top bit of b1; F64 b0 & 0x7f and b1 >> 4.
            f.let_("b0", call("rt_b", vec![vget(v("b"), v("p"))]));
            f.let_("b1", call("rt_b", vec![vget(v("b"), add(v("p"), u64c(1)))]));
            f.var("rest_zero", boolc(true));
            f.for_u64("i", u64c(2), v("width"), |f| {
                f.if_(ne(call("rt_b", vec![vget(v("b"), add(v("p"), v("i")))]), u64c(0)), |f| {
                    f.set("rest_zero", boolc(false));
                });
            });
            f.let_("f32", eq(v("width"), u64c(4)));
            // Negative zero: 0x80 then all zero.
            f.let_("lowmask", select(v("f32"), u64c(0x80), u64c(0xf0)));
            f.let_(
                "negzero",
                and(and(eq(v("b0"), u64c(0x80)), eq(v("b1"), u64c(0))), v("rest_zero")),
            );
            f.let_(
                "exp_ones",
                and(
                    eq(rem(v("b0"), u64c(128)), u64c(0x7f)),
                    ge(v("b1"), v("lowmask")),
                ),
            );
            // Mantissa bits within b1: below the exponent bits.
            f.let_("b1_mant", rem(v("b1"), select(v("f32"), u64c(0x80), u64c(0x10))));
            f.let_("mant_zero", and(eq(v("b1_mant"), u64c(0)), v("rest_zero")));
            // The single canonical NaN: 7fc0... (F32) or 7ff8... (F64).
            f.let_(
                "canon_nan",
                and(
                    and(eq(v("b0"), u64c(0x7f)), v("rest_zero")),
                    eq(v("b1"), select(v("f32"), u64c(0xc0), u64c(0xf8))),
                ),
            );
            f.let_("nan", and(v("exp_ones"), not(v("mant_zero"))));
            f.ret(not(or(v("negzero"), and(v("nan"), not(v("canon_nan"))))));
        },
    );
    // The walker.
    m.func(
        "scb_walk",
        &[
            buf(),
            ("bs", t::bytes()),
            ("s", t::u64()),
            ("e", t::u64()),
            ("kind", t::u64()),
            ("depth", t::u64()),
        ],
        r(t::unit()),
        |f| {
            f.var("stack", vnew(item_ty(), vec![]));
            f.var("top", u64c(0));
            push_item(f, v("kind"), v("s"), v("e"), v("depth"));
            f.while_(gt(v("top"), u64c(0)), |f| {
                f.set("top", sub(v("top"), u64c(1)));
                f.let_("item", vget(v("stack"), v("top")));
                f.let_("k", tget(v("item"), 0));
                f.var("p", tget(v("item"), 1));
                f.let_("e", tget(v("item"), 2));
                f.let_("d", tget(v("item"), 3));
                f.if_else(
                    lt(v("k"), u64c(K_CONST)),
                    walk_type_kinds,
                    walk_const_kinds,
                );
            });
            f.ret_ok(unitc());
        },
    );
}

fn table_u64_u128() -> E {
    use sley_ssmc::{ConstData, ConstValue, MapEntryConst};
    let mut entries: Vec<(ConstValue, ConstValue)> = (0..128_u128)
        .map(|i| {
            (
                ConstValue {
                    value_type: t::u64(),
                    data: ConstData::UInt(i),
                },
                ConstValue {
                    value_type: t::u128(),
                    data: ConstData::UInt(i),
                },
            )
        })
        .collect();
    entries.sort_by_key(|(k, _)| sley_mutate::encode_const_value(k).expect("canonical key"));
    E::Lit(ConstValue {
        value_type: t::map(t::u64(), t::u128()),
        data: ConstData::Map(
            entries
                .into_iter()
                .map(|(key, value)| MapEntryConst { key, value })
                .collect(),
        ),
    })
}

/// Reads one union header at `p`: binds `tag`, `ps`, `pe`; `p` moves to `pe`.
fn read_union(f: &mut FnB<'_>) {
    rd(f, "tag", "p", rcall("rt_rd_uvar", vec![u32c(32)]));
    rd(f, "ps", "p", rcall("rt_rd_payload", vec![]));
    f.let_("pe", v("p"));
}

#[allow(clippy::too_many_lines)]
fn walk_type_kinds(f: &mut FnB<'_>) {
    // TypeExpr union.
    f.if_(eq(v("k"), u64c(K_TYPE)), |f| {
        container_depth(f);
        read_union(f);
        finished(f);
        f.let_("empty", eq(v("ps"), v("pe")));
        f.let_("child", add(v("d"), u64c(1)));
        f.if_(
            or(
                or(eq(v("tag"), u64c(1)), eq(v("tag"), u64c(2))),
                or(
                    or(eq(v("tag"), u64c(5)), eq(v("tag"), u64c(6))),
                    or(eq(v("tag"), u64c(7)), eq(v("tag"), u64c(8))),
                ),
            ),
            |f| {
                f.if_(not(v("empty")), |f| f.ret_err(code(SCB_UNION_INVALID)));
                f.continue_();
            },
        );
        for (tag, kind) in [
            (3, K_U16),
            (4, K_U16),
            (9, K_TYPELIST),
            (10, K_NAMED),
            (11, K_TYPE),
            (12, K_PAIRTYPE),
            (13, K_TYPE),
            (14, K_PAIRTYPE),
            (15, K_FUNTYPE),
            (16, K_ID),
            (17, K_ID),
            (18, K_TYPE),
            (19, K_U32),
            (20, K_BFKIND),
        ] {
            f.if_(eq(v("tag"), u64c(tag)), move |f| {
                push_item(f, u64c(kind), v("ps"), v("pe"), v("child"));
                f.continue_();
            });
        }
        f.ret_err(code(SCB_UNION_INVALID));
    });
    // List<TypeExpr>.
    f.if_(eq(v("k"), u64c(K_TYPELIST)), |f| {
        container_depth(f);
        list_children(f, K_TYPE);
        f.continue_();
    });
    // Records.
    for (kind, fields) in [
        (K_NAMED, vec![K_ID, K_TYPELIST]),
        (K_PAIRTYPE, vec![K_TYPE, K_TYPE]),
        (K_FUNTYPE, vec![K_TYPELIST, K_TYPE, K_IDSET]),
    ] {
        f.if_(eq(v("k"), u64c(kind)), move |f| {
            container_depth(f);
            read_record(f, u64::try_from(fields.len()).expect("small"));
            finished(f);
            push_fields(f, &fields);
            f.continue_();
        });
    }
    // Fixed 32 bytes.
    f.if_(eq(v("k"), u64c(K_ID)), |f| {
        leaf_depth(f);
        f.if_(lt(sub(v("e"), v("p")), u64c(32)), |f| {
            f.ret_err(code(SCB_LENGTH_OVERFLOW));
        });
        f.if_(gt(sub(v("e"), v("p")), u64c(32)), |f| {
            f.ret_err(code(SCB_TRAILING_BYTES));
        });
        f.continue_();
    });
    // Integers.
    for (kind, width) in [(K_U16, 16_u32), (K_U32, 32), (K_BFKIND, 16)] {
        f.if_(eq(v("k"), u64c(kind)), move |f| {
            leaf_depth(f);
            rd(f, "x", "p", rcall("rt_rd_uvar", vec![u32c(width)]));
            if kind == K_BFKIND {
                f.if_(or(lt(v("x"), u64c(1)), gt(v("x"), u64c(5))), |f| {
                    f.ret_err(code(SCB_UNION_INVALID));
                });
            }
            finished(f);
            f.continue_();
        });
    }
    // Set<EntityId>: exact 32-byte elements, strictly increasing.
    f.if_(eq(v("k"), u64c(K_IDSET)), |f| {
        container_depth(f);
        rd(f, "count", "p", rcall("rt_rd_count", vec![]));
        f.var("prev", bytesc(&[]));
        f.for_u64("i", u64c(0), v("count"), |f| {
            rd(f, "es", "p", rcall("rt_rd_payload", vec![]));
            f.if_(gt(add(v("d"), u64c(1)), u64c(64)), |f| {
                f.ret_err(code(SCB_RESOURCE_LIMIT));
            });
            f.if_(lt(sub(v("p"), v("es")), u64c(32)), |f| {
                f.ret_err(code(SCB_LENGTH_OVERFLOW));
            });
            f.if_(gt(sub(v("p"), v("es")), u64c(32)), |f| {
                f.ret_err(code(SCB_TRAILING_BYTES));
            });
            f.let_("id", slice(v("bs"), v("es"), u64c(32)));
            f.if_(gt(v("i"), u64c(0)), |f| {
                f.if_(eq(v("prev"), v("id")), |f| f.ret_err(code(SCB_MAP_DUPLICATE)));
                f.if_(gt(v("prev"), v("id")), |f| f.ret_err(code(SCB_MAP_ORDER)));
            });
            f.set("prev", v("id"));
        });
        finished(f);
        f.continue_();
    });
    f.trap();
}

/// A list whose elements are regions of `kind` (count, then sized payloads).
fn list_children(f: &mut FnB<'_>, kind: u64) {
    rd(f, "count", "p", rcall("rt_rd_count", vec![]));
    f.var("regions", vnew(pair_ty(), vec![]));
    f.for_u64("i", u64c(0), v("count"), |f| {
        rd(f, "es", "p", rcall("rt_rd_payload", vec![]));
        f.set("regions", push(v("regions"), tup(vec![v("es"), v("p")])));
    });
    finished(f);
    f.var("j", v("count"));
    f.while_(gt(v("j"), u64c(0)), move |f| {
        f.set("j", sub(v("j"), u64c(1)));
        f.let_("region", vget(v("regions"), v("j")));
        push_item(
            f,
            u64c(kind),
            tget(v("region"), 0),
            tget(v("region"), 1),
            add(v("d"), u64c(1)),
        );
    });
}

#[allow(clippy::too_many_lines)]
fn walk_const_kinds(f: &mut FnB<'_>) {
    for (kind, fields) in [
        (K_CONST, vec![K_TYPE, K_DATA]),
        (K_RECCONST, vec![K_ID, K_FIELDLIST]),
        (K_FIELDCONST, vec![K_ID, K_CONST]),
        (K_VARCONST, vec![K_ID, K_ID, K_OPTCONST]),
        (K_MAPENTRY, vec![K_CONST, K_CONST]),
        (K_FUNREF, vec![K_ID, K_TYPELIST]),
        (K_BFVALUE, vec![K_BFKIND, K_U16]),
    ] {
        f.if_(eq(v("k"), u64c(kind)), move |f| {
            container_depth(f);
            read_record(f, u64::try_from(fields.len()).expect("small"));
            finished(f);
            push_fields(f, &fields);
            f.continue_();
        });
    }
    // ConstData union.
    f.if_(eq(v("k"), u64c(K_DATA)), |f| {
        container_depth(f);
        read_union(f);
        finished(f);
        f.let_("child", add(v("d"), u64c(1)));
        f.if_(eq(v("tag"), u64c(1)), |f| {
            f.if_(ne(v("ps"), v("pe")), |f| f.ret_err(code(SCB_UNION_INVALID)));
            f.continue_();
        });
        for (tag, kind) in [
            (2, K_BOOL),
            (3, K_SINT),
            (4, K_UINT),
            (5, K_F32),
            (6, K_F64),
            (7, K_BYTES),
            (8, K_TEXT),
            (9, K_CONSTLIST),
            (10, K_RECCONST),
            (11, K_VARCONST),
            (12, K_MAPENTRIES),
            (13, K_OPTCONST),
            (14, K_RESCONST),
            (15, K_FUNREF),
            (16, K_BFVALUE),
        ] {
            f.if_(eq(v("tag"), u64c(tag)), move |f| {
                push_item(f, u64c(kind), v("ps"), v("pe"), v("child"));
                f.continue_();
            });
        }
        f.ret_err(code(SCB_UNION_INVALID));
    });
    // Bool.
    f.if_(eq(v("k"), u64c(K_BOOL)), |f| {
        leaf_depth(f);
        rd(f, "x", "p", rcall("rt_rd_u8", vec![]));
        f.if_(gt(v("x"), u64c(1)), |f| f.ret_err(code(SCB_BOOL_INVALID)));
        finished(f);
        f.continue_();
    });
    // 128-bit integers.
    f.if_(or(eq(v("k"), u64c(K_SINT)), eq(v("k"), u64c(K_UINT))), |f| {
        leaf_depth(f);
        rd(f, "x", "p", rcall("scb_rd_uvar128", vec![]));
        finished(f);
        f.continue_();
    });
    // Floats (validated directly, like the native nested cursor).
    for (kind, width) in [(K_F32, 4_u64), (K_F64, 8)] {
        f.if_(eq(v("k"), u64c(kind)), move |f| {
            f.if_(lt(sub(v("e"), v("p")), u64c(width)), |f| {
                f.ret_err(code(SCB_LENGTH_OVERFLOW));
            });
            f.if_(
                not(call("scb_float_ok", vec![v("b"), v("p"), u64c(width)])),
                |f| f.ret_err(code(SCB_FLOAT_NON_CANONICAL)),
            );
            f.if_(gt(sub(v("e"), v("p")), u64c(width)), |f| {
                f.ret_err(code(SCB_TRAILING_BYTES));
            });
            f.continue_();
        });
    }
    // Bytes and Text.
    f.if_(or(eq(v("k"), u64c(K_BYTES)), eq(v("k"), u64c(K_TEXT))), |f| {
        leaf_depth(f);
        rd(f, "len", "p", rcall("rt_rd_len", vec![u64c(MAX_BYTE_PAYLOAD)]));
        f.let_("start", v("p"));
        f.match_res(
            try_add(v("p"), v("len")),
            "end",
            |f| {
                f.if_(gt(v("end"), v("e")), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
                f.set("p", v("end"));
            },
            "_",
            |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)),
        );
        f.if_(eq(v("k"), u64c(K_TEXT)), |f| {
            f.if_(
                not(call("scb_utf8_ok", vec![v("b"), v("start"), v("p")])),
                |f| f.ret_err(code(SCB_UTF8_INVALID)),
            );
        });
        finished(f);
        f.continue_();
    });
    // Lists.
    f.if_(eq(v("k"), u64c(K_CONSTLIST)), |f| {
        container_depth(f);
        list_children(f, K_CONST);
        f.continue_();
    });
    f.if_(eq(v("k"), u64c(K_FIELDLIST)), |f| {
        container_depth(f);
        list_children(f, K_FIELDCONST);
        f.continue_();
    });
    // Option<ConstValue>.
    f.if_(eq(v("k"), u64c(K_OPTCONST)), |f| {
        container_depth(f);
        read_union(f);
        finished(f);
        f.if_(eq(v("tag"), u64c(0)), |f| {
            f.if_(ne(v("ps"), v("pe")), |f| f.ret_err(code(SCB_UNION_INVALID)));
            f.continue_();
        });
        f.if_(eq(v("tag"), u64c(1)), |f| {
            push_item(f, u64c(K_CONST), v("ps"), v("pe"), add(v("d"), u64c(1)));
            f.continue_();
        });
        f.ret_err(code(SCB_UNION_INVALID));
    });
    // ResultConst.
    f.if_(eq(v("k"), u64c(K_RESCONST)), |f| {
        container_depth(f);
        read_union(f);
        finished(f);
        f.if_(or(eq(v("tag"), u64c(1)), eq(v("tag"), u64c(2))), |f| {
            push_item(f, u64c(K_CONST), v("ps"), v("pe"), add(v("d"), u64c(1)));
            f.continue_();
        });
        f.ret_err(code(SCB_UNION_INVALID));
    });
    // Map entries: strictly increasing canonical key bytes.
    f.if_(eq(v("k"), u64c(K_MAPENTRIES)), |f| {
        container_depth(f);
        rd(f, "count", "p", rcall("rt_rd_count", vec![]));
        f.var("regions", vnew(pair_ty(), vec![]));
        f.var("prev", bytesc(&[]));
        f.for_u64("i", u64c(0), v("count"), |f| {
            rd(f, "es", "p", rcall("rt_rd_payload", vec![]));
            f.set("regions", push(v("regions"), tup(vec![v("es"), v("p")])));
            // The key region: the entry's field-1 payload, when the entry
            // frames it; a malformed entry refuses through its own walk.
            f.match_res(
                call("scb_entry_key", vec![v("b"), v("es"), v("p")]),
                "key",
                |f| {
                    f.let_(
                        "kb",
                        slice(
                            v("bs"),
                            tget(v("key"), 0),
                            sub(tget(v("key"), 1), tget(v("key"), 0)),
                        ),
                    );
                    f.if_(gt(v("i"), u64c(0)), |f| {
                        f.if_(eq(v("prev"), v("kb")), |f| f.ret_err(code(SCB_MAP_DUPLICATE)));
                        f.if_(gt(v("prev"), v("kb")), |f| f.ret_err(code(SCB_MAP_ORDER)));
                    });
                    f.set("prev", v("kb"));
                },
                "_",
                |_| {},
            );
        });
        finished(f);
        f.var("j", v("count"));
        f.while_(gt(v("j"), u64c(0)), |f| {
            f.set("j", sub(v("j"), u64c(1)));
            f.let_("region", vget(v("regions"), v("j")));
            push_item(
                f,
                u64c(K_MAPENTRY),
                tget(v("region"), 0),
                tget(v("region"), 1),
                add(v("d"), u64c(1)),
            );
        });
        f.continue_();
    });
    f.trap();
}

/// Defines the map-entry key locator used by the walker.
pub fn define_entry_key(m: &mut Module) {
    m.func(
        "scb_entry_key",
        &[("b", t::vec(t::u8())), ("p", t::u64()), ("e", t::u64())],
        r(pair_ty()),
        |f| {
            f.var("p", v("p"));
            rd(f, "count", "p", rcall("rt_rd_fields", vec![]));
            rd(f, "tag", "p", rcall("rt_rd_uvar", vec![u32c(32)]));
            f.if_(ne(v("tag"), u64c(1)), |f| f.ret_err(code(SCB_FIELD_UNKNOWN)));
            rd(f, "start", "p", rcall("rt_rd_payload", vec![]));
            f.ret_ok(tup(vec![v("start"), v("p")]));
        },
    );
}
