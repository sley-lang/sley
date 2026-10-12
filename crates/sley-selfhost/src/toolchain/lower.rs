//! `lw_*`: SSMC-to-`SLEYBC02` lowering and image encoding.
//!
//! `lw_image(inv, entry)` returns exactly the bytes the native
//! `sley_vm::lower_function` returns under `CacheProfile::EXTENDED_V1` for
//! the entry and its transitive `call_direct` closure. The program is
//! assumed checked (CFG, types and operation judgment belong to other
//! modules); the refusals a checked program can still hit are replicated
//! with the native codes and in the native phase order: profile (26000),
//! entry preflight and register/work/size limits (26006), unknown callee
//! (26003). Resource limits are checked at phase boundaries (registers,
//! judgment, emission) rather than at every charge, so when several faults
//! coincide inside one phase the reported one can differ from native; a
//! malformed inventory reports 26004.
//!
//! Layout (big-endian; a list is a `u64` count): `"SLEYBC02" || u32(1) ||
//! body(entry) || u64(callees) || body(callee)*` with callees ascending by
//! function id. A body is `id || registers(function parameters) ||
//! u64(register count) || type* || type(result) || u32(entry slot) ||
//! u64(blocks) || block*`, a block `u32(slot) || registers(params) ||
//! u64(instructions) || (u32(opcode) || registers(operands) ||
//! registers(results) || immediate)* || terminator || u32(reachability)`.
//!
//! Registers: function parameters in order, then for every block in
//! `Function.blocks` order its parameters and each operation's results.
//! Types are converted from canonical SCB1 `TypeExpr` bytes to the
//! Appendix-A encoding once per distinct type (`tm` memo).
//!
//! Bodies are built as `Bytes` pieces and joined with `concat`; `u32`
//! encodings come from a dense table (`tbl`, index = value) that grows to
//! the largest register or block slot seen; small `u64` counts come from a
//! fixed 256-entry table.

use sley_ssmc::{ConstData, ConstValue};

use super::inv::{KIND_BLOCK, KIND_FUNCTION, KIND_OPERATION, KIND_PARAMETER};
use super::model::*;
use super::rt::{code, r};
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
/// `VM_LOWER_CACHE_KEY_UNSUPPORTED`.
pub const VM_LOWER_CACHE_KEY_UNSUPPORTED: u32 = 26_005;
/// `VM_LOWER_RESOURCE_LIMIT`.
pub const VM_LOWER_RESOURCE_LIMIT: u32 = 26_006;

/// Maximum registers in one lowered function.
pub const MAX_REGISTERS: u64 = 1_000_000;
/// Maximum blocks in the lowered entry.
pub const MAX_LOWERED_BLOCKS: u64 = 4_096;
/// Maximum instructions in the lowered entry.
pub const MAX_INSTRUCTIONS: u64 = 1_000_000;
/// Maximum operands or results on one entry instruction.
pub const MAX_INSTRUCTION_VALUES: u64 = 65_535;
/// Shared lowering work budget (entry plus callees).
pub const MAX_LOWERING_WORK: u64 = 100_000_000;
/// Maximum encoded image bytes.
pub const MAX_BYTECODE_BYTES: u64 = 67_108_864;
/// Maximum `TypeExpr` encoding depth.
pub const MAX_TYPE_DEPTH: u64 = 64;

/// Flush threshold for a piece vector (below the 2^20 push ceiling).
const FLUSH: u64 = 262_144;
const K32: u64 = 1 << 32;
const K33: u64 = 1 << 33;
/// Stack-depth marker of a raw (already encoded) piece in `lw_ty`.
const RAW: u64 = 1 << 32;

fn bytes_lit(value: Vec<u8>) -> ConstValue {
    ConstValue {
        value_type: t::bytes(),
        data: ConstData::Bytes(value),
    }
}

/// `Vector<Bytes>` literal of the big-endian encodings of 0..256.
fn table_lit(width: usize) -> E {
    E::Lit(ConstValue {
        value_type: t::vec(t::bytes()),
        data: ConstData::Sequence(
            (0..256_u64)
                .map(|i| bytes_lit(i.to_be_bytes()[8 - width..].to_vec()))
                .collect(),
        ),
    })
}

fn memo_ty() -> Ty {
    t::map(t::bytes(), t::tuple(vec![t::bytes(), t::u64()]))
}

fn fn_out_ty() -> Ty {
    t::tuple(vec![
        t::bytes(),
        t::vec(t::bytes()),
        t::u64(),
        t::u64(),
        memo_ty(),
        t::vec(t::bytes()),
    ])
}

/// `parts += piece`. The piece is bound first.
///
/// Rule used throughout this module: an operand list never places a large
/// variable before an operand that splits blocks (checked arithmetic,
/// `vget`/`mget` unwraps, bridges, `tri`, `select`). The seed keeps earlier
/// operands pending across the split and threads them as extra block
/// arguments, so the variable's value gets two holders and the next
/// consuming use (`push`, `mins`, a dead-operand `vget`) deep-copies it.
fn put(f: &mut FnB<'_>, parts: &str, piece: E) {
    f.let_("#piece", piece);
    f.set(parts, push(v(parts), v("#piece")));
}

/// `x += amount`.
fn bump(f: &mut FnB<'_>, var: &str, amount: E) {
    f.set(var, add(v(var), amount));
}

