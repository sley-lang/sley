//! `jg_*`: extended operation judgment and the bootstrap-profile gate.
//!
//! - `jg_function(inv, fid)`: exactly what native `EXTENDED_V1` lowering
//!   judges for one function after graph validation (`sley-vm`
//!   `lower::judge_function_operations`): the resource preflight bounds,
//!   `check_result_type` (no cell, handle, or token escapes), then every
//!   operation of every block in `Function.blocks` order through
//!   `judge_extended_operation` (the derived result type must equal the
//!   declared one exactly), then `require_canonical_referenced_constants`.
//! - `jg_gate(inv, entry, image)`: exactly `judge_bootstrap_profile` under
//!   `BootstrapProfileVersion::V3` (closure walk, inventory closure, import
//!   registry, constant membership, per-function membership and counts),
//!   returning the `Gate` claims with empty fingerprints.
//!
//! Types are canonical SCB1 `TypeExpr` bytes. Every type here already
//! passed `ck_*` (closed, well formed, epoch-1 widths), so tags are single
//! bytes and the walkers parse framing without re-validating it. Recursion
//! is excluded: every walk is an explicit stack.
//!
//! Per-call memoization (the driver calls `jg_function` once per function):
//! register types are collected once with their tag masks; traits,
//! callee signatures, and bridge resolutions are memoized by key within one
//! call.

use sley_ssmc::{
    BuiltinFailureKind, ConstData, ConstValue, IntegerWidth, MapEntryConst, TypeExpr,
};

use super::inv::{INV, KIND_FUNCTION, KIND_IMPORT};
use super::model::*;
use super::rt::{code, r};
use super::types::ty_lit;
use crate::seed::*;

/// `VM_LOWER_PROFILE_UNSUPPORTED`.
pub const VM_LOWER_PROFILE_UNSUPPORTED: u32 = 26_000;
/// `VM_LOWER_OPCODE_UNSUPPORTED`.
pub const VM_LOWER_OPCODE_UNSUPPORTED: u32 = 26_001;
/// `VM_LOWER_SIGNATURE_MISMATCH`.
pub const VM_LOWER_SIGNATURE_MISMATCH: u32 = 26_002;
/// `VM_LOWER_IMMEDIATE_MISMATCH`.
pub const VM_LOWER_IMMEDIATE_MISMATCH: u32 = 26_003;
/// `VM_LOWER_LOCAL_REFERENCE_INVALID`.
pub const VM_LOWER_LOCAL_REFERENCE_INVALID: u32 = 26_004;
/// `VM_LOWER_RESOURCE_LIMIT`.
pub const VM_LOWER_RESOURCE_LIMIT: u32 = 26_006;

const OU: u64 = VM_LOWER_OPCODE_UNSUPPORTED as u64;
const SM: u64 = VM_LOWER_SIGNATURE_MISMATCH as u64;
const IM: u64 = VM_LOWER_IMMEDIATE_MISMATCH as u64;

/// Frozen bootstrap opcodes (`PERMITTED_BOOTSTRAP_OPCODES`).
const BOOTSTRAP_OPCODES: &[u64] = &[
    1, 16, 17, 18, 19, 20, 21, 32, 33, 34, 35, 36, 37, 38, 39, 40, 64, 65, 66, 67, 68, 69, 70, 71,
    96, 97, 98, 99, 100, 101, 102, 103, 104, 112, 128, 129, 130, 131, 176, 177, 178, 192,
];

/// The `RAW_HASH_MAX_BYTES` carried-constant bound.
const RAW_HASH_MAX_BYTES: u64 = 1_048_576;

/// Traits bits of `jg_tinfo`: equality, total order, canonical hash,
/// persistable, contains-float.
const TR_TO: u64 = 2;
const TR_HS: u64 = 4;
const TR_PS: u64 = 8;
const TR_FL: u64 = 16;

// ----- seed-side literals -----

fn bytes_const(value: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(value.to_vec()),
    }
}

fn u64_const(value: u64) -> ConstValue {
    ConstValue {
        value_type: t::u64(),
        data: ConstData::UInt(u128::from(value)),
    }
}

