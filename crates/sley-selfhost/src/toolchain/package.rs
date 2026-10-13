//! `pk_*`: `EXEC_PACKAGE_V3` sections, digests, and envelope.
//!
//! Byte-for-byte `sley_vm::encode_package_envelope_v3` for the package
//! `{ image, constants: inv.csts, type_definitions: inv.tdefs, imports:
//! inv.imps (all in inventory order), globals: [], contracts: [], entry,
//! epoch, root, profile: EXTENDED_V1, admitted_limits: lim, gate counts and
//! fingerprints }`.
//!
//! Sections keep their rows in the given order (no sorting):
//!
//! - constants: `u64(n)`, then per row `id || u64(len) || ConstValue` where
//!   the value is its canonical SCB1 encoding (the model's `Cst.value`
//!   bytes, sliced, never re-encoded);
//! - layouts: `u64(n)`, then `u64(len) || row` per type definition, in the
//!   package structural encoding (`TypeExpr` as in `fp_ty`);
//! - imports: `u64(n)`, then `u64(len) || row` per adapter import;
//! - dependency: entry, epoch, root, the `EXTENDED_V1` profile words, gate
//!   counts and fingerprints, limits, empty globals and contracts.
//!
//! The 316-byte header binds the SHA-256 section digests; the envelope is
//! `header || (u64(len) || section)` for image, constants, layouts,
//! imports, dependency.
//!
//! `pk_envelope` measures the image with `rt_blen` (the `BLN1` length
//! primitive, any size); `pk_envelope_sized` and `pk_package_digest` take
//! the length from the caller.

use super::fingerprint::{keep, lit32, memo_ty, put, tyc};
use super::inv::INV;
use super::model::{GATE, LIM};
use super::rt::{code, r};
use crate::seed::{
    E, FnB, Module, Ty, add, b2v, blake3_large, bytesc, call, concat, field, gt, lt, mnew, ne, or,
    push, rec, rem, sha256, shl, slice, sub, t, tget, tri, tup, u32c, u64c, v, vget, vlen, vnew,
    vset,
};

/// A section, the envelope, or the image exceeds its ceiling
/// (`PackageError::Oversized`).
pub const PK_OVERSIZED: u32 = 29_300;
/// A value the package layout cannot carry: a gate count above `u32`, or
/// an entry/epoch/root that is not 32 bytes (`PackageError::Malformed`).
pub const PK_MALFORMED: u32 = 29_301;

const MAX_PACKAGE: u64 = 67_108_864;
const MAX_CONSTANTS_BYTES: u64 = 8_388_608;
const MAX_LAYOUTS_BYTES: u64 = 8_388_608;
const MAX_IMPORTS_BYTES: u64 = 1_048_576;
const MAX_DEPENDENCY_BYTES: u64 = 8_388_608;
const MAX_ROWS: u64 = 1_000_000;
const U32_MAX: u64 = 4_294_967_295;

/// `EXTENDED_V1` profile words of the dependency section: VM version,
/// lowering profile, lowerer version, entry type arguments, adapter ABI
/// entries, execution ABI flags.
fn profile_words() -> Vec<u8> {
    let profile = sley_vm::CacheProfile::EXTENDED_V1;
    let mut out = Vec::new();
    for part in profile.vm_version {
        out.extend_from_slice(&part.to_be_bytes());
    }
    out.extend_from_slice(&profile.lowering_profile.to_be_bytes());
    for part in profile.lowerer_version {
        out.extend_from_slice(&part.to_be_bytes());
    }
    out.extend_from_slice(&profile.entry_type_arguments.to_be_bytes());
    out.extend_from_slice(&profile.adapter_abi_entries.to_be_bytes());
    out.extend_from_slice(&profile.execution_abi_flags.to_be_bytes());
    out
}

/// The fixed header prefix: magic, version 3, `BOOTSTRAP_PROFILE_3_DIGEST`,
/// `HOST_ABI_V3_VERSION`, VM version.
fn header_prefix() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(sley_vm::exec_package::EXEC_PACKAGE_MAGIC);
    out.extend_from_slice(&sley_vm::exec_package::EXEC_PACKAGE_V3_VERSION.to_be_bytes());
    out.extend_from_slice(&sley_vm::exec_package::BOOTSTRAP_PROFILE_3_DIGEST);
    out.extend_from_slice(&sley_vm::host_abi::HOST_ABI_V3_VERSION.to_be_bytes());
    for part in sley_vm::CacheProfile::EXTENDED_V1.vm_version {
        out.extend_from_slice(&part.to_be_bytes());
    }
    out
}