/// `u32` encoding of a value known to be below `vlen(tbl)`.
fn u32t(x: E) -> E {
    vget(v("tbl"), x)
}

/// `u32` encoding of any `u32` value (`x` must be a plain variable).
fn u32any(x: &str) -> E {
    select(
        lt(v(x), v("tl")),
        vget(v("tbl"), v(x)),
        call("be32", vec![v(x)]),
    )
}

/// `u64` encoding of a count (`x` must be a plain variable).
fn u64any(x: &str) -> E {
    select(
        lt(v(x), v("K256")),
        vget(v("t64"), v(x)),
        call("be64", vec![v(x)]),
    )
}

/// Pushes the converted type of `ty` onto `parts` and adds its length to
/// `size`, through the `tm` memo.
fn tyc(f: &mut FnB<'_>, ty: E, parts: &'static str, size: &'static str) {
    f.let_("#ty", ty);
    f.match_opt(
        mget_opt(v("tm"), v("#ty")),
        "#m",
        move |f| {
            put(f, parts, tget(v("#m"), 0));
            bump(f, size, tget(v("#m"), 1));
        },
        move |f| {
            f.let_("#cv", tri(call("lw_ty", vec![v("#ty")])));
            f.set("tm", mins(v("tm"), v("#ty"), v("#cv")));
            put(f, parts, tget(v("#cv"), 0));
            bump(f, size, tget(v("#cv"), 1));
        },
    );
}

/// Sets the existing variable `out` to the register of the `VRef` `vr`
/// (`rmap` value: `register * 2^33 + is_parameter * 2^32 + result count`).
fn reg(f: &mut FnB<'_>, out: &'static str, vr: E) {
    f.let_("#vr", vr);
    f.match_opt(
        mget_opt(v("rmap"), field(v("#vr"), "id")),
        "#c",
        move |f| {
            f.let_("#hi", div(v("#c"), v("K32")));
            f.let_("#idx", field(v("#vr"), "idx"));
            f.if_(
                or(
                    ne(rem(v("#hi"), v("K2")), sub(v("K2"), field(v("#vr"), "kind"))),
                    ge(v("#idx"), rem(v("#c"), v("K32"))),
                ),
                |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
            );
            if crate::seed::fault("lower-register") {
                f.set(out, add(add(div(v("#hi"), v("K2")), v("#idx")), u64c(1)));
            } else {
                f.set(out, add(div(v("#hi"), v("K2")), v("#idx")));
            }
        },
        |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
    );
}

/// Sets `out` to the block slot of `id`.
fn slot(f: &mut FnB<'_>, out: &'static str, id: E) {
    f.match_opt(
        mget_opt(v("smap"), id),
        "#s",
        move |f| f.set(out, v("#s")),
        |f| f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID)),
    );
}

/// Binds `out` to the entity `id` of index kind `kind` from the kind vector
/// `vec`, or returns `Err(fail)`.
///
/// Lookups go through `idx` (the inventory index) and kind vectors bound
/// once by `lw_image` and passed down: `inv_*` passes `field(inv, kind)` to
/// `vector_get` as a dead temporary, and the VM's owned `vector_get`
/// materializes a dead shared view (`into_value` on a shared root clones the
/// whole vector), while `record_get` on a live record walks the selected
/// field for its value units; either way each `inv_*` lookup costs O(|S|).
fn find(f: &mut FnB<'_>, out: &str, vec: &str, kind: u64, id: E, fail: u32) {
    f.let_("#k", mget_opt(v("idx"), id));
    f.if_(not(is_some(v("#k"))), move |f| f.ret_err(code(fail)));
    f.let_("#kc", unwrap(v("#k")));
    f.if_(ne(div(v("#kc"), u64c(K32)), u64c(kind)), move |f| f.ret_err(code(fail)));
    f.let_("#kp", rem(v("#kc"), u64c(K32)));
    f.let_(out, vget(v(vec), v("#kp")));
}

/// Charges `work` and refuses beyond the budget.
fn charge(f: &mut FnB<'_>, amount: E) {
    bump(f, "work", amount);
    f.if_(gt(v("work"), u64c(MAX_LOWERING_WORK)), |f| {
        f.ret_err(code(VM_LOWER_RESOURCE_LIMIT));
    });
}

/// Encodes one ordinary target edge (`Edge` record).
fn edge(f: &mut FnB<'_>, e: E) {
    f.let_("#e", e);
    slot(f, "x", field(v("#e"), "target"));
    put(f, "parts", u32t(v("x")));
    f.let_("#args", field(v("#e"), "args"));
    f.let_("#n", vlen(v("#args")));
    put(f, "parts", u64any("#n"));
    f.for_u64("#a", u64c(0), v("#n"), |f| {
        reg(f, "x", vget(v("#args"), v("#a")));
        put(f, "parts", u32t(v("x")));
    });
    bump(f, "work", v("#n"));
    bump(f, "sz", add(u64c(12), mul(u64c(4), v("#n"))));
}