fn bool_const(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

fn lit_map(key: Ty, value: Ty, mut entries: Vec<(ConstValue, ConstValue)>) -> E {
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

/// `Map<Bytes, UInt(64)>`: one-byte `Bytes` to its value.
fn byte_table() -> E {
    lit_map(
        t::bytes(),
        t::u64(),
        (0..=255_u8)
            .map(|b| (bytes_const(&[b]), u64_const(u64::from(b))))
            .collect(),
    )
}

/// `Map<UInt(64), Bytes>`: the single-byte varints 0..128.
fn uv_table() -> E {
    lit_map(
        t::u64(),
        t::bytes(),
        (0..128_u8)
            .map(|b| (u64_const(u64::from(b)), bytes_const(&[b])))
            .collect(),
    )
}

/// `Map<UInt(64), UInt(64)>`: `2^k` for `k` in 0..64.
fn pow2_table() -> E {
    lit_map(
        t::u64(),
        t::u64(),
        (0..64_u32).map(|k| (u64_const(k.into()), u64_const(1_u64 << k))).collect(),
    )
}

/// `Map<UInt(64), Bool>`: the frozen bootstrap opcodes.
fn bootstrap_table() -> E {
    lit_map(
        t::u64(),
        t::bool(),
        BOOTSTRAP_OPCODES
            .iter()
            .map(|op| (u64_const(*op), bool_const(true)))
            .collect(),
    )
}

/// Opcode family: 1 data, 2 records/variants, 3 vectors, 4 maps,
/// 5 integers, 6 floats, 7 comparisons/Boolean, 8 calls/options/results,
/// 9 contracts/bridges/cells/hash/globals/function references.
fn family_table() -> E {
    let mut rows = Vec::new();
    for (family, opcodes) in [
        (1_u64, &[1_u64, 16, 17][..]),
        (2, &[18, 19, 20, 21]),
        (3, &[32, 33, 34, 35]),
        (4, &[36, 37, 38, 39, 40]),
        (5, &[64, 65, 66, 67, 68, 69, 70, 71]),
        (6, &[80, 81, 82, 83, 84, 85]),
        (7, &[96, 97, 98, 99, 100, 101, 102, 103, 104]),
        (8, &[112, 128, 129, 130, 131]),
        (9, &[144, 161, 176, 177, 178, 192, 193, 194]),
    ] {
        for opcode in opcodes {
            rows.push((u64_const(*opcode), u64_const(family)));
        }
    }
    lit_map(t::u64(), t::u64(), rows)
}

fn tyb(ty: &TypeExpr) -> E {
    ty_lit(ty)
}

fn u64_ty() -> E {
    tyb(&t::u64())
}

fn u32_ty() -> E {
    tyb(&TypeExpr::UInt(IntegerWidth::from_bits(32)))
}

fn bool_ty() -> E {
    tyb(&TypeExpr::Bool)
}

fn unit_ty() -> E {
    tyb(&TypeExpr::Unit)
}

fn bytes_ty() -> E {
    tyb(&TypeExpr::Bytes)
}

fn bf(kind: BuiltinFailureKind) -> E {
    tyb(&TypeExpr::BuiltinFailure(kind))
}

/// The frozen identity of a bridge entry (discriminator 0).
fn bridge(code4: &[u8; 4]) -> E {
    bytesc(bridge_id(*code4, 0).as_bytes())
}

/// Whether type bytes `e` carry tag `tag` (tags are single bytes).
fn tag_is(e: E, tag: u8) -> E {
    eq(slice(e, u64c(0), u64c(1)), bytesc(&[tag]))
}

/// Bit `k` of a `UInt(64)` mask.
fn bit(mask: E, k: E) -> E {
    eq(rem(div(mask, mget(pow2_table(), k)), u64c(2)), u64c(1))
}

/// Bit for a literal power of two.
fn has(mask: E, power: u64) -> E {
    eq(rem(div(mask, u64c(power)), u64c(2)), u64c(1))
}

fn ot(i: u64) -> E {
    vget(v("ot"), u64c(i))
}

fn sig_ty() -> Ty {
    t::tuple(vec![t::u64(), t::vec(t::bytes()), t::bytes(), t::vec(t::bytes())])
}

fn br_ty() -> Ty {
    t::tuple(vec![t::u64(), t::bool(), t::bytes(), t::bytes()])
}

fn fam_ty() -> Ty {
    t::tuple(vec![t::u64(), t::bytes()])
}

/// Defines the `jg_*` functions.
pub fn define(m: &mut Module) {
    // A lowering code as the toolchain error type.
    m.func("jg_code", &[("c", t::u64())], t::u32(), |f| {
        f.ret(mget(
            lit_map(
                t::u64(),
                t::u32(),
                (26_000..=26_006_u32)
                    .map(|c| {
                        (
                            u64_const(u64::from(c)),
                            ConstValue {
                                value_type: t::u32(),
                                data: ConstData::UInt(u128::from(c)),
                            },
                        )
                    })
                    .collect(),
            ),
            v("c"),
        ));
    });
    define_types(m);
    define_traits(m);
    define_ops(m);
    define_function(m);
    define_gate(m);
    define_tests(m);
}

// ----- type bytes -----

#[allow(clippy::too_many_lines)]
fn define_types(m: &mut Module) {
    // One byte of `bs` as UInt(64).
    m.func("jg_byte", &[("bs", t::bytes()), ("p", t::u64())], t::u64(), |f| {
        f.ret(mget(byte_table(), slice(v("bs"), v("p"), u64c(1))));
    });
    // A canonical LE7 varint at `p`: (value, new position).
    m.func(
        "jg_rdv",
        &[("bs", t::bytes()), ("p", t::u64())],
        t::tuple(vec![t::u64(), t::u64()]),
        |f| {
            f.var("value", u64c(0));
            f.var("mult", u64c(1));
            f.var("p", v("p"));
            f.loop_(|f| {
                f.let_("b", call("jg_byte", vec![v("bs"), v("p")]));
                f.set("p", add(v("p"), u64c(1)));
                f.if_(lt(v("b"), u64c(128)), |f| {
                    f.ret(tup(vec![add(v("value"), mul(v("b"), v("mult"))), v("p")]));
                });
                f.set("value", add(v("value"), mul(sub(v("b"), u64c(128)), v("mult"))));
                f.set("mult", mul(v("mult"), u64c(128)));
            });
        },
    );
    // Varint bytes.
    m.func("jg_uv", &[("x", t::u64())], t::bytes(), |f| {
        f.if_(lt(v("x"), u64c(128)), |f| f.ret(mget(uv_table(), v("x"))));
        f.ret(call("uvarb", vec![v("x")]));
    });
    // The payload of a type: `uvar(tag) || uvar(len) || payload`.
    m.func("jg_pl", &[("ty", t::bytes())], t::bytes(), |f| {
        f.let_("n", blen(v("ty")));
        f.let_(
            "k",
            select(
                lt(v("n"), u64c(130)),
                u64c(1),
                select(
                    lt(v("n"), u64c(16_387)),
                    u64c(2),
                    select(lt(v("n"), u64c(2_097_156)), u64c(3), u64c(4)),
                ),
            ),
        );
        f.ret(slice(v("ty"), add(v("k"), u64c(1)), sub(sub(v("n"), u64c(1)), v("k"))));
    });
    // A type from a tag and payload.
    m.func("jg_mk", &[("tag", t::u64()), ("p", t::bytes())], t::bytes(), |f| {
        f.ret(concat(vnew(
            t::bytes(),
            vec![
                call("jg_uv", vec![v("tag")]),
                call("jg_uv", vec![blen(v("p"))]),
                v("p"),
            ],
        )));
    });
    // A two-field record payload `{1: a, 2: b}`.
    m.func("jg_rec2", &[("a", t::bytes()), ("b", t::bytes())], t::bytes(), |f| {
        f.ret(concat(vnew(
            t::bytes(),
            vec![
                bytesc(&[2, 1]),
                call("jg_uv", vec![blen(v("a"))]),
                v("a"),
                bytesc(&[2]),
                call("jg_uv", vec![blen(v("b"))]),
                v("b"),
            ],
        )));
    });
    // The two members of a Result / OrderedMap type.
    m.func(
        "jg_pair",
        &[("ty", t::bytes())],
        t::tuple(vec![t::bytes(), t::bytes()]),
        |f| {
            f.let_("p", call("jg_pl", vec![v("ty")]));
            f.let_("r1", call("jg_rdv", vec![v("p"), u64c(2)]));
            f.let_("s", tget(v("r1"), 1));
            f.let_("la", tget(v("r1"), 0));
            f.let_("r2", call("jg_rdv", vec![v("p"), add(add(v("s"), v("la")), u64c(1))]));
            f.ret(tup(vec![
                slice(v("p"), v("s"), v("la")),
                slice(v("p"), tget(v("r2"), 1), tget(v("r2"), 0)),
            ]));
        },
    );
    // Items of a `List<TypeExpr>` encoding.
    m.func("jg_listp", &[("lb", t::bytes())], t::vec(t::bytes()), |f| {
        f.let_("r", call("jg_rdv", vec![v("lb"), u64c(0)]));
        f.var("p", tget(v("r"), 1));
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("i", u64c(0), tget(v("r"), 0), |f| {
            f.let_("r2", call("jg_rdv", vec![v("lb"), v("p")]));
            f.set("out", push(v("out"), slice(v("lb"), tget(v("r2"), 1), tget(v("r2"), 0))));
            f.set("p", add(tget(v("r2"), 1), tget(v("r2"), 0)));
        });
        f.ret(v("out"));
    });
    // A `List<TypeExpr>` encoding.
    m.func("jg_listb", &[("items", t::vec(t::bytes()))], t::bytes(), |f| {
        f.var("parts", vnew(t::bytes(), vec![call("jg_uv", vec![vlen(v("items"))])]));
        f.for_u64("i", u64c(0), vlen(v("items")), |f| {
            f.let_("item", vget(v("items"), v("i")));
            f.set("parts", push(v("parts"), call("jg_uv", vec![blen(v("item"))])));
            f.set("parts", push(v("parts"), v("item")));
        });
        f.ret(concat(v("parts")));
    });
    // (definition, arguments) of a Named type.
    m.func(
        "jg_named",
        &[("ty", t::bytes())],
        t::tuple(vec![t::bytes(), t::vec(t::bytes())]),
        |f| {
            f.let_("p", call("jg_pl", vec![v("ty")]));
            f.let_("r", call("jg_rdv", vec![v("p"), u64c(36)]));
            f.ret(tup(vec![
                slice(v("p"), u64c(3), u64c(32)),
                call("jg_listp", vec![slice(v("p"), tget(v("r"), 1), tget(v("r"), 0))]),
            ]));
        },
    );
    // `Named(def, [])`.
    m.func("jg_named0", &[("def", t::bytes())], t::bytes(), |f| {
        f.ret(concat(vnew(
            t::bytes(),
            vec![bytesc(&[10, 38, 2, 1, 32]), v("def"), bytesc(&[2, 1, 0])],
        )));
    });
    // The definition of a non-generic Named type (`named_definition`).
    m.func("jg_ndef", &[("ty", t::bytes())], t::opt(t::bytes()), |f| {
        f.if_(and(eq(blen(v("ty")), u64c(40)), tag_is(v("ty"), 10)), |f| {
            f.ret(some(slice(v("ty"), u64c(5), u64c(32))));
        });
        f.ret(none(t::bytes()));
    });
    // (parameters, result) of a FunctionRef type.
    m.func(
        "jg_fparts",
        &[("ty", t::bytes())],
        t::tuple(vec![t::vec(t::bytes()), t::bytes()]),
        |f| {
            f.let_("p", call("jg_pl", vec![v("ty")]));
            f.let_("r1", call("jg_rdv", vec![v("p"), u64c(2)]));
            f.let_("ls", tget(v("r1"), 1));
            f.let_("ll", tget(v("r1"), 0));
            f.let_("r2", call("jg_rdv", vec![v("p"), add(add(v("ls"), v("ll")), u64c(1))]));
            f.ret(tup(vec![
                call("jg_listp", vec![slice(v("p"), v("ls"), v("ll"))]),
                slice(v("p"), tget(v("r2"), 1), tget(v("r2"), 0)),
            ]));
        },
    );
    // A FunctionRef type from its parts.
    m.func(
        "jg_funref",
        &[
            ("params", t::vec(t::bytes())),
            ("result", t::bytes()),
            ("effects", t::vec(t::bytes())),
        ],
        t::bytes(),
        |f| {
            f.let_("lb", call("jg_listb", vec![v("params")]));
            f.var("sp", vnew(t::bytes(), vec![call("jg_uv", vec![vlen(v("effects"))])]));
            f.for_u64("i", u64c(0), vlen(v("effects")), |f| {
                f.set("sp", push(v("sp"), bytesc(&[32])));
                f.set("sp", push(v("sp"), vget(v("effects"), v("i"))));
            });
            f.let_("sb", concat(v("sp")));
            f.ret(call(
                "jg_mk",
                vec![
                    u64c(15),
                    concat(vnew(
                        t::bytes(),
                        vec![
                            bytesc(&[3, 1]),
                            call("jg_uv", vec![blen(v("lb"))]),
                            v("lb"),
                            bytesc(&[2]),
                            call("jg_uv", vec![blen(v("result"))]),
                            v("result"),
                            bytesc(&[3]),
                            call("jg_uv", vec![blen(v("sb"))]),
                            v("sb"),
                        ],
                    )),
                ],
            ));
        },
    );
    // The nested type expressions of one type (not through definitions).
    m.func("jg_kids", &[("ty", t::bytes())], t::vec(t::bytes()), |f| {
        f.let_("tag", call("jg_byte", vec![v("ty"), u64c(0)]));
        f.if_(lt(v("tag"), u64c(9)), |f| f.ret(vnew(t::bytes(), vec![])));
        f.if_(eq(v("tag"), u64c(9)), |f| {
            f.ret(call("jg_listp", vec![call("jg_pl", vec![v("ty")])]));
        });
        f.if_(eq(v("tag"), u64c(10)), |f| {
            f.ret(tget(call("jg_named", vec![v("ty")]), 1));
        });
        f.if_(
            or(or(eq(v("tag"), u64c(11)), eq(v("tag"), u64c(13))), eq(v("tag"), u64c(18))),
            |f| f.ret(vnew(t::bytes(), vec![call("jg_pl", vec![v("ty")])])),
        );
        f.if_(or(eq(v("tag"), u64c(12)), eq(v("tag"), u64c(14))), |f| {
            f.let_("pr", call("jg_pair", vec![v("ty")]));
            f.ret(vnew(t::bytes(), vec![tget(v("pr"), 0), tget(v("pr"), 1)]));
        });
        f.if_(eq(v("tag"), u64c(15)), |f| {
            f.let_("fp", call("jg_fparts", vec![v("ty")]));
            f.ret(push(tget(v("fp"), 0), tget(v("fp"), 1)));
        });
        f.ret(vnew(t::bytes(), vec![]));
    });
    // Explicit-stack push that overwrites past `top`.
    m.func(
        "jg_spush",
        &[("xs", t::vec(t::bytes())), ("top", t::u64()), ("x", t::bytes())],
        t::vec(t::bytes()),
        |f| {
            f.if_(lt(v("top"), vlen(v("xs"))), |f| f.ret(vset(v("xs"), v("top"), v("x"))));
            f.ret(push(v("xs"), v("x")));
        },
    );
    m.func(
        "jg_upush",
        &[("xs", t::vec(t::u64())), ("top", t::u64()), ("x", t::u64())],
        t::vec(t::u64()),
        |f| {
            f.if_(lt(v("top"), vlen(v("xs"))), |f| f.ret(vset(v("xs"), v("top"), v("x"))));
            f.ret(push(v("xs"), v("x")));
        },
    );
    // Bit mask of every type tag occurring in a type expression (through
    // Named arguments and FunctionRef signatures, not definitions).
    m.func("jg_mask", &[("ty", t::bytes())], t::u64(), |f| {
        f.var("st", vnew(t::bytes(), vec![v("ty")]));
        f.var("top", u64c(1));
        f.var("mask", u64c(0));
        f.while_(gt(v("top"), u64c(0)), |f| {
            f.set("top", sub(v("top"), u64c(1)));
            f.let_("x", vget(v("st"), v("top")));
            f.let_("tag", call("jg_byte", vec![v("x"), u64c(0)]));
            f.if_(not(bit(v("mask"), v("tag"))), |f| {
                f.set("mask", add(v("mask"), mget(pow2_table(), v("tag"))));
            });
            f.if_(gt(v("tag"), u64c(8)), |f| {
                f.let_("kids", call("jg_kids", vec![v("x")]));
                f.for_u64("i", u64c(0), vlen(v("kids")), |f| {
                    f.set(
                        "st",
                        call("jg_spush", vec![v("st"), v("top"), vget(v("kids"), v("i"))]),
                    );
                    f.set("top", add(v("top"), u64c(1)));
                });
            });
        });
        f.ret(v("mask"));
    });
}

// ----- S20-210 traits -----

#[allow(clippy::too_many_lines)]
fn define_traits(m: &mut Module) {
    // Traits of one closed type: equality 1, total order 2, canonical hash
    // 4, persistable 8 (`TypeEnvironment::traits`; a failing `traits`
    // reads as all-false, which every judgment use treats identically).
    // Named types expand through their definitions with explicit argument
    // environments (`substitute`).
    m.func("jg_traits", &[("inv", m.ty(INV)), ("ty", t::bytes())], t::u64(), |f| {
        f.var("st", vnew(t::bytes(), vec![v("ty")]));
        f.var("se", vnew(t::u64(), vec![u64c(0)]));
        f.var("top", u64c(1));
        // Environment 0 is empty; others are (arguments, parent).
        f.var("eargs", vnew(t::vec(t::bytes()), vec![vnew(t::bytes(), vec![])]));
        f.var("epar", vnew(t::u64(), vec![u64c(0)]));
        f.var("seen", mnew(t::bytes(), t::bool(), vec![]));
        f.var("eq_", boolc(true));
        f.var("to", boolc(true));
        f.var("hs", boolc(true));
        f.var("ps", boolc(true));
        f.while_(gt(v("top"), u64c(0)), |f| {
            f.set("top", sub(v("top"), u64c(1)));
            f.let_("x", vget(v("st"), v("top")));
            f.let_("env", vget(v("se"), v("top")));
            f.let_("tag", call("jg_byte", vec![v("x"), u64c(0)]));
            let pushk = |f: &mut FnB<'_>, ty: E, env: E| {
                f.set("st", call("jg_spush", vec![v("st"), v("top"), ty]));
                f.set("se", call("jg_upush", vec![v("se"), v("top"), env]));
                f.set("top", add(v("top"), u64c(1)));
            };
            f.if_(or(eq(v("tag"), u64c(5)), eq(v("tag"), u64c(6))), |f| {
                f.set("to", boolc(false));
            });
            f.if_(eq(v("tag"), u64c(9)), |f| {
                f.let_("items", call("jg_listp", vec![call("jg_pl", vec![v("x")])]));
                f.for_u64("i", u64c(0), vlen(v("items")), |f| {
                    pushk(f, vget(v("items"), v("i")), v("env"));
                });
            });
            f.if_(eq(v("tag"), u64c(10)), |f| {
                f.let_("nm", call("jg_named", vec![v("x")]));
                f.let_("def", tget(v("nm"), 0));
                f.let_("args", tget(v("nm"), 1));
                f.var("ne", u64c(0));
                f.var("walk", boolc(true));
                f.if_else(
                    eq(vlen(v("args")), u64c(0)),
                    |f| {
                        f.if_else(
                            mhas(v("seen"), v("def")),
                            |f| f.set("walk", boolc(false)),
                            |f| f.set("seen", mins(v("seen"), v("def"), boolc(true))),
                        );
                    },
                    |f| {
                        f.set("ne", vlen(v("eargs")));
                        f.set("eargs", push(v("eargs"), v("args")));
                        f.set("epar", push(v("epar"), v("env")));
                    },
                );
                f.if_(v("walk"), |f| {
                    f.match_opt(
                        call("inv_tdef", vec![v("inv"), v("def")]),
                        "td",
                        |f| {
                            f.let_("ms", field(v("td"), "members"));
                            f.for_u64("i", u64c(0), vlen(v("ms")), |f| {
                                f.let_("mm", vget(v("ms"), v("i")));
                                f.if_(field(v("mm"), "has_ty"), |f| {
                                    pushk(f, field(v("mm"), "ty"), v("ne"));
                                });
                            });
                        },
                        |f| f.ret(u64c(0)),
                    );
                });
            });
            f.if_(or(eq(v("tag"), u64c(11)), eq(v("tag"), u64c(12))), |f| {
                f.set("to", boolc(false));
            });
            f.if_(or(eq(v("tag"), u64c(11)), eq(v("tag"), u64c(13))), |f| {
                pushk(f, call("jg_pl", vec![v("x")]), v("env"));
            });
            f.if_(or(eq(v("tag"), u64c(12)), eq(v("tag"), u64c(14))), |f| {
                f.let_("pr", call("jg_pair", vec![v("x")]));
                pushk(f, tget(v("pr"), 0), v("env"));
                pushk(f, tget(v("pr"), 1), v("env"));
            });
            f.if_(eq(v("tag"), u64c(15)), |f| f.set("to", boolc(false)));
            f.if_(or(eq(v("tag"), u64c(16)), eq(v("tag"), u64c(17))), |f| {
                f.set("to", boolc(false));
                f.set("hs", boolc(false));
                f.set("ps", boolc(false));
            });
            f.if_(eq(v("tag"), u64c(18)), |f| f.ret(u64c(0)));
            f.if_(eq(v("tag"), u64c(19)), |f| {
                f.if_(eq(v("env"), u64c(0)), |f| f.ret(u64c(0)));
                f.let_("pi", call("jg_rdv", vec![call("jg_pl", vec![v("x")]), u64c(0)]));
                f.let_("ea", vget(v("eargs"), v("env")));
                f.if_(ge(tget(v("pi"), 0), vlen(v("ea"))), |f| f.ret(u64c(0)));
                pushk(f, vget(v("ea"), tget(v("pi"), 0)), vget(v("epar"), v("env")));
            });
        });
        let b = |name: &str, weight: u64| select(v(name), u64c(weight), u64c(0));
        f.ret(add(add(add(b("eq_", 1), b("to", 2)), b("hs", 4)), b("ps", 8)));
    });
    // Traits plus the contains-float bit (16) of one type.
    m.func("jg_tinfo", &[("inv", m.ty(INV)), ("ty", t::bytes())], t::u64(), |f| {
        f.let_("mask", call("jg_mask", vec![v("ty")]));
        f.let_(
            "fl",
            select(or(has(v("mask"), 1 << 5), has(v("mask"), 1 << 6)), u64c(TR_FL), u64c(0)),
        );
        f.if_(has(v("mask"), 1 << 19), |f| f.ret(v("fl")));
        f.ret(add(call("jg_traits", vec![v("inv"), v("ty")]), v("fl")));
    });
}

// ----- operation judgment -----

/// Defines one family judgment: `(code, derived type)`.
fn family(m: &mut Module, name: &str, body: impl FnOnce(&mut FnB<'_>) + 'static) {
    m.func(
        name,
        &[
            ("inv", m.ty(INV)),
            ("op", m.ty(OP)),
            ("ot", t::vec(t::bytes())),
            ("tr", t::u64()),
            ("sig", sig_ty()),
            ("br", br_ty()),
        ],
        fam_ty(),
        move |f| {
            f.let_("opc", field(v("op"), "opcode"));
            f.let_("imm", field(v("op"), "imm"));
            f.let_("itag", field(v("imm"), "tag"));
            f.let_("n", vlen(v("ot")));
            f.let_("dec", field(v("op"), "results"));
            body(f);
        },
    );
}

fn fail(f: &mut FnB<'_>, code_: u64) {
    f.ret(tup(vec![u64c(code_), bytesc(&[])]));
}

fn done(f: &mut FnB<'_>, derived: E) {
    f.ret(tup(vec![u64c(0), derived]));
}

fn fail_if(f: &mut FnB<'_>, condition: E, code_: u64) {
    f.if_(condition, move |f| fail(f, code_));
}

fn imm_none(f: &mut FnB<'_>) {
    fail_if(f, ne(v("itag"), u64c(1)), IM);
}

fn arity(f: &mut FnB<'_>, count: u64) {
    fail_if(f, ne(v("n"), u64c(count)), SM);
}

fn mk(tag: u64, payload: E) -> E {
    call("jg_mk", vec![u64c(tag), payload])
}

fn rec2(a: E, b: E) -> E {
    call("jg_rec2", vec![a, b])
}

fn pl(ty: E) -> E {
    call("jg_pl", vec![ty])
}

fn is_int(ty: E) -> E {
    or(tag_is(ty.clone(), 3), tag_is(ty, 4))
}

fn is_float(ty: E) -> E {
    or(tag_is(ty.clone(), 5), tag_is(ty, 6))
}

#[allow(clippy::too_many_lines)]
fn define_ops(m: &mut Module) {
    // A non-generic definition of the requested form.
    m.func(
        "jg_tdef",
        &[("inv", m.ty(INV)), ("id", t::bytes()), ("rec", t::bool())],
        t::opt(m.ty(TDEF)),
        |f| {
            f.match_opt(
                call("inv_tdef", vec![v("inv"), v("id")]),
                "td",
                |f| {
                    f.if_(
                        and(
                            eq(vlen(field(v("td"), "tparams")), u64c(0)),
                            eq(field(v("td"), "is_rec"), v("rec")),
                        ),
                        |f| f.ret(some(v("td"))),
                    );
                },
                |_| {},
            );
            f.ret(none(m_ty(TDEF)));
        },
    );
    // The member `mid` of a definition.
    m.func(
        "jg_member",
        &[("td", m.ty(TDEF)), ("mid", t::bytes())],
        t::opt(m.ty(MEM)),
        |f| {
            f.let_("ms", field(v("td"), "members"));
            f.for_u64("i", u64c(0), vlen(v("ms")), |f| {
                f.let_("mm", vget(v("ms"), v("i")));
                f.if_(eq(field(v("mm"), "member"), v("mid")), |f| f.ret(some(v("mm"))));
            });
            f.ret(none(m_ty(MEM)));
        },
    );
    // Data: constant_ref, tuple_new, tuple_get.
    family(m, "jg_f1", |f| {
        f.if_(eq(v("opc"), u64c(1)), |f| {
            fail_if(f, ne(v("itag"), u64c(2)), IM);
            arity(f, 0);
            f.match_opt(
                call("inv_cst", vec![v("inv"), field(v("imm"), "id")]),
                "c",
                |f| done(f, field(v("c"), "ty")),
                |f| fail(f, IM),
            );
        });
        f.if_(eq(v("opc"), u64c(16)), |f| {
            imm_none(f);
            fail_if(f, gt(v("n"), u64c(64)), SM);
            done(f, mk(9, call("jg_listb", vec![v("ot")])));
        });
        fail_if(f, ne(v("itag"), u64c(3)), IM);
        arity(f, 1);
        fail_if(f, not(tag_is(ot(0), 9)), SM);
        f.let_("items", call("jg_listp", vec![pl(ot(0))]));
        f.let_("ix", field(v("imm"), "index"));
        fail_if(f, ge(v("ix"), vlen(v("items"))), IM);
        done(f, vget(v("items"), v("ix")));
    });
    // Records and variants.
    family(m, "jg_f2", |f| {
        f.if_(eq(v("opc"), u64c(18)), |f| {
            fail_if(f, ne(v("itag"), u64c(2)), IM);
            f.let_("def", field(v("imm"), "id"));
            f.match_opt(
                call("jg_tdef", vec![v("inv"), v("def"), boolc(true)]),
                "td",
                |f| {
                    f.let_("ms", field(v("td"), "members"));
                    fail_if(f, ne(v("n"), vlen(v("ms"))), SM);
                    f.for_u64("i", u64c(0), v("n"), |f| {
                        fail_if(
                            f,
                            ne(vget(v("ot"), v("i")), field(vget(v("ms"), v("i")), "ty")),
                            SM,
                        );
                    });
                    done(f, call("jg_named0", vec![v("def")]));
                },
                |f| fail(f, IM),
            );
        });
        f.if_(eq(v("opc"), u64c(19)), |f| {
            fail_if(f, ne(v("itag"), u64c(4)), IM);
            arity(f, 1);
            f.match_opt(
                call("jg_ndef", vec![ot(0)]),
                "def",
                |f| {
                    f.match_opt(
                        call("jg_tdef", vec![v("inv"), v("def"), boolc(true)]),
                        "td",
                        |f| {
                            f.match_opt(
                                call("jg_member", vec![v("td"), field(v("imm"), "id")]),
                                "mm",
                                |f| done(f, field(v("mm"), "ty")),
                                |f| fail(f, IM),
                            );
                        },
                        |f| fail(f, IM),
                    );
                },
                |f| fail(f, SM),
            );
        });
        // Variants: the immediate is Variant.
        fail_if(f, ne(v("itag"), u64c(5)), IM);
        f.let_("def", field(v("imm"), "id"));
        f.if_(eq(v("opc"), u64c(21)), |f| {
            arity(f, 1);
            f.match_opt(
                call("jg_ndef", vec![ot(0)]),
                "d0",
                |f| fail_if(f, ne(v("d0"), v("def")), SM),
                |f| fail(f, SM),
            );
        });
        f.match_opt(
            call("jg_tdef", vec![v("inv"), v("def"), boolc(false)]),
            "td",
            |f| {
                f.match_opt(
                    call("jg_member", vec![v("td"), field(v("imm"), "member")]),
                    "case",
                    |f| {
                        f.if_(eq(v("opc"), u64c(21)), |f| {
                            fail_if(f, not(field(v("case"), "has_ty")), IM);
                            done(f, mk(13, field(v("case"), "ty")));
                        });
                        f.if_else(
                            field(v("case"), "has_ty"),
                            |f| {
                                arity(f, 1);
                                fail_if(f, ne(ot(0), field(v("case"), "ty")), SM);
                            },
                            |f| arity(f, 0),
                        );
                        done(f, call("jg_named0", vec![v("def")]));
                    },
                    |f| fail(f, IM),
                );
            },
            |f| fail(f, IM),
        );
    });
    // Vectors.
    family(m, "jg_f3", |f| {
        imm_none(f);
        f.if_(eq(v("opc"), u64c(32)), |f| {
            f.if_(eq(v("n"), u64c(0)), |f| {
                fail_if(f, ne(vlen(v("dec")), u64c(1)), SM);
                fail_if(f, not(tag_is(vget(v("dec"), u64c(0)), 11)), SM);
                done(f, vget(v("dec"), u64c(0)));
            });
            f.for_u64("i", u64c(1), v("n"), |f| {
                fail_if(f, ne(vget(v("ot"), v("i")), ot(0)), SM);
            });
            done(f, mk(11, ot(0)));
        });
        f.if_(eq(v("opc"), u64c(33)), |f| {
            arity(f, 1);
            fail_if(f, not(tag_is(ot(0), 11)), SM);
            done(f, u64_ty());
        });
        f.if_(eq(v("opc"), u64c(34)), |f| {
            arity(f, 2);
            fail_if(f, not(tag_is(ot(0), 11)), SM);
            fail_if(f, ne(ot(1), u64_ty()), SM);
            done(f, mk(13, pl(ot(0))));
        });
        arity(f, 3);
        fail_if(f, not(tag_is(ot(0), 11)), SM);
        fail_if(f, or(ne(ot(1), u64_ty()), ne(ot(2), pl(ot(0)))), SM);
        done(f, mk(14, rec2(ot(0), bf(BuiltinFailureKind::Index))));
    });
    // Maps (`tr`: the key's traits).
    family(m, "jg_f4", |f| {
        imm_none(f);
        f.let_(
            "adm",
            and(has(v("tr"), TR_TO), not(has(v("tr"), TR_FL))),
        );
        f.if_(eq(v("opc"), u64c(36)), |f| {
            fail_if(f, ne(rem(v("n"), u64c(2)), u64c(0)), SM);
            f.if_(eq(v("n"), u64c(0)), |f| {
                fail_if(f, ne(vlen(v("dec")), u64c(1)), SM);
                f.let_("d0", vget(v("dec"), u64c(0)));
                fail_if(f, not(tag_is(v("d0"), 14)), SM);
                f.let_("pr", call("jg_pair", vec![v("d0")]));
                fail_if(f, not(tag_is(tget(v("pr"), 0), 12)), SM);
                fail_if(f, ne(tget(v("pr"), 1), bf(BuiltinFailureKind::DuplicateKey)), SM);
                fail_if(f, not(v("adm")), SM);
                done(f, v("d0"));
            });
            f.for_u64("i", u64c(2), v("n"), |f| {
                fail_if(
                    f,
                    ne(vget(v("ot"), v("i")), vget(v("ot"), rem(v("i"), u64c(2)))),
                    SM,
                );
            });
            fail_if(f, not(v("adm")), SM);
            done(
                f,
                mk(
                    14,
                    rec2(mk(12, rec2(ot(0), ot(1))), bf(BuiltinFailureKind::DuplicateKey)),
                ),
            );
        });
        f.if_else(eq(v("opc"), u64c(39)), |f| arity(f, 3), |f| arity(f, 2));
        fail_if(f, not(tag_is(ot(0), 12)), SM);
        f.let_("pr", call("jg_pair", vec![ot(0)]));
        fail_if(f, ne(ot(1), tget(v("pr"), 0)), SM);
        f.if_(eq(v("opc"), u64c(37)), |f| done(f, mk(13, tget(v("pr"), 1))));
        f.if_(eq(v("opc"), u64c(38)), |f| done(f, bool_ty()));
        f.if_(eq(v("opc"), u64c(39)), |f| {
            fail_if(f, ne(ot(2), tget(v("pr"), 1)), SM);
        });
        done(f, ot(0));
    });
    // Checked integers.
    family(m, "jg_f5", |f| {
        imm_none(f);
        let arith = || mk(14, rec2(ot(0), bf(BuiltinFailureKind::Arithmetic)));
        f.if_(eq(v("opc"), u64c(69)), |f| {
            arity(f, 1);
            fail_if(f, not(tag_is(ot(0), 3)), SM);
            done(f, arith());
        });
        arity(f, 2);
        f.if_(ge(v("opc"), u64c(70)), |f| {
            fail_if(f, or(not(is_int(ot(0))), ne(ot(1), u32_ty())), SM);
            done(f, arith());
        });
        fail_if(f, or(ne(ot(0), ot(1)), not(is_int(ot(0)))), SM);
        done(f, arith());
    });
    // Floats.
    family(m, "jg_f6", |f| {
        imm_none(f);
        f.if_(eq(v("opc"), u64c(84)), |f| {
            arity(f, 1);
            fail_if(f, not(is_float(ot(0))), SM);
            done(f, ot(0));
        });
        f.if_(eq(v("opc"), u64c(85)), |f| {
            arity(f, 3);
            fail_if(
                f,
                or(or(ne(ot(0), ot(1)), ne(ot(1), ot(2))), not(is_float(ot(0)))),
                SM,
            );
            done(f, ot(0));
        });
        arity(f, 2);
        fail_if(f, or(ne(ot(0), ot(1)), not(is_float(ot(0)))), SM);
        done(f, ot(0));
    });
    // Comparisons and Boolean operations.
    family(m, "jg_f7", |f| {
        imm_none(f);
        f.if_(eq(v("opc"), u64c(102)), |f| {
            arity(f, 1);
            fail_if(f, ne(ot(0), bool_ty()), SM);
            done(f, bool_ty());
        });
        arity(f, 2);
        f.if_(ge(v("opc"), u64c(103)), |f| {
            fail_if(f, or(ne(ot(0), bool_ty()), ne(ot(1), bool_ty())), SM);
            done(f, bool_ty());
        });
        fail_if(f, ne(ot(0), ot(1)), SM);
        f.if_(le(v("opc"), u64c(97)), |f| {
            fail_if(f, not(has(v("tr"), TR_HS)), SM);
            fail_if(f, and(has(v("tr"), TR_FL), not(is_float(ot(0)))), SM);
            done(f, bool_ty());
        });
        f.let_("tag", call("jg_byte", vec![ot(0), u64c(0)]));
        fail_if(f, or(lt(v("tag"), u64c(2)), gt(v("tag"), u64c(8))), SM);
        done(f, bool_ty());
    });
    // Direct calls, options, results.
    family(m, "jg_f8", |f| {
        f.if_(eq(v("opc"), u64c(112)), |f| {
            fail_if(f, ne(v("itag"), u64c(7)), IM);
            fail_if(f, ne(vlen(field(v("imm"), "targs")), u64c(0)), IM);
            fail_if(f, ne(tget(v("sig"), 0), u64c(0)), IM);
            f.let_("ps", tget(v("sig"), 1));
            fail_if(f, ne(v("n"), vlen(v("ps"))), SM);
            f.for_u64("i", u64c(0), v("n"), |f| {
                fail_if(f, ne(vget(v("ot"), v("i")), vget(v("ps"), v("i"))), SM);
            });
            done(f, tget(v("sig"), 2));
        });
        imm_none(f);
        f.if_(eq(v("opc"), u64c(128)), |f| {
            arity(f, 1);
            done(f, mk(13, ot(0)));
        });
        f.if_(eq(v("opc"), u64c(129)), |f| {
            arity(f, 0);
            fail_if(f, ne(vlen(v("dec")), u64c(1)), SM);
            fail_if(f, not(tag_is(vget(v("dec"), u64c(0)), 13)), SM);
            done(f, vget(v("dec"), u64c(0)));
        });
        arity(f, 1);
        fail_if(f, ne(vlen(v("dec")), u64c(1)), SM);
        f.let_("d0", vget(v("dec"), u64c(0)));
        fail_if(f, not(tag_is(v("d0"), 14)), SM);
        f.let_("pr", call("jg_pair", vec![v("d0")]));
        f.let_(
            "want",
            select(eq(v("opc"), u64c(130)), tget(v("pr"), 0), tget(v("pr"), 1)),
        );
        fail_if(f, ne(ot(0), v("want")), SM);
        done(f, v("d0"));
    });
    // Contracts, bridges, cells, hashing, globals, function references.
    family(m, "jg_f9", |f| {
        f.if_(eq(v("opc"), u64c(144)), |f| {
            fail_if(f, ne(v("itag"), u64c(2)), IM);
            // No ContractDefinition is in the toolchain's domain.
            fail(f, IM);
        });
        f.if_(eq(v("opc"), u64c(193)), |f| {
            fail_if(f, ne(v("itag"), u64c(2)), IM);
            arity(f, 0);
            // No GlobalValueDefinition is in the toolchain's domain.
            fail(f, IM);
        });
        f.if_(eq(v("opc"), u64c(194)), |f| {
            fail_if(f, ne(v("itag"), u64c(7)), IM);
            fail_if(f, ne(vlen(field(v("imm"), "targs")), u64c(0)), IM);
            arity(f, 0);
            fail_if(f, ne(tget(v("sig"), 0), u64c(0)), IM);
            done(
                f,
                call(
                    "jg_funref",
                    vec![tget(v("sig"), 1), tget(v("sig"), 2), tget(v("sig"), 3)],
                ),
            );
        });
        f.if_(eq(v("opc"), u64c(161)), |f| {
            fail_if(f, ne(v("itag"), u64c(2)), IM);
            f.let_("kind", tget(v("br"), 0));
            fail_if(f, eq(v("kind"), u64c(0)), OU);
            arity(f, 2);
            f.let_("req", tget(v("br"), 2));
            f.let_("resp", tget(v("br"), 3));
            f.if_else(
                eq(v("kind"), u64c(3)),
                |f| {
                    fail_if(f, not(tget(v("br"), 1)), SM);
                    fail_if(f, or(ne(ot(1), v("req")), ne(ot(0), v("resp"))), SM);
                },
                |f| {
                    f.let_("scope", select(eq(v("kind"), u64c(8)), bytes_ty(), unit_ty()));
                    fail_if(f, or(ne(ot(0), v("scope")), ne(ot(1), v("req"))), SM);
                },
            );
            done(f, mk(14, rec2(v("resp"), bf(BuiltinFailureKind::Index))));
        });
        imm_none(f);
        f.if_(eq(v("opc"), u64c(176)), |f| {
            arity(f, 1);
            fail_if(f, not(has(v("tr"), TR_PS)), SM);
            done(f, mk(18, ot(0)));
        });
        f.if_(eq(v("opc"), u64c(177)), |f| {
            arity(f, 1);
            fail_if(f, not(tag_is(ot(0), 18)), SM);
            done(f, pl(ot(0)));
        });
        f.if_(eq(v("opc"), u64c(178)), |f| {
            arity(f, 2);
            fail_if(f, not(tag_is(ot(0), 18)), SM);
            fail_if(f, ne(ot(1), pl(ot(0))), SM);
            done(f, unit_ty());
        });
        // value_hash
        arity(f, 1);
        fail_if(f, not(has(v("tr"), TR_HS)), SM);
        done(f, bytes_ty());
    });
    // One operation: the cell precheck, the family judgment, and the
    // exact declared-result comparison.
    m.func(
        "jg_op",
        &[
            ("inv", m.ty(INV)),
            ("op", m.ty(OP)),
            ("ot", t::vec(t::bytes())),
            ("cell", t::bool()),
            ("tr", t::u64()),
            ("sig", sig_ty()),
            ("br", br_ty()),
        ],
        t::u64(),
        |f| {
            f.let_("opc", field(v("op"), "opcode"));
            f.if_(
                and(v("cell"), and(ne(v("opc"), u64c(177)), ne(v("opc"), u64c(178)))),
                |f| f.ret(u64c(SM)),
            );
            f.var("fam", u64c(0));
            f.match_opt(
                mget_opt(family_table(), v("opc")),
                "x",
                |f| f.set("fam", v("x")),
                |f| f.ret(u64c(OU)),
            );
            let args = || vec![v("inv"), v("op"), v("ot"), v("tr"), v("sig"), v("br")];
            f.var("res", tup(vec![u64c(0), bytesc(&[])]));
            for k in 1..=9_u64 {
                let name = format!("jg_f{k}");
                f.if_(eq(v("fam"), u64c(k)), move |f| {
                    f.set("res", call(&name, args()));
                });
            }
            f.let_("c", tget(v("res"), 0));
            f.if_(ne(v("c"), u64c(0)), |f| f.ret(v("c")));
            f.let_("dec", field(v("op"), "results"));
            f.if_(ne(vlen(v("dec")), u64c(1)), |f| f.ret(u64c(SM)));
            if !crate::seed::fault("judge-declared") {
                f.if_(ne(vget(v("dec"), u64c(0)), tget(v("res"), 1)), |f| f.ret(u64c(SM)));
            }
            f.ret(u64c(0));
        },
    );
    // The `map_new` key type: the first operand, or the declared map's key.
    m.func(
        "jg_mkey",
        &[("ot", t::vec(t::bytes())), ("dec", t::vec(t::bytes()))],
        t::opt(t::bytes()),
        |f| {
            f.if_(gt(vlen(v("ot")), u64c(0)), |f| f.ret(some(ot(0))));
            f.if_(ne(vlen(v("dec")), u64c(1)), |f| f.ret(none(t::bytes())));
            f.let_("d0", vget(v("dec"), u64c(0)));
            f.if_(not(tag_is(v("d0"), 14)), |f| f.ret(none(t::bytes())));
            f.let_("okt", tget(call("jg_pair", vec![v("d0")]), 0));
            f.if_(not(tag_is(v("okt"), 12)), |f| f.ret(none(t::bytes())));
            f.ret(some(tget(call("jg_pair", vec![v("okt")]), 0)));
        },
    );
    // A callee signature (`function_signature`): (status, parameters,
    // result, effects).
    m.func("jg_sig", &[("inv", m.ty(INV)), ("id", t::bytes())], sig_ty(), |f| {
        let bad = || {
            tup(vec![
                u64c(IM),
                vnew(t::bytes(), vec![]),
                bytesc(&[]),
                vnew(t::bytes(), vec![]),
            ])
        };
        f.match_opt(
            call("inv_fun", vec![v("inv"), v("id")]),
            "g",
            |f| {
                f.if_(ne(vlen(field(v("g"), "tparams")), u64c(0)), move |f| f.ret(bad()));
                f.let_("ps", field(v("g"), "params"));
                f.var("tys", vnew(t::bytes(), vec![]));
                f.for_u64("i", u64c(0), vlen(v("ps")), |f| {
                    f.match_opt(
                        call("inv_prm", vec![v("inv"), vget(v("ps"), v("i"))]),
                        "p",
                        |f| f.set("tys", push(v("tys"), field(v("p"), "ty"))),
                        move |f| f.ret(bad()),
                    );
                });
                f.ret(tup(vec![
                    u64c(0),
                    v("tys"),
                    field(v("g"), "result"),
                    field(v("g"), "effects"),
                ]));
            },
            move |f| f.ret(bad()),
        );
    });
    // Whether an import row is a frozen-field push row (`is_push_row`).
    m.func("jg_push_row", &[("imp", m.ty(IMP))], t::bool(), |f| {
        f.let_("id", field(v("imp"), "id"));
        f.ret(and(
            and(
                call("jg_push_id", vec![v("id")]),
                call("jg_frozen", vec![v("imp")]),
            ),
            eq(field(v("imp"), "resp"), mk(11, field(v("imp"), "req"))),
        ));
    });
    // `is_push_identity`.
    m.func("jg_push_id", &[("id", t::bytes())], t::bool(), |f| {
        f.ret(and(
            eq(slice(v("id"), u64c(0), u64c(16)), bytesc(b"SLY1/BRIDGE/PSH1")),
            eq(slice(v("id"), u64c(20), u64c(12)), bytesc(&[0; 12])),
        ));
    });
    // The frozen import fields: adapter identity, ABI 1, `Index` failure,
    // no effects.
    m.func("jg_frozen", &[("imp", m.ty(IMP))], t::bool(), |f| {
        f.ret(and(
            and(
                eq(field(v("imp"), "adapter"), field(v("imp"), "id")),
                eq(field(v("imp"), "abi"), u64c(1)),
            ),
            and(
                eq(field(v("imp"), "fail"), bf(BuiltinFailureKind::Index)),
                eq(vlen(field(v("imp"), "effects")), u64c(0)),
            ),
        ));
    });
    // The resolved non-push bridge kind of a carried row
    // (`resolve_bridge_entry`): 0 unresolved, 1 B2V1, 2 V2B1, 4 RHW1,
    // 5 RHW2, 6 RSH2, 7 BCT1, 8 BSL1, 9 BLN1.
    m.func("jg_row_kind", &[("imp", m.ty(IMP))], t::u64(), |f| {
        f.let_("id", field(v("imp"), "id"));
        f.var("kind", u64c(0));
        f.var("req", bytesc(&[]));
        f.var("resp", bytesc(&[]));
        let u8v = TypeExpr::Vector(Box::new(t::u8()));
        let rows: Vec<(u64, &[u8; 4], TypeExpr, TypeExpr)> = vec![
            (1, b"B2V1", t::bytes(), u8v.clone()),
            (2, b"V2B1", u8v, t::bytes()),
            (4, b"RHW1", t::bytes(), t::bytes()),
            (5, b"RHW2", t::bytes(), t::bytes()),
            (6, b"RSH2", t::bytes(), t::bytes()),
            (7, b"BCT1", t::vec(t::bytes()), t::bytes()),
            (8, b"BSL1", t::tuple(vec![t::u64(), t::u64()]), t::bytes()),
            (9, b"BLN1", t::bytes(), t::u64()),
        ];
        for (kind, code4, request, response) in rows {
            f.if_(eq(v("id"), bridge(code4)), move |f| {
                f.set("kind", u64c(kind));
                f.set("req", tyb(&request));
                f.set("resp", tyb(&response));
            });
        }
        f.if_(eq(v("kind"), u64c(0)), |f| f.ret(u64c(0)));
        f.if_(not(call("jg_frozen", vec![v("imp")])), |f| f.ret(u64c(0)));
        f.if_(
            or(ne(field(v("imp"), "req"), v("req")), ne(field(v("imp"), "resp"), v("resp"))),
            |f| f.ret(u64c(0)),
        );
        f.ret(v("kind"));
    });
    // Bridge resolution for one `adapter_invoke` immediate: (kind, push
    // row conforming, request, response); kind 0 refuses, 3 is push.
    m.func("jg_bridge", &[("inv", m.ty(INV)), ("id", t::bytes())], br_ty(), |f| {
        f.let_("row", call("inv_imp", vec![v("inv"), v("id")]));
        f.if_(call("jg_push_id", vec![v("id")]), |f| {
            f.var("rowok", boolc(false));
            f.var("req", bytesc(&[]));
            f.var("resp", bytesc(&[]));
            f.match_opt(
                v("row"),
                "imp",
                |f| {
                    f.set("rowok", call("jg_push_row", vec![v("imp")]));
                    f.set("req", field(v("imp"), "req"));
                    f.set("resp", field(v("imp"), "resp"));
                },
                |_| {},
            );
            f.var("ok_", v("rowok"));
            f.if_(eq(v("id"), bridge(b"PSH1")), |f| {
                // The frozen identity is approved by any carried push row.
                f.set("ok_", boolc(false));
                f.let_("imps", field(v("inv"), "imps"));
                f.for_u64("i", u64c(0), vlen(v("imps")), |f| {
                    f.if_(call("jg_push_row", vec![vget(v("imps"), v("i"))]), |f| {
                        f.set("ok_", boolc(true));
                    });
                });
            });
            f.ret(tup(vec![
                select(v("ok_"), u64c(3), u64c(0)),
                v("rowok"),
                v("req"),
                v("resp"),
            ]));
        });
        f.match_opt(
            v("row"),
            "imp",
            |f| {
                f.ret(tup(vec![
                    call("jg_row_kind", vec![v("imp")]),
                    boolc(true),
                    field(v("imp"), "req"),
                    field(v("imp"), "resp"),
                ]));
            },
            |_| {},
        );
        f.ret(tup(vec![u64c(0), boolc(false), bytesc(&[]), bytesc(&[])]));
    });
}

/// A model type by name.
fn m_ty(name: &str) -> Ty {
    Module::new(super::TOOLCHAIN_SEED).ty(name)
}

// ----- jg_function -----

#[allow(clippy::too_many_lines)]
fn define_function(m: &mut Module) {
    // Memoized tag mask insertion.
    m.func(
        "jg_mmemo",
        &[("mm", t::map(t::bytes(), t::u64())), ("ty", t::bytes())],
        t::map(t::bytes(), t::u64()),
        |f| {
            f.if_(mhas(v("mm"), v("ty")), |f| f.ret(v("mm")));
            f.ret(mins(v("mm"), v("ty"), call("jg_mask", vec![v("ty")])));
        },
    );
    m.func("jg_function", &[("inv", m.ty(INV)), ("fid", t::bytes())], r(t::unit()), |f| {
        f.var("fun_", none(m_ty(FUN)));
        f.match_opt(
            call("inv_fun", vec![v("inv"), v("fid")]),
            "g",
            |f| f.set("fun_", some(v("g"))),
            |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
        );
        f.let_("fun", unwrap(v("fun_")));
        f.let_("blocks", field(v("fun"), "blocks"));
        // Register types (`Maps::build`), with tag masks.
        f.var("pty", mnew(t::bytes(), t::bytes(), vec![]));
        f.var("oty", mnew(t::bytes(), t::vec(t::bytes()), vec![]));
        f.var("mm", mnew(t::bytes(), t::u64(), vec![]));
        f.var("ops", vnew(m_ty(OP), vec![]));
        f.var("regs", u64c(0));
        f.var("big", boolc(false));
        let param = |f: &mut FnB<'_>, id: E| {
            f.let_("pid", id);
            f.match_opt(
                call("inv_prm", vec![v("inv"), v("pid")]),
                "p",
                |f| {
                    f.set("pty", mins(v("pty"), v("pid"), field(v("p"), "ty")));
                    f.set("mm", call("jg_mmemo", vec![v("mm"), field(v("p"), "ty")]));
                    f.set("regs", add(v("regs"), u64c(1)));
                },
                |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
            );
        };
        f.let_("fps", field(v("fun"), "params"));
        f.for_u64("i", u64c(0), vlen(v("fps")), move |f| param(f, vget(v("fps"), v("i"))));
        f.for_u64("bi", u64c(0), vlen(v("blocks")), move |f| {
            f.match_opt(
                call("inv_blk", vec![v("inv"), vget(v("blocks"), v("bi"))]),
                "blk",
                move |f| {
                    f.let_("bps", field(v("blk"), "params"));
                    f.for_u64("i", u64c(0), vlen(v("bps")), move |f| {
                        param(f, vget(v("bps"), v("i")));
                    });
                    f.let_("bops", field(v("blk"), "ops"));
                    f.for_u64("j", u64c(0), vlen(v("bops")), |f| {
                        f.let_("oid", vget(v("bops"), v("j")));
                        f.match_opt(
                            call("inv_op", vec![v("inv"), v("oid")]),
                            "op",
                            |f| {
                                f.let_("res", field(v("op"), "results"));
                                f.if_(
                                    or(
                                        gt(vlen(v("res")), u64c(65_535)),
                                        gt(vlen(field(v("op"), "operands")), u64c(65_535)),
                                    ),
                                    |f| f.set("big", boolc(true)),
                                );
                                f.set("oty", mins(v("oty"), v("oid"), v("res")));
                                f.for_u64("k", u64c(0), vlen(v("res")), |f| {
                                    f.set(
                                        "mm",
                                        call("jg_mmemo", vec![v("mm"), vget(v("res"), v("k"))]),
                                    );
                                });
                                f.set("regs", add(v("regs"), vlen(v("res"))));
                                f.set("ops", push(v("ops"), v("op")));
                            },
                            |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
                        );
                    });
                },
                |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
            );
        });
        // Resource preflight (`preflight_resources`).
        f.if_(
            or(
                or(gt(vlen(v("blocks")), u64c(4_096)), gt(vlen(v("ops")), u64c(1_000_000))),
                or(v("big"), gt(v("regs"), u64c(1_000_000))),
            ),
            |f| f.ret_err(code(VM_LOWER_RESOURCE_LIMIT)),
        );
        // `check_result_type`: no cell, handle, or token in the result.
        f.let_("rmask", call("jg_mask", vec![field(v("fun"), "result")]));
        f.if_(
            or(
                or(has(v("rmask"), 1 << 16), has(v("rmask"), 1 << 17)),
                has(v("rmask"), 1 << 18),
            ),
            |f| f.ret_err(code(VM_LOWER_SIGNATURE_MISMATCH)),
        );
        f.var("trm", mnew(t::bytes(), t::u64(), vec![]));
        f.var("sgm", mnew(t::bytes(), sig_ty(), vec![]));
        f.var("brm", mnew(t::bytes(), br_ty(), vec![]));
        f.for_u64("i", u64c(0), vlen(v("ops")), |f| {
            f.let_("op", vget(v("ops"), v("i")));
            f.let_("vrs", field(v("op"), "operands"));
            f.var("ot", vnew(t::bytes(), vec![]));
            f.var("cell", boolc(false));
            f.for_u64("j", u64c(0), vlen(v("vrs")), |f| {
                f.let_("vr", vget(v("vrs"), v("j")));
                f.var("ty", bytesc(&[]));
                f.if_else(
                    eq(field(v("vr"), "kind"), u64c(1)),
                    |f| {
                        f.match_opt(
                            mget_opt(v("pty"), field(v("vr"), "id")),
                            "x",
                            |f| f.set("ty", v("x")),
                            |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
                        );
                    },
                    |f| {
                        f.match_opt(
                            mget_opt(v("oty"), field(v("vr"), "id")),
                            "rs",
                            |f| {
                                f.match_opt(
                                    vget_opt(v("rs"), field(v("vr"), "idx")),
                                    "x",
                                    |f| f.set("ty", v("x")),
                                    |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
                                );
                            },
                            |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
                        );
                    },
                );
                f.if_(has(mget(v("mm"), v("ty")), 1 << 18), |f| f.set("cell", boolc(true)));
                f.set("ot", push(v("ot"), v("ty")));
            });
            f.let_("opc", field(v("op"), "opcode"));
            f.let_("imm", field(v("op"), "imm"));
            // Traits of the judged operand (or the map key).
            f.var("key", none(t::bytes()));
            f.if_(
                and(
                    gt(vlen(v("ot")), u64c(0)),
                    or(
                        or(eq(v("opc"), u64c(96)), eq(v("opc"), u64c(97))),
                        or(eq(v("opc"), u64c(176)), eq(v("opc"), u64c(192))),
                    ),
                ),
                |f| f.set("key", some(vget(v("ot"), u64c(0)))),
            );
            f.if_(eq(v("opc"), u64c(36)), |f| {
                f.set("key", call("jg_mkey", vec![v("ot"), field(v("op"), "results")]));
            });
            f.var("tr", u64c(0));
            f.match_opt(
                v("key"),
                "kt",
                |f| {
                    f.match_opt(
                        mget_opt(v("trm"), v("kt")),
                        "x",
                        |f| f.set("tr", v("x")),
                        |f| {
                            f.set("tr", call("jg_tinfo", vec![v("inv"), v("kt")]));
                            f.set("trm", mins(v("trm"), v("kt"), v("tr")));
                        },
                    );
                },
                |_| {},
            );
            f.var(
                "sig",
                tup(vec![u64c(0), vnew(t::bytes(), vec![]), bytesc(&[]), vnew(t::bytes(), vec![])]),
            );
            f.if_(
                and(
                    or(eq(v("opc"), u64c(112)), eq(v("opc"), u64c(194))),
                    eq(field(v("imm"), "tag"), u64c(7)),
                ),
                |f| {
                    f.let_("cid", field(v("imm"), "id"));
                    f.match_opt(
                        mget_opt(v("sgm"), v("cid")),
                        "x",
                        |f| f.set("sig", v("x")),
                        |f| {
                            f.set("sig", call("jg_sig", vec![v("inv"), v("cid")]));
                            f.set("sgm", mins(v("sgm"), v("cid"), v("sig")));
                        },
                    );
                },
            );
            f.var("br", tup(vec![u64c(0), boolc(false), bytesc(&[]), bytesc(&[])]));
            f.if_(
                and(eq(v("opc"), u64c(161)), eq(field(v("imm"), "tag"), u64c(2))),
                |f| {
                    f.let_("bid", field(v("imm"), "id"));
                    f.match_opt(
                        mget_opt(v("brm"), v("bid")),
                        "x",
                        |f| f.set("br", v("x")),
                        |f| {
                            f.set("br", call("jg_bridge", vec![v("inv"), v("bid")]));
                            f.set("brm", mins(v("brm"), v("bid"), v("br")));
                        },
                    );
                },
            );
            f.let_(
                "c",
                call(
                    "jg_op",
                    vec![v("inv"), v("op"), v("ot"), v("cell"), v("tr"), v("sig"), v("br")],
                ),
            );
            f.if_(ne(v("c"), u64c(0)), |f| f.ret_err(call("jg_code", vec![v("c")])));
        });
        // `require_canonical_referenced_constants`: every constant a
        // `constant_ref` names resolved above, and a decoded canonical
        // constant always re-encodes (the loader admitted exactly its
        // canonical bytes), so nothing further can refuse here.
        f.ret_ok(unitc());
    });
}

// ----- jg_gate -----

#[allow(clippy::too_many_lines)]
fn define_gate(m: &mut Module) {
    // Structural bootstrap scan of one type (not through definitions):
    // (bad at a boundary, bad inside an execution, Named definitions).
    m.func(
        "jg_bt_scan",
        &[("ty", t::bytes())],
        t::tuple(vec![t::bool(), t::bool(), t::vec(t::bytes())]),
        |f| {
            f.var("st", vnew(t::bytes(), vec![v("ty")]));
            f.var("top", u64c(1));
            f.var("bb", boolc(false));
            f.var("bn", boolc(false));
            f.var("deps", vnew(t::bytes(), vec![]));
            f.while_(gt(v("top"), u64c(0)), |f| {
                f.set("top", sub(v("top"), u64c(1)));
                f.let_("x", vget(v("st"), v("top")));
                f.let_("tag", call("jg_byte", vec![v("x"), u64c(0)]));
                f.if_(
                    or(
                        or(eq(v("tag"), u64c(5)), eq(v("tag"), u64c(6))),
                        or(
                            or(eq(v("tag"), u64c(15)), eq(v("tag"), u64c(16))),
                            or(eq(v("tag"), u64c(17)), eq(v("tag"), u64c(19))),
                        ),
                    ),
                    |f| {
                        f.set("bb", boolc(true));
                        f.set("bn", boolc(true));
                    },
                );
                f.if_(eq(v("tag"), u64c(18)), |f| f.set("bb", boolc(true)));
                f.if_(
                    and(gt(v("tag"), u64c(8)), and(ne(v("tag"), u64c(15)), ne(v("tag"), u64c(19)))),
                    |f| {
                        f.if_(eq(v("tag"), u64c(10)), |f| {
                            f.set("deps", push(v("deps"), slice(call("jg_pl", vec![v("x")]), u64c(3), u64c(32))));
                        });
                        f.let_("kids", call("jg_kids", vec![v("x")]));
                        f.for_u64("i", u64c(0), vlen(v("kids")), |f| {
                            f.set(
                                "st",
                                call("jg_spush", vec![v("st"), v("top"), vget(v("kids"), v("i"))]),
                            );
                            f.set("top", add(v("top"), u64c(1)));
                        });
                    },
                );
            });
            f.ret(tup(vec![v("bb"), v("bn"), v("deps")]));
        },
    );
    // Per-definition badness (boundary, inside) as maps keyed by definition:
    // a definition is bad when an excluded form is reachable through its
    // members and the definitions they name (`bootstrap_type_ok`'s
    // coinductive walk accepts exactly when none is reachable).
    m.func(
        "jg_bt_defs",
        &[("inv", m.ty(INV))],
        t::tuple(vec![t::map(t::bytes(), t::bool()), t::map(t::bytes(), t::bool())]),
        |f| {
            f.let_("tds", field(v("inv"), "tdefs"));
            f.let_("nd", vlen(v("tds")));
            f.var("db", vnew(t::bool(), vec![]));
            f.var("dn", vnew(t::bool(), vec![]));
            f.var("deps", vnew(t::vec(t::bytes()), vec![]));
            f.for_u64("i", u64c(0), v("nd"), |f| {
                f.let_("ms", field(vget(v("tds"), v("i")), "members"));
                f.var("b1", boolc(false));
                f.var("b2", boolc(false));
                f.var("ds", vnew(t::bytes(), vec![]));
                f.for_u64("j", u64c(0), vlen(v("ms")), |f| {
                    f.let_("mm", vget(v("ms"), v("j")));
                    f.if_(field(v("mm"), "has_ty"), |f| {
                        f.let_("sc", call("jg_bt_scan", vec![field(v("mm"), "ty")]));
                        f.if_(tget(v("sc"), 0), |f| f.set("b1", boolc(true)));
                        f.if_(tget(v("sc"), 1), |f| f.set("b2", boolc(true)));
                        f.let_("more", tget(v("sc"), 2));
                        f.for_u64("k", u64c(0), vlen(v("more")), |f| {
                            f.set("ds", push(v("ds"), vget(v("more"), v("k"))));
                        });
                    });
                });
                f.set("db", push(v("db"), v("b1")));
                f.set("dn", push(v("dn"), v("b2")));
                f.set("deps", push(v("deps"), v("ds")));
            });
            // Propagate badness to a fixpoint.
            f.var("changed", boolc(true));
            f.while_(v("changed"), |f| {
                f.set("changed", boolc(false));
                f.for_u64("i", u64c(0), v("nd"), |f| {
                    f.let_("ds", vget(v("deps"), v("i")));
                    f.for_u64("k", u64c(0), vlen(v("ds")), |f| {
                        f.match_opt(
                            call("inv_find", vec![
                                field(v("inv"), "index"),
                                vget(v("ds"), v("k")),
                                u64c(super::inv::KIND_TYPEDEF),
                            ]),
                            "q",
                            |f| {
                                f.if_(and(vget(v("db"), v("q")), not(vget(v("db"), v("i")))), |f| {
                                    f.set("db", vset(v("db"), v("i"), boolc(true)));
                                    f.set("changed", boolc(true));
                                });
                                f.if_(and(vget(v("dn"), v("q")), not(vget(v("dn"), v("i")))), |f| {
                                    f.set("dn", vset(v("dn"), v("i"), boolc(true)));
                                    f.set("changed", boolc(true));
                                });
                            },
                            |f| {
                                // An unresolvable definition fails closed.
                                f.if_(not(vget(v("db"), v("i"))), |f| {
                                    f.set("db", vset(v("db"), v("i"), boolc(true)));
                                    f.set("changed", boolc(true));
                                });
                                f.if_(not(vget(v("dn"), v("i"))), |f| {
                                    f.set("dn", vset(v("dn"), v("i"), boolc(true)));
                                    f.set("changed", boolc(true));
                                });
                            },
                        );
                    });
                });
            });
            f.var("mb", mnew(t::bytes(), t::bool(), vec![]));
            f.var("mn", mnew(t::bytes(), t::bool(), vec![]));
            f.for_u64("i", u64c(0), v("nd"), |f| {
                f.let_("id", field(vget(v("tds"), v("i")), "id"));
                f.set("mb", mins(v("mb"), v("id"), vget(v("db"), v("i"))));
                f.set("mn", mins(v("mn"), v("id"), vget(v("dn"), v("i"))));
            });
            f.ret(tup(vec![v("mb"), v("mn")]));
        },
    );
    // `bootstrap_type_ok` against per-definition badness `bad` (the map
    // for the same boundary mode).
    m.func(
        "jg_bt_ok",
        &[("ty", t::bytes()), ("boundary", t::bool()), ("bad", t::map(t::bytes(), t::bool()))],
        t::bool(),
        |f| {
            f.let_("sc", call("jg_bt_scan", vec![v("ty")]));
            f.if_(select(v("boundary"), tget(v("sc"), 0), tget(v("sc"), 1)), |f| {
                f.ret(boolc(false));
            });
            f.let_("ds", tget(v("sc"), 2));
            f.for_u64("i", u64c(0), vlen(v("ds")), |f| {
                f.match_opt(
                    mget_opt(v("bad"), vget(v("ds"), v("i"))),
                    "b",
                    |f| f.if_(v("b"), |f| f.ret(boolc(false))),
                    |f| f.ret(boolc(false)),
                );
            });
            f.ret(boolc(true));
        },
    );
    // Whether one carried row is a bootstrap-schema bridge row under V3
    // (`bootstrap_row_ok`; every resolvable kind is admitted under V3).
    m.func(
        "jg_row_ok",
        &[("imp", m.ty(IMP)), ("bb", t::map(t::bytes(), t::bool()))],
        t::bool(),
        |f| {
            f.var("ok_", boolc(false));
            f.if_else(
                call("jg_push_id", vec![field(v("imp"), "id")]),
                |f| f.set("ok_", call("jg_push_row", vec![v("imp")])),
                |f| f.set("ok_", ne(call("jg_row_kind", vec![v("imp")]), u64c(0))),
            );
            f.if_(not(v("ok_")), |f| f.ret(boolc(false)));
            f.ret(and(
                call("jg_bt_ok", vec![field(v("imp"), "req"), boolc(true), v("bb")]),
                call("jg_bt_ok", vec![field(v("imp"), "resp"), boolc(true), v("bb")]),
            ));
        },
    );
    // A carried `Bytes` constant longer than the raw-hash ceiling.
    m.func("jg_big_bytes", &[("value", t::bytes()), ("ty", t::bytes())], t::bool(), |f| {
        f.if_(ne(v("ty"), bytes_ty()), |f| f.ret(boolc(false)));
        // ConstValue {1: Bytes type [7, 0], 2: ConstData}: the data starts
        // at offset 6 after `uvar(2) uvar(1) uvar(2) 7 0 uvar(2)`.
        f.let_("r1", call("jg_rdv", vec![v("value"), u64c(6)]));
        f.let_("p", tget(v("r1"), 1));
        f.if_(ne(call("jg_byte", vec![v("value"), v("p")]), u64c(7)), |f| {
            f.ret(boolc(false));
        });
        f.let_("r2", call("jg_rdv", vec![v("value"), add(v("p"), u64c(1))]));
        f.let_("r3", call("jg_rdv", vec![v("value"), tget(v("r2"), 1)]));
        f.ret(gt(tget(v("r3"), 0), u64c(RAW_HASH_MAX_BYTES)));
    });
    // The gate.
    m.func(
        "jg_gate",
        &[("inv", m.ty(INV)), ("entry", t::bytes()), ("image", t::bytes())],
        r(m.ty(GATE)),
        |f| {
            f.let_("index", field(v("inv"), "index"));
            f.let_("fns", field(v("inv"), "fns"));
            f.let_("nf", vlen(v("fns")));
            let find = |id: E, kind: u64| call("inv_find", vec![v("index"), id, u64c(kind)]);
            f.if_(not(is_some(find(v("entry"), KIND_FUNCTION))), |f| {
                f.ret_err(code(VM_LOWER_IMMEDIATE_MISMATCH));
            });
            f.let_("bads", call("jg_bt_defs", vec![v("inv")]));
            f.let_("badb", tget(v("bads"), 0));
            f.let_("badn", tget(v("bads"), 1));
            // Per-function vectors.
            f.var("zeros", vnew(t::u64(), vec![]));
            f.var("falses", vnew(t::bool(), vec![]));
            f.for_u64("i", u64c(0), v("nf"), |f| {
                f.set("zeros", push(v("zeros"), u64c(0)));
                f.set("falses", push(v("falses"), boolc(false)));
            });
            f.var("okn", mnew(t::bytes(), t::bool(), vec![]));
            f.var("okb", mnew(t::bytes(), t::bool(), vec![]));
            // Blocks: owner positions and block-parameter membership.
            f.var("bfn", mnew(t::bytes(), t::u64(), vec![]));
            f.var("bpbad", v("falses"));
            f.let_("blks", field(v("inv"), "blks"));
            f.for_u64("i", u64c(0), vlen(v("blks")), |f| {
                f.let_("blk", vget(v("blks"), v("i")));
                f.match_opt(
                    find(field(v("blk"), "function"), KIND_FUNCTION),
                    "fp",
                    |f| {
                        f.set("bfn", mins(v("bfn"), field(v("blk"), "id"), v("fp")));
                        f.let_("bps", field(v("blk"), "params"));
                        f.for_u64("j", u64c(0), vlen(v("bps")), |f| {
                            f.match_opt(
                                call("inv_prm", vec![v("inv"), vget(v("bps"), v("j"))]),
                                "p",
                                |f| {
                                    f.let_("ty", field(v("p"), "ty"));
                                    f.if_(not(mhas(v("okn"), v("ty"))), |f| {
                                        f.set(
                                            "okn",
                                            mins(
                                                v("okn"),
                                                v("ty"),
                                                call("jg_bt_ok", vec![v("ty"), boolc(false), v("badn")]),
                                            ),
                                        );
                                    });
                                    f.if_(not(mget(v("okn"), v("ty"))), |f| {
                                        f.set("bpbad", vset(v("bpbad"), v("fp"), boolc(true)));
                                    });
                                },
                                |_| {},
                            );
                        });
                    },
                    |_| {},
                );
            });
            // Operations in inventory order: calls, generic calls, first
            // per-function refusal, counts, referenced imports.
            f.var("ecount", v("zeros"));
            f.var("efrom", vnew(t::u64(), vec![]));
            f.var("eto", vnew(t::bytes(), vec![]));
            f.var("generic", v("falses"));
            f.var("first", v("zeros"));
            f.var("opn", u64c(0));
            f.var("brn", u64c(0));
            f.var("refd", mnew(t::bytes(), t::bool(), vec![]));
            f.let_("ops", field(v("inv"), "ops"));
            f.for_u64("i", u64c(0), vlen(v("ops")), |f| {
                f.let_("op", vget(v("ops"), v("i")));
                f.match_opt(
                    mget_opt(v("bfn"), field(v("op"), "block")),
                    "fp",
                    |f| {
                        f.set("opn", add(v("opn"), u64c(1)));
                        f.let_("opc", field(v("op"), "opcode"));
                        f.let_("imm", field(v("op"), "imm"));
                        f.if_(
                            and(eq(v("opc"), u64c(112)), eq(field(v("imm"), "tag"), u64c(7))),
                            |f| {
                                f.if_else(
                                    eq(vlen(field(v("imm"), "targs")), u64c(0)),
                                    |f| {
                                        f.set("efrom", push(v("efrom"), v("fp")));
                                        f.set("eto", push(v("eto"), field(v("imm"), "id")));
                                        f.set(
                                            "ecount",
                                            vset(
                                                v("ecount"),
                                                v("fp"),
                                                add(vget(v("ecount"), v("fp")), u64c(1)),
                                            ),
                                        );
                                    },
                                    |f| f.set("generic", vset(v("generic"), v("fp"), boolc(true))),
                                );
                            },
                        );
                        f.var("c", u64c(0));
                        f.if_else(
                            eq(v("opc"), u64c(161)),
                            |f| {
                                f.set("brn", add(v("brn"), u64c(1)));
                                f.if_else(
                                    ne(field(v("imm"), "tag"), u64c(2)),
                                    |f| f.set("c", u64c(IM)),
                                    |f| {
                                        f.set(
                                            "refd",
                                            mins(v("refd"), field(v("imm"), "id"), boolc(true)),
                                        );
                                        f.if_(
                                            not(is_some(find(field(v("imm"), "id"), KIND_IMPORT))),
                                            |f| f.set("c", u64c(OU)),
                                        );
                                    },
                                );
                            },
                            |f| {
                                f.if_(not(mhas(bootstrap_table(), v("opc"))), |f| {
                                    f.set("c", u64c(OU));
                                });
                            },
                        );
                        f.if_(eq(v("c"), u64c(0)), |f| {
                            f.let_("res", field(v("op"), "results"));
                            f.for_u64("k", u64c(0), vlen(v("res")), |f| {
                                f.let_("ty", vget(v("res"), v("k")));
                                f.if_(not(mhas(v("okn"), v("ty"))), |f| {
                                    f.set(
                                        "okn",
                                        mins(
                                            v("okn"),
                                            v("ty"),
                                            call("jg_bt_ok", vec![v("ty"), boolc(false), v("badn")]),
                                        ),
                                    );
                                });
                                f.if_(not(mget(v("okn"), v("ty"))), |f| f.set("c", u64c(SM)));
                            });
                        });
                        f.if_(
                            and(ne(v("c"), u64c(0)), eq(vget(v("first"), v("fp")), u64c(0))),
                            |f| f.set("first", vset(v("first"), v("fp"), v("c"))),
                        );
                    },
                    |_| {},
                );
            });
            // Callees grouped per function in inventory order.
            f.var("starts", v("zeros"));
            f.var("acc", u64c(0));
            f.for_u64("i", u64c(0), v("nf"), |f| {
                f.set("starts", vset(v("starts"), v("i"), v("acc")));
                f.set("acc", add(v("acc"), vget(v("ecount"), v("i"))));
            });
            f.var("cursor", v("starts"));
            f.var("eord", vnew(t::bytes(), vec![]));
            f.for_u64("e", u64c(0), vlen(v("eto")), |f| {
                f.set("eord", push(v("eord"), bytesc(&[])));
            });
            f.for_u64("e", u64c(0), vlen(v("eto")), |f| {
                f.let_("fp", vget(v("efrom"), v("e")));
                f.let_("at", vget(v("cursor"), v("fp")));
                f.set("eord", vset(v("eord"), v("at"), vget(v("eto"), v("e"))));
                f.set("cursor", vset(v("cursor"), v("fp"), add(v("at"), u64c(1))));
            });
            // The pre-order closure walk (explicit stack; 1 visiting, 2 done).
            f.var("state", v("zeros"));
            f.var("reached", vnew(t::bytes(), vec![]));
            f.var("sf", vnew(t::u64(), vec![]));
            f.var("si", vnew(t::u64(), vec![]));
            f.var("top", u64c(0));
            f.var("pend", v("entry"));
            f.var("has_pend", boolc(true));
            f.loop_(|f| {
                f.if_(v("has_pend"), |f| {
                    f.set("has_pend", boolc(false));
                    f.match_opt(
                        find(v("pend"), KIND_FUNCTION),
                        "fp",
                        |f| {
                            f.let_("st", vget(v("state"), v("fp")));
                            f.if_(eq(v("st"), u64c(1)), |f| {
                                f.ret_err(code(VM_LOWER_PROFILE_UNSUPPORTED));
                            });
                            f.if_(eq(v("st"), u64c(0)), |f| {
                                f.let_("g", vget(v("fns"), v("fp")));
                                f.if_(
                                    or(
                                        or(
                                            gt(vlen(field(v("g"), "effects")), u64c(0)),
                                            gt(vlen(field(v("g"), "contracts")), u64c(0)),
                                        ),
                                        gt(vlen(field(v("g"), "tparams")), u64c(0)),
                                    ),
                                    |f| f.ret_err(code(VM_LOWER_PROFILE_UNSUPPORTED)),
                                );
                                f.set("reached", push(v("reached"), v("pend")));
                                f.if_(vget(v("generic"), v("fp")), |f| {
                                    f.ret_err(code(VM_LOWER_PROFILE_UNSUPPORTED));
                                });
                                f.set("state", vset(v("state"), v("fp"), u64c(1)));
                                f.set("sf", call("jg_upush", vec![v("sf"), v("top"), v("fp")]));
                                f.set(
                                    "si",
                                    call(
                                        "jg_upush",
                                        vec![v("si"), v("top"), vget(v("starts"), v("fp"))],
                                    ),
                                );
                                f.set("top", add(v("top"), u64c(1)));
                            });
                        },
                        |f| f.ret_err(code(VM_LOWER_IMMEDIATE_MISMATCH)),
                    );
                });
                f.if_(eq(v("top"), u64c(0)), |f| f.break_());
                f.let_("t1", sub(v("top"), u64c(1)));
                f.let_("cf", vget(v("sf"), v("t1")));
                f.let_("ci", vget(v("si"), v("t1")));
                f.if_else(
                    lt(v("ci"), add(vget(v("starts"), v("cf")), vget(v("ecount"), v("cf")))),
                    |f| {
                        f.set("si", vset(v("si"), v("t1"), add(v("ci"), u64c(1))));
                        f.set("pend", vget(v("eord"), v("ci")));
                        f.set("has_pend", boolc(true));
                    },
                    |f| {
                        f.set("top", v("t1"));
                        f.set("state", vset(v("state"), v("cf"), u64c(2)));
                    },
                );
            });
            f.if_(ne(vlen(v("reached")), v("nf")), |f| {
                f.ret_err(code(VM_LOWER_PROFILE_UNSUPPORTED));
            });
            // Imports: referenced, bootstrap rows (inventory order).
            f.var("imports", vnew(t::bytes(), vec![]));
            f.let_("imps", field(v("inv"), "imps"));
            f.for_u64("i", u64c(0), vlen(v("imps")), |f| {
                f.let_("imp", vget(v("imps"), v("i")));
                f.if_(
                    or(
                        not(mhas(v("refd"), field(v("imp"), "id"))),
                        not(call("jg_row_ok", vec![v("imp"), v("badb")])),
                    ),
                    |f| f.ret_err(code(VM_LOWER_OPCODE_UNSUPPORTED)),
                );
                f.set("imports", push(v("imports"), field(v("imp"), "id")));
            });
            // Constants: boundary membership and the Bytes bound.
            f.let_("csts", field(v("inv"), "csts"));
            f.for_u64("i", u64c(0), vlen(v("csts")), |f| {
                f.let_("c", vget(v("csts"), v("i")));
                f.let_("ty", field(v("c"), "ty"));
                f.if_(not(mhas(v("okb"), v("ty"))), |f| {
                    f.set(
                        "okb",
                        mins(v("okb"), v("ty"), call("jg_bt_ok", vec![v("ty"), boolc(true), v("badb")])),
                    );
                });
                f.if_(not(mget(v("okb"), v("ty"))), |f| {
                    f.ret_err(code(VM_LOWER_SIGNATURE_MISMATCH));
                });
                f.if_(call("jg_big_bytes", vec![field(v("c"), "value"), v("ty")]), |f| {
                    f.ret_err(code(VM_LOWER_RESOURCE_LIMIT));
                });
            });
            // Reached functions in order.
            f.for_u64("i", u64c(0), vlen(v("reached")), |f| {
                f.let_("fp", unwrap(find(vget(v("reached"), v("i")), KIND_FUNCTION)));
                f.if_(vget(v("bpbad"), v("fp")), |f| f.ret_err(code(VM_LOWER_SIGNATURE_MISMATCH)));
                f.let_("c", vget(v("first"), v("fp")));
                f.if_(ne(v("c"), u64c(0)), |f| f.ret_err(call("jg_code", vec![v("c")])));
                f.let_("g", vget(v("fns"), v("fp")));
                f.let_("ps", field(v("g"), "params"));
                f.for_u64("j", u64c(0), vlen(v("ps")), |f| {
                    f.match_opt(
                        call("inv_prm", vec![v("inv"), vget(v("ps"), v("j"))]),
                        "p",
                        |f| {
                            f.if_(
                                not(call("jg_bt_ok", vec![field(v("p"), "ty"), boolc(true), v("badb")])),
                                |f| f.ret_err(code(VM_LOWER_SIGNATURE_MISMATCH)),
                            );
                        },
                        |_| {},
                    );
                });
                f.if_(
                    not(call("jg_bt_ok", vec![field(v("g"), "result"), boolc(true), v("badb")])),
                    |f| f.ret_err(code(VM_LOWER_SIGNATURE_MISMATCH)),
                );
            });
            f.ret_ok(rec(
                GATE,
                vec![
                    v("reached"),
                    v("opn"),
                    v("brn"),
                    v("imports"),
                    vnew(t::bytes(), vec![]),
                ],
            ));
        },
    );
}

// ----- test entries -----

fn define_tests(m: &mut Module) {
    let load = |f: &mut FnB<'_>| {
        f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
    };
    // Judges each of `fids`.
    m.func(
        "t_jg_each",
        &[
            ("objects", t::vec(t::bytes())),
            ("epoch", t::bytes()),
            ("fids", t::vec(t::bytes())),
        ],
        r(t::vec(r(t::unit()))),
        move |f| {
            load(f);
            f.var("out", vnew(r(t::unit()), vec![]));
            f.for_u64("i", u64c(0), vlen(v("fids")), |f| {
                f.set(
                    "out",
                    push(v("out"), call("jg_function", vec![v("inv"), vget(v("fids"), v("i"))])),
                );
            });
            f.ret_ok(v("out"));
        },
    );
    // Judges every inventory function.
    m.func(
        "t_jg_all",
        &[("objects", t::vec(t::bytes())), ("epoch", t::bytes())],
        r(t::vec(r(t::unit()))),
        move |f| {
            load(f);
            f.var("out", vnew(r(t::unit()), vec![]));
            f.let_("fns", field(v("inv"), "fns"));
            f.for_u64("i", u64c(0), vlen(v("fns")), |f| {
                f.set(
                    "out",
                    push(
                        v("out"),
                        call("jg_function", vec![v("inv"), field(vget(v("fns"), v("i")), "id")]),
                    ),
                );
            });
            f.ret_ok(v("out"));
        },
    );
    // Only loads (instruction baseline).
    m.func(
        "t_jg_load",
        &[("objects", t::vec(t::bytes())), ("epoch", t::bytes())],
        r(t::u64()),
        move |f| {
            load(f);
            f.ret_ok(vlen(field(v("inv"), "fns")));
        },
    );
    // The gate: (functions, ops, bridges, imports).
    m.func(
        "t_jg_gate",
        &[
            ("objects", t::vec(t::bytes())),
            ("epoch", t::bytes()),
            ("entry", t::bytes()),
            ("image", t::bytes()),
        ],
        r(t::tuple(vec![t::vec(t::bytes()), t::u64(), t::u64(), t::vec(t::bytes())])),
        move |f| {
            load(f);
            f.let_("g", tri(call("jg_gate", vec![v("inv"), v("entry"), v("image")])));
            f.ret_ok(tup(vec![
                field(v("g"), "functions"),
                field(v("g"), "ops"),
                field(v("g"), "bridges"),
                field(v("g"), "imports"),
            ]));
        },
    );
    m.func("t_jg_big", &[("value", t::bytes()), ("ty", t::bytes())], t::bool(), |f| {
        f.ret(call("jg_big_bytes", vec![v("value"), v("ty")]));
    });
}