fn fail(f: &mut FnB<'_>, condition: E, err: u32) {
    f.if_(condition, |f| f.ret_err(code(err)));
}

fn put_len(f: &mut FnB<'_>, x: E) {
    put(f, call("fp_be64", vec![vlen(x)]), u64c(8));
}

/// Opens a length-framed row: a placeholder piece and the row start.
fn row_open(f: &mut FnB<'_>) {
    f.set("at", vlen(v("out")));
    f.set("out", push(v("out"), bytesc(&[])));
    f.set("n", add(v("n"), u64c(8)));
    f.set("start", v("n"));
}

/// Closes a row: the placeholder becomes `u64(row length)`.
fn row_close(f: &mut FnB<'_>) {
    f.set(
        "out",
        vset(
            v("out"),
            v("at"),
            call("fp_be64", vec![sub(v("n"), v("start"))]),
        ),
    );
}

fn section_vars(f: &mut FnB<'_>) {
    f.var("out", vnew(t::bytes(), vec![]));
    f.var("n", u64c(0));
    f.var("at", u64c(0));
    f.var("start", u64c(0));
}

fn no_self() -> E {
    bytesc(&[])
}

/// Defines the `pk_*` functions.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    let inv_ty = m.ty(INV);
    let lim_ty = m.ty(LIM);
    let gate_ty = m.ty(GATE);
    let sec = t::tuple(vec![t::bytes(), t::u64()]);
    let sec_memo = t::tuple(vec![t::bytes(), t::u64(), memo_ty()]);

    // Length of a canonical ConstValue encoding from its type slice: the
    // record `uvar(2) || uvar(1) || uvar(len t) || t || uvar(2) ||
    // uvar(len d) || d`, reading only the data-length varint bytes.
    m.func(
        "pk_cst_len",
        &[("val", t::bytes()), ("ty", t::bytes())],
        t::u64(),
        |f| {
            f.let_("lt", call("rt_blen", vec![v("ty")]));
            f.var(
                "p",
                add(
                    add(u64c(3), v("lt")),
                    call("rt_blen", vec![call("uvarb", vec![v("lt")])]),
                ),
            );
            f.var("ld", u64c(0));
            f.var("shift", u32c(0));
            f.loop_(|f| {
                f.let_(
                    "byte",
                    call(
                        "rt_b",
                        vec![vget(b2v(slice(v("val"), v("p"), u64c(1))), u64c(0))],
                    ),
                );
                f.set("p", add(v("p"), u64c(1)));
                f.set(
                    "ld",
                    add(v("ld"), shl(rem(v("byte"), u64c(128)), v("shift"))),
                );
                f.if_(lt(v("byte"), u64c(128)), super::super::seed::FnB::break_);
                f.set("shift", add(v("shift"), u32c(7)));
            });
            f.ret(add(v("p"), v("ld")));
        },
    );
    // Constants section.
    m.func(
        "pk_constants",
        &[("inv", inv_ty.clone())],
        r(sec.clone()),
        |f| {
            f.let_("cs", field(v("inv"), "csts"));
            fail(f, gt(vlen(v("cs")), u64c(MAX_ROWS)), PK_OVERSIZED);
            section_vars(f);
            put_len(f, v("cs"));
            f.for_u64("i", u64c(0), vlen(v("cs")), |f| {
                f.let_("c", vget(v("cs"), v("i")));
                f.let_(
                    "vl",
                    call(
                        "pk_cst_len",
                        vec![field(v("c"), "value"), field(v("c"), "ty")],
                    ),
                );
                put(f, field(v("c"), "id"), u64c(32));
                put(f, call("fp_be64", vec![v("vl")]), u64c(8));
                put(f, field(v("c"), "value"), v("vl"));
            });
            keep(f, &["inv"]);
            fail(f, gt(v("n"), u64c(MAX_CONSTANTS_BYTES)), PK_OVERSIZED);
            f.ret_ok(tup(vec![concat(v("out")), v("n")]));
        },
    );
    // Layouts section (one row per type definition).
    m.func(
        "pk_layouts",
        &[("inv", inv_ty.clone()), ("memo", memo_ty())],
        r(sec_memo.clone()),
        |f| {
            f.var("memo", v("memo"));
            f.let_("ds", field(v("inv"), "tdefs"));
            fail(f, gt(vlen(v("ds")), u64c(MAX_ROWS)), PK_OVERSIZED);
            section_vars(f);
            put_len(f, v("ds"));
            f.for_u64("i", u64c(0), vlen(v("ds")), |f| {
                f.let_("d", vget(v("ds"), v("i")));
                row_open(f);
                put(f, field(v("d"), "id"), u64c(32));
                f.let_("tps", field(v("d"), "tparams"));
                put_len(f, v("tps"));
                f.for_u64("k", u64c(0), vlen(v("tps")), |f| {
                    put(f, call("fp_be32", vec![vget(v("tps"), v("k"))]), u64c(4));
                });
                f.let_("rec", field(v("d"), "is_rec"));
                f.if_else(
                    v("rec"),
                    |f| put(f, lit32(1), u64c(4)),
                    |f| put(f, lit32(2), u64c(4)),
                );
                f.let_("ms", field(v("d"), "members"));
                put_len(f, v("ms"));
                f.for_u64("k", u64c(0), vlen(v("ms")), |f| {
                    f.let_("mm", vget(v("ms"), v("k")));
                    put(f, field(v("mm"), "member"), u64c(32));
                    f.if_else(
                        v("rec"),
                        |f| {
                            tyc(f, field(v("mm"), "ty"), no_self());
                            put(f, call("fp_be32", vec![field(v("mm"), "vis")]), u64c(4));
                        },
                        |f| {
                            f.if_else(
                                field(v("mm"), "has_ty"),
                                |f| {
                                    put(f, lit32(2), u64c(4));
                                    tyc(f, field(v("mm"), "ty"), no_self());
                                },
                                |f| put(f, lit32(1), u64c(4)),
                            );
                        },
                    );
                });
                f.let_("is", field(v("d"), "invariants"));
                put_len(f, v("is"));
                f.for_u64("k", u64c(0), vlen(v("is")), |f| {
                    put(f, vget(v("is"), v("k")), u64c(32));
                });
                put(f, call("fp_be32", vec![field(v("d"), "vis")]), u64c(4));
                row_close(f);
            });
            keep(f, &["inv"]);
            fail(f, gt(v("n"), u64c(MAX_LAYOUTS_BYTES)), PK_OVERSIZED);
            f.ret_ok(tup(vec![concat(v("out")), v("n"), v("memo")]));
        },
    );
    // Imports section (exact rows).
    m.func(
        "pk_imports",
        &[("inv", inv_ty.clone()), ("memo", memo_ty())],
        r(sec_memo),
        |f| {
            f.var("memo", v("memo"));
            f.let_("is", field(v("inv"), "imps"));
            fail(f, gt(vlen(v("is")), u64c(MAX_ROWS)), PK_OVERSIZED);
            section_vars(f);
            put_len(f, v("is"));
            f.for_u64("i", u64c(0), vlen(v("is")), |f| {
                f.let_("x", vget(v("is"), v("i")));
                row_open(f);
                put(f, field(v("x"), "id"), u64c(32));
                put(f, field(v("x"), "adapter"), u64c(32));
                put(f, call("fp_be32", vec![field(v("x"), "abi")]), u64c(4));
                tyc(f, field(v("x"), "req"), no_self());
                tyc(f, field(v("x"), "resp"), no_self());
                tyc(f, field(v("x"), "fail"), no_self());
                f.let_("es", field(v("x"), "effects"));
                put_len(f, v("es"));
                f.for_u64("k", u64c(0), vlen(v("es")), |f| {
                    put(f, vget(v("es"), v("k")), u64c(32));
                });
                row_close(f);
            });
            keep(f, &["inv"]);
            fail(f, gt(v("n"), u64c(MAX_IMPORTS_BYTES)), PK_OVERSIZED);
            f.ret_ok(tup(vec![concat(v("out")), v("n"), v("memo")]));
        },
    );
    // Dependency section (no globals, no contracts).
    m.func(
        "pk_dependency",
        &[
            ("entry", t::bytes()),
            ("epoch", t::bytes()),
            ("root", t::bytes()),
            ("lim", lim_ty.clone()),
            ("gate", gate_ty.clone()),
        ],
        r(sec),
        |f| {
            f.let_("ops", field(v("gate"), "ops"));
            f.let_("brs", field(v("gate"), "bridges"));
            fail(
                f,
                or(gt(v("ops"), u64c(U32_MAX)), gt(v("brs"), u64c(U32_MAX))),
                PK_MALFORMED,
            );
            f.let_("fps", field(v("gate"), "fps"));
            section_vars(f);
            put(f, v("entry"), u64c(32));
            put(f, v("epoch"), u64c(32));
            put(f, v("root"), u64c(32));
            let words = profile_words();
            let words_len = words.len() as u64;
            put(f, bytesc(&words), u64c(words_len));
            put(f, call("fp_be32", vec![v("ops")]), u64c(4));
            put(f, call("fp_be32", vec![v("brs")]), u64c(4));
            put_len(f, v("fps"));
            f.for_u64("k", u64c(0), vlen(v("fps")), |f| {
                f.let_("fp", vget(v("fps"), v("k")));
                fail(
                    f,
                    ne(call("rt_blen", vec![v("fp")]), u64c(32)),
                    PK_MALFORMED,
                );
                put(f, v("fp"), u64c(32));
            });
            for name in ["instructions", "fuel", "units", "output"] {
                put(f, call("fp_be64", vec![field(v("lim"), name)]), u64c(8));
            }
            f.if_else(
                field(v("lim"), "has_cancel"),
                |f| {
                    put(f, lit32(2), u64c(4));
                    put(f, call("fp_be64", vec![field(v("lim"), "cancel")]), u64c(8));
                },
                |f| put(f, lit32(1), u64c(4)),
            );
            // Empty globals and contracts.
            put(f, bytesc(&[0; 16]), u64c(16));
            fail(f, gt(v("n"), u64c(MAX_DEPENDENCY_BYTES)), PK_OVERSIZED);
            f.ret_ok(tup(vec![concat(v("out")), v("n")]));
        },
    );
    // The four derived sections:
    // (constants, len, layouts, len, imports, len, dependency, len).
    let secs = t::tuple(vec![
        t::bytes(),
        t::u64(),
        t::bytes(),
        t::u64(),
        t::bytes(),
        t::u64(),
        t::bytes(),
        t::u64(),
    ]);
    m.func(
        "pk_sections",
        &[
            ("inv", inv_ty.clone()),
            ("ilen", t::u64()),
            ("entry", t::bytes()),
            ("epoch", t::bytes()),
            ("root", t::bytes()),
            ("lim", lim_ty.clone()),
            ("gate", gate_ty.clone()),
        ],
        r(secs),
        |f| {
            for name in ["entry", "epoch", "root"] {
                fail(
                    f,
                    ne(call("rt_blen", vec![v(name)]), u64c(32)),
                    PK_MALFORMED,
                );
            }
            f.let_("c", tri(call("pk_constants", vec![v("inv")])));
            f.let_(
                "l",
                tri(call(
                    "pk_layouts",
                    vec![
                        v("inv"),
                        mnew(t::bytes(), super::fingerprint::ent_ty(), vec![]),
                    ],
                )),
            );
            f.let_(
                "i",
                tri(call("pk_imports", vec![v("inv"), tget(v("l"), 2)])),
            );
            f.let_(
                "d",
                tri(call(
                    "pk_dependency",
                    vec![v("entry"), v("epoch"), v("root"), v("lim"), v("gate")],
                )),
            );
            fail(f, gt(v("ilen"), u64c(MAX_PACKAGE)), PK_OVERSIZED);
            f.let_(
                "total",
                add(
                    add(v("ilen"), tget(v("c"), 1)),
                    add(add(tget(v("l"), 1), tget(v("i"), 1)), tget(v("d"), 1)),
                ),
            );
            fail(f, gt(v("total"), u64c(MAX_PACKAGE)), PK_OVERSIZED);
            f.ret_ok(tup(vec![
                tget(v("c"), 0),
                tget(v("c"), 1),
                tget(v("l"), 0),
                tget(v("l"), 1),
                tget(v("i"), 0),
                tget(v("i"), 1),
                tget(v("d"), 0),
                tget(v("d"), 1),
            ]));
        },
    );
    // The 316-byte header over the image and the derived sections.
    m.func(
        "pk_header",
        &[
            ("image", t::bytes()),
            ("c", t::bytes()),
            ("l", t::bytes()),
            ("i", t::bytes()),
            ("d", t::bytes()),
            ("entry", t::bytes()),
            ("epoch", t::bytes()),
            ("root", t::bytes()),
        ],
        t::bytes(),
        |f| {
            f.ret(concat(vnew(
                t::bytes(),
                vec![
                    bytesc(&header_prefix()),
                    if crate::seed::fault("package-digest") {
                        blake3_large(v("image"))
                    } else {
                        sha256(v("image"))
                    },
                    sha256(v("c")),
                    sha256(v("l")),
                    sha256(v("i")),
                    sha256(v("d")),
                    v("entry"),
                    v("epoch"),
                    v("root"),
                ],
            )));
        },
    );
    let pk_args = |extra_len: bool| {
        let mut args: Vec<(&'static str, Ty)> =
            vec![("inv", inv_ty.clone()), ("image", t::bytes())];
        if extra_len {
            args.push(("ilen", t::u64()));
        }
        args.extend([
            ("entry", t::bytes()),
            ("epoch", t::bytes()),
            ("root", t::bytes()),
            ("lim", lim_ty.clone()),
            ("gate", gate_ty.clone()),
        ]);
        args
    };
    let sections_call = || {
        tri(call(
            "pk_sections",
            vec![
                v("inv"),
                v("ilen"),
                v("entry"),
                v("epoch"),
                v("root"),
                v("lim"),
                v("gate"),
            ],
        ))
    };
    let header_call = || {
        call(
            "pk_header",
            vec![
                v("image"),
                tget(v("s"), 0),
                tget(v("s"), 2),
                tget(v("s"), 4),
                tget(v("s"), 6),
                v("entry"),
                v("epoch"),
                v("root"),
            ],
        )
    };
    // The package digest: SHA-256 of the header (`ilen` = image length).
    m.func(
        "pk_package_digest",
        &pk_args(true),
        r(t::bytes()),
        move |f| {
            f.let_("s", sections_call());
            f.ret_ok(sha256(header_call()));
        },
    );
    // The complete envelope (`ilen` = image length).
    m.func(
        "pk_envelope_sized",
        &pk_args(true),
        r(t::bytes()),
        move |f| {
            f.let_("s", sections_call());
            f.let_("h", header_call());
            f.ret_ok(concat(vnew(
                t::bytes(),
                vec![
                    v("h"),
                    call("fp_be64", vec![v("ilen")]),
                    v("image"),
                    call("fp_be64", vec![tget(v("s"), 1)]),
                    tget(v("s"), 0),
                    call("fp_be64", vec![tget(v("s"), 3)]),
                    tget(v("s"), 2),
                    call("fp_be64", vec![tget(v("s"), 5)]),
                    tget(v("s"), 4),
                    call("fp_be64", vec![tget(v("s"), 7)]),
                    tget(v("s"), 6),
                ],
            )));
        },
    );
    // The complete envelope, measuring the image (at most 1 MiB).
    m.func("pk_envelope", &pk_args(false), r(t::bytes()), |f| {
        f.ret(call(
            "pk_envelope_sized",
            vec![
                v("inv"),
                v("image"),
                call("rt_blen", vec![v("image")]),
                v("entry"),
                v("epoch"),
                v("root"),
                v("lim"),
                v("gate"),
            ],
        ));
    });

    // ----- test entry -----
    // Loads S and returns (envelope, package digest); `sized` false uses
    // `pk_envelope` (measured image).
    m.func(
        "t_pk_envelope",
        &[
            ("objects", t::vec(t::bytes())),
            ("epoch", t::bytes()),
            ("image", t::bytes()),
            ("ilen", t::u64()),
            ("sized", t::bool()),
            ("entry", t::bytes()),
            ("root", t::bytes()),
            ("limits", t::vec(t::u64())),
            ("has_cancel", t::bool()),
            ("gops", t::u64()),
            ("gbridges", t::u64()),
            ("fps", t::vec(t::bytes())),
        ],
        r(t::tuple(vec![t::bytes(), t::bytes()])),
        |f| {
            f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
            f.let_(
                "lim",
                rec(
                    LIM,
                    vec![
                        vget(v("limits"), u64c(0)),
                        vget(v("limits"), u64c(1)),
                        vget(v("limits"), u64c(2)),
                        vget(v("limits"), u64c(3)),
                        v("has_cancel"),
                        vget(v("limits"), u64c(4)),
                    ],
                ),
            );
            f.let_(
                "gate",
                rec(
                    GATE,
                    vec![
                        vnew(t::bytes(), vec![]),
                        v("gops"),
                        v("gbridges"),
                        vnew(t::bytes(), vec![]),
                        v("fps"),
                    ],
                ),
            );
            let tail = || vec![v("entry"), v("epoch"), v("root"), v("lim"), v("gate")];
            f.var("env", bytesc(&[]));
            f.if_else(
                v("sized"),
                |f| {
                    let mut args = vec![v("inv"), v("image"), v("ilen")];
                    args.extend(tail());
                    f.set("env", tri(call("pk_envelope_sized", args)));
                },
                |f| {
                    let mut args = vec![v("inv"), v("image")];
                    args.extend(tail());
                    f.set("env", tri(call("pk_envelope", args)));
                },
            );
            let mut args = vec![v("inv"), v("image"), v("ilen")];
            args.extend(tail());
            f.let_("dg", tri(call("pk_package_digest", args)));
            f.ret_ok(tup(vec![v("env"), v("dg")]));
        },
    );
}