/// Defines the `lw_*` functions.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    define_ty(m);
    define_sort(m);
    define_fn(m);
    let inv_ty = m.ty(super::inv::INV);
    // The complete SLEYBC02 image of `entry` and its call closure.
    m.func(
        "lw_image",
        &[("inv", inv_ty.clone()), ("entry", t::bytes())],
        r(t::bytes()),
        |f| {
            // Views of the inventory, taken once: the VM's `record_get` on a
            // live record walks the selected field for its value units, so
            // `field(inv, ..)` costs O(|S|) per access.
            f.let_("idx", field(v("inv"), "index"));
            f.let_("ifns", field(v("inv"), "fns"));
            f.let_("iprms", field(v("inv"), "prms"));
            f.let_("iblks", field(v("inv"), "blks"));
            f.let_("iops", field(v("inv"), "ops"));
            find(f, "efun", "ifns", KIND_FUNCTION, v("entry"), VM_LOWER_LOCAL_REFERENCE_INVALID);
            f.let_(
                "res",
                tri(call(
                    "lw_fn",
                    vec![
                        v("idx"),
                        v("iprms"),
                        v("iblks"),
                        v("iops"),
                        v("efun"),
                        boolc(true),
                        u64c(0),
                        mnew(t::bytes(), t::tuple(vec![t::bytes(), t::u64()]), vec![]),
                        table_lit(4),
                    ],
                )),
            );
            f.let_("ebody", tget(v("res"), 0));
            f.var("st", tget(v("res"), 1));
            f.var("work", tget(v("res"), 2));
            f.var("total", add(u64c(20), tget(v("res"), 3)));
            f.var("tm", tget(v("res"), 4));
            f.var("tbl", tget(v("res"), 5));
            f.var("sp", vlen(v("st")));
            f.var("done", mnew(t::bytes(), t::bytes(), vec![]));
            f.var("ids", vnew(t::bytes(), vec![]));
            // Pending callees: a stack popped from the end, as native.
            f.while_(gt(v("sp"), u64c(0)), |f| {
                f.set("sp", sub(v("sp"), u64c(1)));
                f.let_("fid", vget(v("st"), v("sp")));
                f.if_(
                    and(ne(v("fid"), v("entry")), not(mhas(v("done"), v("fid")))),
                    |f| {
                        find(f, "fun", "ifns", KIND_FUNCTION, v("fid"), VM_LOWER_IMMEDIATE_MISMATCH);
                        f.let_(
                            "cr",
                            tri(call(
                                "lw_fn",
                                vec![
                                    v("idx"),
                                    v("iprms"),
                                    v("iblks"),
                                    v("iops"),
                                    v("fun"),
                                    boolc(false),
                                    v("work"),
                                    v("tm"),
                                    v("tbl"),
                                ],
                            )),
                        );
                        f.set("done", mins(v("done"), v("fid"), tget(v("cr"), 0)));
                        f.set("ids", push(v("ids"), v("fid")));
                        f.set("work", tget(v("cr"), 2));
                        f.set("total", add(v("total"), tget(v("cr"), 3)));
                        f.let_("calls", tget(v("cr"), 1));
                        f.set("tm", tget(v("cr"), 4));
                        f.set("tbl", tget(v("cr"), 5));
                        f.for_u64("i", u64c(0), vlen(v("calls")), |f| {
                            f.let_("c", vget(v("calls"), v("i")));
                            f.if_else(
                                lt(v("sp"), vlen(v("st"))),
                                |f| f.set("st", vset(v("st"), v("sp"), v("c"))),
                                |f| f.set("st", push(v("st"), v("c"))),
                            );
                            f.set("sp", add(v("sp"), u64c(1)));
                        });
                    },
                );
            });
            f.if_(gt(v("total"), u64c(MAX_BYTECODE_BYTES)), |f| {
                f.ret_err(code(VM_LOWER_RESOURCE_LIMIT));
            });
            f.let_("sorted", call("lw_sort", vec![v("ids")]));
            f.var(
                "out",
                vnew(
                    t::bytes(),
                    vec![
                        bytesc(b"SLEYBC02"),
                        bytesc(&1_u32.to_be_bytes()),
                        v("ebody"),
                        call("be64", vec![vlen(v("sorted"))]),
                    ],
                ),
            );
            f.for_u64("i", u64c(0), vlen(v("sorted")), |f| {
                f.let_("id", vget(v("sorted"), v("i")));
                put(f, "out", mget(v("done"), v("id")));
            });
            f.ret_ok(concat(v("out")));
        },
    );
    // Test entry: load S, then lower `entry`.
    m.func(
        "t_lw_image",
        &[
            ("objects", t::vec(t::bytes())),
            ("epoch", t::bytes()),
            ("entry", t::bytes()),
        ],
        r(t::bytes()),
        |f| {
            f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
            f.ret(call("lw_image", vec![v("inv"), v("entry")]));
        },
    );
    // Test entry: one type conversion.
    m.func(
        "t_lw_ty",
        &[("ty", t::bytes())],
        r(t::tuple(vec![t::bytes(), t::u64()])),
        |f| f.ret(call("lw_ty", vec![v("ty")])),
    );
}

/// `lw_ty(ty) -> Result<(Bytes, UInt(64)), UInt(32)>`: the Appendix-A
/// encoding of canonical `TypeExpr` bytes and its length, at root depth 1;
/// a node deeper than 64 refuses with `VM_LOWER_RESOURCE_LIMIT`.
#[allow(clippy::too_many_lines)]
fn define_ty(m: &mut Module) {
    m.func(
        "lw_ty",
        &[("ty", t::bytes())],
        r(t::tuple(vec![t::bytes(), t::u64()])),
        |f| {
            // Work stack: (bytes, depth); depth RAW + n marks an encoded
            // piece of n bytes.
            f.var("st", vnew(t::bytes(), vec![v("ty")]));
            f.var("sd", vnew(t::u64(), vec![u64c(1)]));
            f.var("sp", u64c(1));
            f.var("out", vnew(t::bytes(), vec![]));
            f.var("n", u64c(0));
            let spush = |f: &mut FnB<'_>, x: E, d: E| {
                f.let_("#x", x);
                f.let_("#d", d);
                f.if_else(
                    lt(v("sp"), vlen(v("st"))),
                    |f| {
                        f.set("st", vset(v("st"), v("sp"), v("#x")));
                        f.set("sd", vset(v("sd"), v("sp"), v("#d")));
                    },
                    |f| {
                        f.set("st", push(v("st"), v("#x")));
                        f.set("sd", push(v("sd"), v("#d")));
                    },
                );
                f.set("sp", add(v("sp"), u64c(1)));
            };
            let emit = |f: &mut FnB<'_>, piece: E, len: E| {
                put(f, "out", piece);
                bump(f, "n", len);
            };
            f.while_(gt(v("sp"), u64c(0)), move |f| {
                f.set("sp", sub(v("sp"), u64c(1)));
                f.let_("x", vget(v("st"), v("sp")));
                f.let_("d", vget(v("sd"), v("sp")));
                f.if_else(
                    ge(v("d"), u64c(RAW)),
                    move |f| emit(f, v("x"), sub(v("d"), u64c(RAW))),
                    move |f| {
                        f.if_(gt(v("d"), u64c(MAX_TYPE_DEPTH)), |f| {
                            f.ret_err(code(VM_LOWER_RESOURCE_LIMIT));
                        });
                        f.let_("d1", add(v("d"), u64c(1)));
                        f.let_("parts", call("ty_parts", vec![v("x")]));
                        f.let_("tag", tget(v("parts"), 0));
                        f.let_("pl", tget(v("parts"), 1));
                        emit(f, call("be32", vec![v("tag")]), u64c(4));
                        // SInt / UInt: u16 width.
                        f.if_(or(eq(v("tag"), u64c(3)), eq(v("tag"), u64c(4))), move |f| {
                            emit(
                                f,
                                slice(
                                    call("be32", vec![call("ty_uvar_payload", vec![v("pl")])]),
                                    u64c(2),
                                    u64c(2),
                                ),
                                u64c(2),
                            );
                        });
                        // Tuple: u64 count then items.
                        f.if_(eq(v("tag"), u64c(9)), move |f| {
                            f.let_("items", call("ty_list", vec![v("pl")]));
                            f.let_("k", vlen(v("items")));
                            emit(f, call("be64", vec![v("k")]), u64c(8));
                            f.for_u64("i", u64c(0), v("k"), move |f| {
                                f.let_("j", sub(sub(v("k"), v("i")), u64c(1)));
                                spush(f, vget(v("items"), v("j")), v("d1"));
                            });
                        });
                        // Named: entity(definition), u64 count, arguments.
                        f.if_(eq(v("tag"), u64c(10)), move |f| {
                            f.let_("fs", call("ty_fields", vec![v("pl")]));
                            f.let_("args", call("ty_list", vec![vget(v("fs"), u64c(1))]));
                            f.let_("k", vlen(v("args")));
                            emit(f, bytesc(&1_u32.to_be_bytes()), u64c(4));
                            emit(f, vget(v("fs"), u64c(0)), u64c(32));
                            emit(f, call("be64", vec![v("k")]), u64c(8));
                            f.for_u64("i", u64c(0), v("k"), move |f| {
                                f.let_("j", sub(sub(v("k"), v("i")), u64c(1)));
                                spush(f, vget(v("args"), v("j")), v("d1"));
                            });
                        });
                        // Vector / Option / LocalCell: the element.
                        f.if_(
                            or(
                                or(eq(v("tag"), u64c(11)), eq(v("tag"), u64c(13))),
                                eq(v("tag"), u64c(18)),
                            ),
                            move |f| spush(f, v("pl"), v("d1")),
                        );
                        // OrderedMap / Result: two fields in order.
                        f.if_(or(eq(v("tag"), u64c(12)), eq(v("tag"), u64c(14))), move |f| {
                            f.let_("fs", call("ty_fields", vec![v("pl")]));
                            spush(f, vget(v("fs"), u64c(1)), v("d1"));
                            spush(f, vget(v("fs"), u64c(0)), v("d1"));
                        });
                        // FunctionRef: parameters, result, effect entities.
                        f.if_(eq(v("tag"), u64c(15)), move |f| {
                            f.let_("fs", call("ty_fields", vec![v("pl")]));
                            f.let_("ps", call("ty_list", vec![vget(v("fs"), u64c(0))]));
                            f.let_("es", call("ty_list", vec![vget(v("fs"), u64c(2))]));
                            f.let_("k", vlen(v("ps")));
                            f.let_("ke", vlen(v("es")));
                            emit(f, call("be64", vec![v("k")]), u64c(8));
                            f.var("tail", vnew(t::bytes(), vec![call("be64", vec![v("ke")])]));
                            f.for_u64("i", u64c(0), v("ke"), |f| {
                                put(f, "tail", bytesc(&1_u32.to_be_bytes()));
                                put(f, "tail", vget(v("es"), v("i")));
                            });
                            spush(
                                f,
                                concat(v("tail")),
                                add(u64c(RAW + 8), mul(u64c(36), v("ke"))),
                            );
                            spush(f, vget(v("fs"), u64c(1)), v("d1"));
                            f.for_u64("i", u64c(0), v("k"), move |f| {
                                f.let_("j", sub(sub(v("k"), v("i")), u64c(1)));
                                spush(f, vget(v("ps"), v("j")), v("d1"));
                            });
                        });
                        // AdapterHandle / CapabilityToken: entity.
                        f.if_(or(eq(v("tag"), u64c(16)), eq(v("tag"), u64c(17))), move |f| {
                            emit(f, bytesc(&1_u32.to_be_bytes()), u64c(4));
                            emit(f, v("pl"), u64c(32));
                        });
                        // TypeParameter: u32.
                        f.if_(eq(v("tag"), u64c(19)), move |f| {
                            emit(
                                f,
                                call("be32", vec![call("ty_uvar_payload", vec![v("pl")])]),
                                u64c(4),
                            );
                        });
                        // BuiltinFailure: u16 kind.
                        f.if_(eq(v("tag"), u64c(20)), move |f| {
                            emit(
                                f,
                                slice(
                                    call("be32", vec![call("ty_uvar_payload", vec![v("pl")])]),
                                    u64c(2),
                                    u64c(2),
                                ),
                                u64c(2),
                            );
                        });
                    },
                );
            });
            f.ret_ok(tup(vec![concat(v("out")), v("n")]));
        },
    );
}

/// `lw_sort(xs: Vector<Bytes>) -> Vector<Bytes>`: ascending bottom-up merge
/// sort (no recursion).
fn define_sort(m: &mut Module) {
    m.func("lw_sort", &[("xs", t::vec(t::bytes()))], t::vec(t::bytes()), |f| {
        f.let_("n", vlen(v("xs")));
        f.var("src", v("xs"));
        f.var("w", u64c(1));
        f.while_(lt(v("w"), v("n")), |f| {
            f.var("dst", vnew(t::bytes(), vec![]));
            f.var("i", u64c(0));
            f.while_(lt(v("i"), v("n")), |f| {
                f.let_("mid", select(lt(add(v("i"), v("w")), v("n")), add(v("i"), v("w")), v("n")));
                f.let_(
                    "hi",
                    select(
                        lt(add(v("mid"), v("w")), v("n")),
                        add(v("mid"), v("w")),
                        v("n"),
                    ),
                );
                f.var("a", v("i"));
                f.var("b", v("mid"));
                f.while_(or(lt(v("a"), v("mid")), lt(v("b"), v("hi"))), |f| {
                    f.var("take_a", lt(v("a"), v("mid")));
                    f.if_(and(v("take_a"), lt(v("b"), v("hi"))), |f| {
                        f.set("take_a", le(vget(v("src"), v("a")), vget(v("src"), v("b"))));
                    });
                    f.if_else(
                        v("take_a"),
                        |f| {
                            put(f, "dst", vget(v("src"), v("a")));
                            f.set("a", add(v("a"), u64c(1)));
                        },
                        |f| {
                            put(f, "dst", vget(v("src"), v("b")));
                            f.set("b", add(v("b"), u64c(1)));
                        },
                    );
                });
                f.set("i", v("hi"));
            });
            f.set("src", v("dst"));
            f.set("w", mul(v("w"), u64c(2)));
        });
        f.ret(v("src"));
    });
}

/// `lw_fn(idx, iprms, iblks, iops, fun, is_entry, work, tm, tbl)` (the
/// inventory index and kind vectors): lowers and encodes one
/// function body. Returns `(body, call_direct targets in instruction order,
/// work, body size, tm, tbl)`.
#[allow(clippy::too_many_lines)]
fn define_fn(m: &mut Module) {
    let blk_ty = m.ty(BLK);
    let op_ty = m.ty(OP);
    let fun_ty = m.ty(FUN);
    m.func(
        "lw_fn",
        &[
            ("idx", t::map(t::bytes(), t::u64())),
            ("iprms", t::vec(m.ty(PRM))),
            ("iblks", t::vec(blk_ty.clone())),
            ("iops", t::vec(op_ty.clone())),
            ("fun", fun_ty),
            ("is_entry", t::bool()),
            ("work", t::u64()),
            ("tm", memo_ty()),
            ("tbl", t::vec(t::bytes())),
        ],
        r(fn_out_ty()),
        move |f| {
            f.let_("K32", u64c(K32));
            f.let_("K2", u64c(2));
            f.let_("K256", u64c(256));
            f.let_("t64", table_lit(8));
            f.let_("fid", field(v("fun"), "id"));
            f.if_(
                or(
                    or(
                        gt(vlen(field(v("fun"), "tparams")), u64c(0)),
                        gt(vlen(field(v("fun"), "effects")), u64c(0)),
                    ),
                    gt(vlen(field(v("fun"), "contracts")), u64c(0)),
                ),
                |f| f.ret_err(code(VM_LOWER_PROFILE_UNSUPPORTED)),
            );
            f.var("work", v("work"));
            f.var("tm", v("tm"));
            f.var("tbl", v("tbl"));

            // ----- registers (Maps::build) -----
            f.var("rmap", mnew(t::bytes(), t::u64(), vec![]));
            f.var("smap", mnew(t::bytes(), t::u64(), vec![]));
            f.var("tparts", vnew(t::bytes(), vec![]));
            f.var("tsz", u64c(0));
            f.var("r", u64c(0));
            f.var("nprm", u64c(0));
            f.var("nopnd", u64c(0));
            f.var("big", boolc(false));
            f.var("blks", vnew(blk_ty.clone(), vec![]));
            f.var("ops", vnew(op_ty.clone(), vec![]));
            // One parameter owned by `owner`.
            let param = |f: &mut FnB<'_>, pid: E, owner: E| {
                f.let_("pid", pid);
                find(f, "prm", "iprms", KIND_PARAMETER, v("pid"), VM_LOWER_LOCAL_REFERENCE_INVALID);
                f.if_(ne(field(v("prm"), "owner"), owner), |f| {
                    f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID));
                });
                tyc(f, field(v("prm"), "ty"), "tparts", "tsz");
                f.let_("rv", add(mul(v("r"), u64c(K33)), u64c(K32 + 1)));
                f.set("rmap", mins(v("rmap"), v("pid"), v("rv")));
                f.set("r", add(v("r"), u64c(1)));
                f.set("nprm", add(v("nprm"), u64c(1)));
            };
            f.let_("fps", field(v("fun"), "params"));
            f.let_("np", vlen(v("fps")));
            f.for_u64("i", u64c(0), v("np"), move |f| {
                param(f, vget(v("fps"), v("i")), v("fid"));
            });
            f.let_("fbs", field(v("fun"), "blocks"));
            f.let_("nb", vlen(v("fbs")));
            f.for_u64("b", u64c(0), v("nb"), move |f| {
                f.let_("bid", vget(v("fbs"), v("b")));
                find(f, "blk", "iblks", KIND_BLOCK, v("bid"), VM_LOWER_LOCAL_REFERENCE_INVALID);
                f.if_(ne(field(v("blk"), "function"), v("fid")), |f| {
                    f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID));
                });
                f.set("smap", mins(v("smap"), v("bid"), v("b")));
                f.let_("bps", field(v("blk"), "params"));
                f.for_u64("j", u64c(0), vlen(v("bps")), move |f| {
                    param(f, vget(v("bps"), v("j")), v("bid"));
                });
                f.let_("bos", field(v("blk"), "ops"));
                f.for_u64("j", u64c(0), vlen(v("bos")), |f| {
                    f.let_("oid", vget(v("bos"), v("j")));
                    find(f, "op", "iops", KIND_OPERATION, v("oid"), VM_LOWER_LOCAL_REFERENCE_INVALID);
                    f.if_(ne(field(v("op"), "block"), v("bid")), |f| {
                        f.ret_err(code(VM_LOWER_LOCAL_REFERENCE_INVALID));
                    });
                    f.let_("res", field(v("op"), "results"));
                    f.let_("nres", vlen(v("res")));
                    f.let_("nop", vlen(field(v("op"), "operands")));
                    f.if_(
                        or(
                            gt(v("nres"), u64c(MAX_INSTRUCTION_VALUES)),
                            gt(v("nop"), u64c(MAX_INSTRUCTION_VALUES)),
                        ),
                        |f| f.set("big", boolc(true)),
                    );
                    bump(f, "nopnd", v("nop"));
                    f.let_("rv", add(mul(v("r"), u64c(K33)), v("nres")));
                    f.set("rmap", mins(v("rmap"), v("oid"), v("rv")));
                    f.for_u64("q", u64c(0), v("nres"), |f| {
                        tyc(f, vget(v("res"), v("q")), "tparts", "tsz");
                    });
                    bump(f, "r", v("nres"));
                    put(f, "ops", v("op"));
                });
                put(f, "blks", v("blk"));
            });
            f.let_("nops", vlen(v("ops")));
            // Entry preflight, then register and judgment charges.
            f.if_else(
                v("is_entry"),
                |f| {
                    f.if_(
                        or(
                            or(
                                gt(v("nb"), u64c(MAX_LOWERED_BLOCKS)),
                                gt(v("nops"), u64c(MAX_INSTRUCTIONS)),
                            ),
                            or(v("big"), gt(v("r"), u64c(MAX_REGISTERS))),
                        ),
                        |f| f.ret_err(code(VM_LOWER_RESOURCE_LIMIT)),
                    );
                    charge(
                        f,
                        add(add(add(u64c(1), v("nprm")), v("nb")), v("nops")),
                    );
                },
                |f| {
                    f.if_(gt(v("r"), u64c(MAX_REGISTERS)), |f| {
                        f.ret_err(code(VM_LOWER_RESOURCE_LIMIT));
                    });
                },
            );
            charge(f, v("r"));
            charge(f, add(v("nops"), v("nopnd")));

            // Grow the u32 table to every register and slot.
            f.let_(
                "need",
                select(gt(v("r"), v("nb")), v("r"), v("nb")),
            );
            f.while_(lt(vlen(v("tbl")), v("need")), |f| {
                f.let_("i", vlen(v("tbl")));
                f.let_("hi", div(v("i"), u64c(256)));
                f.let_("lo", rem(v("i"), u64c(256)));
                put(
                    f,
                    "tbl",
                    concat(vnew(
                        t::bytes(),
                        vec![
                            slice(vget(v("tbl"), v("hi")), u64c(1), u64c(3)),
                            slice(vget(v("tbl"), v("lo")), u64c(3), u64c(1)),
                        ],
                    )),
                );
            });
            f.let_("tl", vlen(v("tbl")));

            // ----- blocks (emit_function) -----
            f.var("parts", vnew(t::bytes(), vec![]));
            f.var("chunks", vnew(t::bytes(), vec![]));
            f.var("sz", u64c(0));
            f.var("calls", vnew(t::bytes(), vec![]));
            f.var("k", u64c(0));
            f.var("x", u64c(0));
            f.var("rr", v("np"));
            f.for_u64("b", u64c(0), v("nb"), |f| {
                f.let_("blk", vget(v("blks"), v("b")));
                put(f, "parts", u32t(v("b")));
                f.let_("nbp", vlen(field(v("blk"), "params")));
                put(f, "parts", u64any("nbp"));
                f.for_u64("j", u64c(0), v("nbp"), |f| {
                    f.let_("rj", add(v("rr"), v("j")));
                    put(f, "parts", u32t(v("rj")));
                });
                bump(f, "rr", v("nbp"));
                f.let_("nbo", vlen(field(v("blk"), "ops")));
                put(f, "parts", u64any("nbo"));
                bump(f, "sz", add(u64c(4 + 8 + 8 + 4), mul(u64c(4), v("nbp"))));
                f.for_u64("j", u64c(0), v("nbo"), |f| {
                    f.let_("op", vget(v("ops"), v("k")));
                    f.set("k", add(v("k"), u64c(1)));
                    f.let_("opc", field(v("op"), "opcode"));
                    put(f, "parts", u32any("opc"));
                    f.let_("opnds", field(v("op"), "operands"));
                    f.let_("n", vlen(v("opnds")));
                    put(f, "parts", u64any("n"));
                    f.for_u64("q", u64c(0), v("n"), |f| {
                        reg(f, "x", vget(v("opnds"), v("q")));
                        put(f, "parts", u32t(v("x")));
                    });
                    f.let_("nres", vlen(field(v("op"), "results")));
                    put(f, "parts", u64any("nres"));
                    f.for_u64("q", u64c(0), v("nres"), |f| {
                        f.let_("rq", add(v("rr"), v("q")));
                        put(f, "parts", u32t(v("rq")));
                    });
                    bump(f, "rr", v("nres"));
                    bump(
                        f,
                        "sz",
                        add(u64c(4 + 8 + 8 + 4), mul(u64c(4), add(v("n"), v("nres")))),
                    );
                    bump(f, "work", v("n"));
                    // Immediate.
                    f.let_("imm", field(v("op"), "imm"));
                    f.let_("itag", field(v("imm"), "tag"));
                    put(f, "parts", u32t(v("itag")));
                    f.if_(gt(v("itag"), u64c(1)), |f| {
                        f.if_(eq(v("itag"), u64c(2)), |f| {
                            put(f, "parts", u32t(u64c(1)));
                            put(f, "parts", field(v("imm"), "id"));
                            bump(f, "sz", u64c(36));
                        });
                        f.if_(eq(v("itag"), u64c(3)), |f| {
                            f.let_("ix", field(v("imm"), "index"));
                            put(f, "parts", u32any("ix"));
                            bump(f, "sz", u64c(4));
                        });
                        f.if_(or(eq(v("itag"), u64c(4)), eq(v("itag"), u64c(6))), |f| {
                            put(f, "parts", field(v("imm"), "id"));
                            bump(f, "sz", u64c(32));
                        });
                        f.if_(eq(v("itag"), u64c(5)), |f| {
                            put(f, "parts", u32t(u64c(1)));
                            put(f, "parts", field(v("imm"), "id"));
                            put(f, "parts", field(v("imm"), "member"));
                            bump(f, "sz", u64c(68));
                        });
                        f.if_(eq(v("itag"), u64c(7)), |f| {
                            put(f, "parts", u32t(u64c(1)));
                            put(f, "parts", field(v("imm"), "id"));
                            f.let_("targs", field(v("imm"), "targs"));
                            f.let_("nt", vlen(v("targs")));
                            put(f, "parts", u64any("nt"));
                            bump(f, "sz", u64c(44));
                            f.for_u64("q", u64c(0), v("nt"), |f| {
                                tyc(f, vget(v("targs"), v("q")), "parts", "sz");
                            });
                            f.if_(eq(v("opc"), u64c(112)), |f| {
                                put(f, "calls", field(v("imm"), "id"));
                            });
                        });
                    });
                    f.if_(ge(vlen(v("parts")), u64c(FLUSH)), |f| {
                        f.if_(gt(v("sz"), u64c(MAX_BYTECODE_BYTES)), |f| {
                            f.ret_err(code(VM_LOWER_RESOURCE_LIMIT));
                        });
                        put(f, "chunks", concat(v("parts")));
                        f.set("parts", vnew(t::bytes(), vec![]));
                    });
                });
                // Terminator.
                f.let_("tr", field(v("blk"), "term"));
                f.let_("tk", field(v("tr"), "kind"));
                put(f, "parts", u32t(v("tk")));
                bump(f, "work", u64c(1));
                bump(f, "sz", u64c(4));
                f.if_(eq(v("tk"), u64c(1)), |f| {
                    reg(f, "x", field(v("tr"), "val"));
                    put(f, "parts", u32t(v("x")));
                    bump(f, "sz", u64c(4));
                });
                f.if_(eq(v("tk"), u64c(2)), |f| {
                    edge(f, vget(field(v("tr"), "edges"), u64c(0)));
                });
                f.if_(eq(v("tk"), u64c(3)), |f| {
                    reg(f, "x", field(v("tr"), "val"));
                    put(f, "parts", u32t(v("x")));
                    bump(f, "sz", u64c(4));
                    edge(f, vget(field(v("tr"), "edges"), u64c(0)));
                    edge(f, vget(field(v("tr"), "edges"), u64c(1)));
                });
                f.if_(eq(v("tk"), u64c(4)), |f| {
                    reg(f, "x", field(v("tr"), "val"));
                    put(f, "parts", u32t(v("x")));
                    f.let_("cases", field(v("tr"), "cases"));
                    f.let_("nc", vlen(v("cases")));
                    put(f, "parts", u64any("nc"));
                    bump(f, "sz", u64c(12));
                    f.for_u64("c", u64c(0), v("nc"), |f| {
                        f.let_("cs", vget(v("cases"), v("c")));
                        f.if_else(
                            field(v("cs"), "member"),
                            |f| {
                                put(f, "parts", u32t(u64c(1)));
                                put(f, "parts", field(v("cs"), "key"));
                                bump(f, "sz", u64c(36));
                            },
                            |f| {
                                put(f, "parts", u32t(u64c(2)));
                                put(f, "parts", u32t(field(v("cs"), "builtin")));
                                bump(f, "sz", u64c(8));
                            },
                        );
                        slot(f, "x", field(v("cs"), "target"));
                        put(f, "parts", u32t(v("x")));
                        f.let_("sargs", field(v("cs"), "args"));
                        f.let_("na", vlen(v("sargs")));
                        put(f, "parts", u64any("na"));
                        bump(f, "sz", u64c(12));
                        bump(f, "work", add(u64c(1), v("na")));
                        f.for_u64("a", u64c(0), v("na"), |f| {
                            f.let_("sa", vget(v("sargs"), v("a")));
                            f.if_else(
                                field(v("sa"), "pay"),
                                |f| {
                                    put(f, "parts", u32t(u64c(2)));
                                    bump(f, "sz", u64c(4));
                                },
                                |f| {
                                    reg(f, "x", field(v("sa"), "val"));
                                    put(f, "parts", u32t(u64c(1)));
                                    put(f, "parts", u32t(v("x")));
                                    bump(f, "sz", u64c(8));
                                },
                            );
                        });
                    });
                });
                f.if_(eq(v("tk"), u64c(5)), |f| {
                    put(f, "parts", u32t(field(v("tr"), "code")));
                    f.if_else(
                        field(v("tr"), "has_val"),
                        |f| {
                            reg(f, "x", field(v("tr"), "val"));
                            put(f, "parts", u32t(u64c(2)));
                            put(f, "parts", u32t(v("x")));
                            bump(f, "sz", u64c(12));
                        },
                        |f| {
                            put(f, "parts", u32t(u64c(1)));
                            bump(f, "sz", u64c(8));
                        },
                    );
                });
                put(f, "parts", u32t(field(v("blk"), "reach")));
            });
            f.if_(gt(v("work"), u64c(MAX_LOWERING_WORK)), |f| {
                f.ret_err(code(VM_LOWER_RESOURCE_LIMIT));
            });

            // ----- header -----
            f.var("head", vnew(t::bytes(), vec![v("fid")]));
            put(f, "head", u64any("np"));
            f.for_u64("i", u64c(0), v("np"), |f| put(f, "head", u32t(v("i"))));
            put(f, "head", call("be64", vec![v("r")]));
            put(f, "head", concat(v("tparts")));
            tyc(f, field(v("fun"), "result"), "head", "tsz");
            slot(f, "x", field(v("fun"), "entry"));
            put(f, "head", u32t(v("x")));
            put(f, "head", u64any("nb"));
            f.let_(
                "size",
                add(
                    add(u64c(32 + 8 + 8 + 4 + 8), mul(u64c(4), v("np"))),
                    add(v("tsz"), v("sz")),
                ),
            );
            f.if_(gt(v("size"), u64c(MAX_BYTECODE_BYTES)), |f| {
                f.ret_err(code(VM_LOWER_RESOURCE_LIMIT));
            });
            f.for_u64("i", u64c(0), vlen(v("chunks")), |f| {
                put(f, "head", vget(v("chunks"), v("i")));
            });
            put(f, "head", concat(v("parts")));
            f.ret_ok(tup(vec![
                concat(v("head")),
                v("calls"),
                v("work"),
                v("size"),
                v("tm"),
                v("tbl"),
            ]));
        },
    );
}
