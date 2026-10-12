//! Entity-object decoding (`cd_*`): the SCB1 standalone envelope, the
//! `ObjectId` digest, the `EntityObject` record, and the bodies of the
//! entity kinds in the toolchain's program domain.
//!
//! Every decoder takes `(b, bs, p, e)`: the object as `Vector<UInt(8)>` and
//! as `Bytes`, a start, and an exclusive end, and must consume `[p, e)`
//! exactly. Nested `TypeExpr` and `ConstValue` regions are validated by the
//! `scb_walk` walker at their native depth and kept as canonical bytes.
//!
//! Domain: Workspace, Package, Namespace, TypeDef, Function, Parameter,
//! Block, Operation, Constant, `AdapterImport`, and `EntryPoint` decode; the
//! other seven kinds refuse with `TOOL_KIND_OUT_OF_DOMAIN`. Labels must be
//! ASCII (always NFC); a non-ASCII label refuses with
//! `TOOL_LABEL_OUT_OF_DOMAIN`.

use super::codes::{
    SCB_CONTRACT_UNKNOWN, SCB_DIGEST_MISMATCH, SCB_EPOCH_MISMATCH, SCB_FIELD_DUPLICATE,
    SCB_FIELD_MISSING, SCB_FIELD_ORDER, SCB_FIELD_UNKNOWN, SCB_LENGTH_OVERFLOW, SCB_MAGIC_INVALID,
    SCB_MAP_DUPLICATE, SCB_MAP_ORDER, SCB_RESOURCE_LIMIT, SCB_TRAILING_BYTES, SCB_UNION_INVALID,
    SCB_VERSION_UNSUPPORTED, TOOL_KIND_OUT_OF_DOMAIN, TOOL_LABEL_OUT_OF_DOMAIN,
};
use super::model::*;
use super::rt::{MAX_STANDALONE, code, r, rcall, rd};
use super::scb::{K_CONST, K_TYPE};
use crate::seed::*;

fn dparams() -> Vec<(&'static str, Ty)> {
    vec![
        ("b", t::vec(t::u8())),
        ("bs", t::bytes()),
        ("p", t::u64()),
        ("e", t::u64()),
    ]
}

/// A decoder call over the region `[s, e)`.
fn dcall(name: &str, s: E, e: E) -> E {
    call(name, vec![v("b"), v("bs"), s, e])
}

/// A decoder call with an extra trailing argument.
fn dcall1(name: &str, s: E, e: E, extra: E) -> E {
    call(name, vec![v("b"), v("bs"), s, e, extra])
}

fn finished(f: &mut FnB<'_>) {
    f.if_(ne(v("p"), v("e")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
}

/// Reads an all-required record of `n` fields into `fs` (regions).
fn fields(f: &mut FnB<'_>, n: u64) {
    f.var("p", v("p"));
    rd(f, "fs", "p", rcall("scb_fields", vec![u64c(n)]));
    finished(f);
}

fn fs(i: u64) -> (E, E) {
    (
        tget(vget(v("fs"), u64c(i)), 0),
        tget(vget(v("fs"), u64c(i)), 1),
    )
}

/// Decodes field `i` with `name` (plus optional extra argument).
fn field(i: u64, name: &str) -> E {
    let (s, e) = fs(i);
    tri(dcall(name, s, e))
}

fn field1(i: u64, name: &str, extra: E) -> E {
    let (s, e) = fs(i);
    tri(dcall1(name, s, e, extra))
}

fn vref_ty(m: &Module) -> Ty {
    m.ty(VREF)
}

fn empty_vref() -> E {
    rec(VREF, vec![u64c(0), bytesc(&[]), u64c(0)])
}

/// Reads one union header into `tag`/`ps`/`pe` and requires it to end the
/// region.
fn union_header(f: &mut FnB<'_>) {
    f.var("p", v("p"));
    rd(f, "tag", "p", rcall("rt_rd_uvar", vec![u32c(32)]));
    rd(f, "ps", "p", rcall("rt_rd_payload", vec![]));
    f.let_("pe", v("p"));
    finished(f);
}

/// Defines the codec.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    let ids = t::vec(t::bytes());
    // EntityId / FixedBytes<32>: exactly 32 bytes.
    m.func("cd_id", &dparams(), r(t::bytes()), |f| {
        f.if_(lt(sub(v("e"), v("p")), u64c(32)), |f| {
            f.ret_err(code(SCB_LENGTH_OVERFLOW));
        });
        f.if_(gt(sub(v("e"), v("p")), u64c(32)), |f| {
            f.ret_err(code(SCB_TRAILING_BYTES));
        });
        f.ret_ok(slice(v("bs"), v("p"), u64c(32)));
    });
    // List<EntityId> (`set`: strictly increasing, no duplicate).
    let mut p = dparams();
    p.push(("set", t::bool()));
    m.func("cd_ids", &p, r(ids.clone()), |f| {
        f.var("p", v("p"));
        rd(f, "count", "p", rcall("rt_rd_count", vec![]));
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("i", u64c(0), v("count"), |f| {
            rd(f, "es", "p", rcall("rt_rd_payload", vec![]));
            f.let_("id", tri(dcall("cd_id", v("es"), v("p"))));
            if !fault("codec-set-order") {
                f.if_(and(v("set"), gt(v("i"), u64c(0))), |f| {
                    f.let_("prev", vget(v("out"), sub(v("i"), u64c(1))));
                    f.if_(eq(v("prev"), v("id")), |f| f.ret_err(code(SCB_MAP_DUPLICATE)));
                    f.if_(gt(v("prev"), v("id")), |f| f.ret_err(code(SCB_MAP_ORDER)));
                });
            }
            f.set("out", push(v("out"), v("id")));
        });
        finished(f);
        f.ret_ok(v("out"));
    });
    // UInt32.
    m.func("cd_u32", &dparams(), r(t::u64()), |f| {
        f.var("p", v("p"));
        rd(f, "x", "p", rcall("rt_rd_uvar", vec![u32c(32)]));
        finished(f);
        f.ret_ok(v("x"));
    });
    // Closed enum 1..=max (encoded as a UInt32 tag).
    let mut p = dparams();
    p.push(("max", t::u64()));
    m.func("cd_enum", &p, r(t::u64()), |f| {
        f.var("p", v("p"));
        rd(f, "x", "p", rcall("rt_rd_uvar", vec![u32c(32)]));
        f.if_(or(lt(v("x"), u64c(1)), gt(v("x"), v("max"))), |f| {
            f.ret_err(code(SCB_UNION_INVALID));
        });
        finished(f);
        f.ret_ok(v("x"));
    });
    // TypeExpr at `depth`, kept as canonical bytes.
    let mut p = dparams();
    p.push(("depth", t::u64()));
    m.func("cd_type", &p, r(t::bytes()), |f| {
        f.expr(tri(call(
            "scb_walk",
            vec![v("b"), v("bs"), v("p"), v("e"), u64c(K_TYPE), v("depth")],
        )));
        f.ret_ok(slice(v("bs"), v("p"), sub(v("e"), v("p"))));
    });
    // List<TypeExpr> at `depth` (elements one deeper).
    let mut p = dparams();
    p.push(("depth", t::u64()));
    m.func("cd_types", &p, r(ids.clone()), |f| {
        f.if_(ge(v("depth"), u64c(64)), |f| f.ret_err(code(SCB_RESOURCE_LIMIT)));
        f.var("p", v("p"));
        rd(f, "count", "p", rcall("rt_rd_count", vec![]));
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("i", u64c(0), v("count"), |f| {
            rd(f, "es", "p", rcall("rt_rd_payload", vec![]));
            f.set(
                "out",
                push(
                    v("out"),
                    tri(dcall1("cd_type", v("es"), v("p"), add(v("depth"), u64c(1)))),
                ),
            );
        });
        finished(f);
        f.ret_ok(v("out"));
    });
    // List<TypeParameterDef>: the ordinals.
    m.func("cd_tparams", &dparams(), r(t::vec(t::u64())), |f| {
        f.var("p", v("p"));
        rd(f, "count", "p", rcall("rt_rd_count", vec![]));
        f.var("out", vnew(t::u64(), vec![]));
        f.for_u64("i", u64c(0), v("count"), |f| {
            rd(f, "es", "p", rcall("rt_rd_payload", vec![]));
            f.let_("q", v("p"));
            f.set("out", push(v("out"), tri(call("cd_tparam", vec![v("b"), v("bs"), v("es"), v("q")]))));
        });
        finished(f);
        f.ret_ok(v("out"));
    });
    m.func("cd_tparam", &dparams(), r(t::u64()), |f| {
        fields(f, 1);
        f.ret_ok(field(0, "cd_u32"));
    });
    // ValueRef.
    let vref = vref_ty(m);
    m.func("cd_vref", &dparams(), r(vref.clone()), |f| {
        union_header(f);
        f.if_(eq(v("tag"), u64c(1)), |f| {
            f.ret_ok(rec(
                VREF,
                vec![u64c(1), tri(dcall("cd_id", v("ps"), v("pe"))), u64c(0)],
            ));
        });
        f.if_(eq(v("tag"), u64c(2)), |f| {
            f.ret(call("cd_opres", vec![v("b"), v("bs"), v("ps"), v("pe")]));
        });
        f.ret_err(code(SCB_UNION_INVALID));
    });
    m.func("cd_opres", &dparams(), r(vref.clone()), |f| {
        fields(f, 2);
        f.ret_ok(rec(VREF, vec![u64c(2), field(0, "cd_id"), field(1, "cd_u32")]));
    });
    m.func("cd_vrefs", &dparams(), r(t::vec(vref.clone())), |f| {
        f.var("p", v("p"));
        rd(f, "count", "p", rcall("rt_rd_count", vec![]));
        f.var("out", vnew(m_vref(), vec![]));
        f.for_u64("i", u64c(0), v("count"), |f| {
            rd(f, "es", "p", rcall("rt_rd_payload", vec![]));
            f.set("out", push(v("out"), tri(dcall("cd_vref", v("es"), v("p")))));
        });
        finished(f);
        f.ret_ok(v("out"));
    });
    // TargetEdge.
    m.func("cd_edge", &dparams(), r(m.ty(EDGE)), |f| {
        fields(f, 2);
        f.ret_ok(rec(EDGE, vec![field(0, "cd_id"), field(1, "cd_vrefs")]));
    });
    // SwitchArgument.
    m.func("cd_sarg", &dparams(), r(m.ty(SARG)), |f| {
        union_header(f);
        f.if_(eq(v("tag"), u64c(1)), |f| {
            f.ret_ok(rec(
                SARG,
                vec![boolc(false), tri(dcall("cd_vref", v("ps"), v("pe")))],
            ));
        });
        f.if_(eq(v("tag"), u64c(2)), |f| {
            f.if_(ne(v("ps"), v("pe")), |f| f.ret_err(code(SCB_UNION_INVALID)));
            f.ret_ok(rec(SARG, vec![boolc(true), empty_vref()]));
        });
        f.ret_err(code(SCB_UNION_INVALID));
    });
    // SwitchCase: CaseKey then SwitchEdge.
    m.func("cd_scase", &dparams(), r(m.ty(SCASE)), |f| {
        fields(f, 2);
        // CaseKey union.
        let (ks, ke) = fs(0);
        f.var("p", ks);
        f.let_("ke", ke);
        rd(f, "ktag", "p", call("rt_rd_uvar", vec![v("b"), v("p"), v("ke"), u32c(32)]));
        rd(f, "kps", "p", call("rt_rd_payload", vec![v("b"), v("p"), v("ke")]));
        f.if_(ne(v("p"), v("ke")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
        f.var("member", boolc(false));
        f.var("key", bytesc(&[]));
        f.var("builtin", u64c(0));
        f.if_else(
            eq(v("ktag"), u64c(1)),
            |f| {
                f.set("member", boolc(true));
                f.set("key", tri(dcall("cd_id", v("kps"), v("ke"))));
            },
            |f| {
                f.if_(ne(v("ktag"), u64c(2)), |f| f.ret_err(code(SCB_UNION_INVALID)));
                f.set("builtin", tri(dcall1("cd_enum", v("kps"), v("ke"), u64c(4))));
            },
        );
        // SwitchEdge record.
        let (es, ee) = fs(1);
        f.let_("edge", tri(call("cd_sedge", vec![v("b"), v("bs"), es, ee])));
        f.ret_ok(rec(
            SCASE,
            vec![
                v("member"),
                v("key"),
                v("builtin"),
                tget(v("edge"), 0),
                tget(v("edge"), 1),
            ],
        ));
    });
    m.func(
        "cd_sedge",
        &dparams(),
        r(t::tuple(vec![t::bytes(), t::vec(m.ty(SARG))])),
        |f| {
            fields(f, 2);
            let (s, e) = fs(1);
            f.let_("ls", s);
            f.let_("le", e);
            f.var("q", v("ls"));
            rd(f, "count", "q", call("rt_rd_count", vec![v("b"), v("q"), v("le")]));
            f.var("args", vnew(m_sarg(), vec![]));
            f.for_u64("i", u64c(0), v("count"), |f| {
                rd(f, "es", "q", call("rt_rd_payload", vec![v("b"), v("q"), v("le")]));
                f.set("args", push(v("args"), tri(dcall("cd_sarg", v("es"), v("q")))));
            });
            f.if_(ne(v("q"), v("le")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
            f.ret_ok(tup(vec![field(0, "cd_id"), v("args")]));
        },
    );
    // Terminator.
    m.func("cd_term", &dparams(), r(m.ty(TERM)), |f| {
        union_header(f);
        f.let_("ps0", v("ps"));
        f.let_("pe0", v("pe"));
        f.if_(or(lt(v("tag"), u64c(1)), gt(v("tag"), u64c(5))), |f| {
            f.ret_err(code(SCB_UNION_INVALID));
        });
        f.let_("nfields", call("cd_term_fields", vec![v("tag")]));
        f.var("p", v("ps0"));
        f.let_("e", v("pe0"));
        rd(f, "fs", "p", rcall("scb_fields", vec![v("nfields")]));
        f.if_(ne(v("p"), v("e")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
        f.var("val", empty_vref());
        f.var("has_val", boolc(false));
        f.var("tcode", u64c(0));
        f.var("edges", vnew(m_edge(), vec![]));
        f.var("cases", vnew(m_scase(), vec![]));
        f.if_(eq(v("tag"), u64c(1)), |f| {
            f.set("val", field(0, "cd_vref"));
            f.set("has_val", boolc(true));
        });
        f.if_(eq(v("tag"), u64c(2)), |f| {
            f.set("edges", push(v("edges"), field(0, "cd_edge")));
        });
        f.if_(eq(v("tag"), u64c(3)), |f| {
            f.set("val", field(0, "cd_vref"));
            f.set("has_val", boolc(true));
            f.set("edges", push(v("edges"), field(1, "cd_edge")));
            f.set("edges", push(v("edges"), field(2, "cd_edge")));
        });
        f.if_(eq(v("tag"), u64c(4)), |f| {
            f.set("val", field(0, "cd_vref"));
            f.set("has_val", boolc(true));
            let (s, e) = fs(1);
            f.let_("ls", s);
            f.let_("le", e);
            f.var("q", v("ls"));
            rd(f, "count", "q", call("rt_rd_count", vec![v("b"), v("q"), v("le")]));
            f.for_u64("i", u64c(0), v("count"), |f| {
                rd(f, "es", "q", call("rt_rd_payload", vec![v("b"), v("q"), v("le")]));
                f.set("cases", push(v("cases"), tri(dcall("cd_scase", v("es"), v("q")))));
            });
            f.if_(ne(v("q"), v("le")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
        });
        f.if_(eq(v("tag"), u64c(5)), |f| {
            f.set("tcode", field1(0, "cd_enum", u64c(4)));
            // Option<ValueRef>.
            let (s, e) = fs(1);
            f.let_("os", s);
            f.let_("oe", e);
            f.var("q", v("os"));
            rd(f, "otag", "q", call("rt_rd_uvar", vec![v("b"), v("q"), v("oe"), u32c(32)]));
            rd(f, "ops", "q", call("rt_rd_payload", vec![v("b"), v("q"), v("oe")]));
            f.if_(ne(v("q"), v("oe")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
            f.if_else(
                eq(v("otag"), u64c(0)),
                |f| {
                    f.if_(ne(v("ops"), v("oe")), |f| f.ret_err(code(SCB_UNION_INVALID)));
                },
                |f| {
                    f.if_(ne(v("otag"), u64c(1)), |f| f.ret_err(code(SCB_UNION_INVALID)));
                    f.set("val", tri(dcall("cd_vref", v("ops"), v("oe"))));
                    f.set("has_val", boolc(true));
                },
            );
        });
        f.ret_ok(rec(
            TERM,
            vec![
                v("tag"),
                v("val"),
                v("has_val"),
                v("tcode"),
                v("edges"),
                v("cases"),
            ],
        ));
    });
    m.func("cd_term_fields", &[("tag", t::u64())], t::u64(), |f| {
        f.ret(select(
            or(eq(v("tag"), u64c(1)), eq(v("tag"), u64c(2))),
            u64c(1),
            select(eq(v("tag"), u64c(3)), u64c(3), u64c(2)),
        ));
    });
    // Immediate (at `depth`, the union's own depth).
    let mut p = dparams();
    p.push(("depth", t::u64()));
    m.func("cd_imm", &p, r(m.ty(IMM)), |f| {
        union_header(f);
        f.var("id", bytesc(&[]));
        f.var("member", bytesc(&[]));
        f.var("index", u64c(0));
        f.var("targs", vnew(t::bytes(), vec![]));
        f.if_(or(lt(v("tag"), u64c(1)), gt(v("tag"), u64c(7))), |f| {
            f.ret_err(code(SCB_UNION_INVALID));
        });
        f.if_(eq(v("tag"), u64c(1)), |f| {
            f.if_(ne(v("ps"), v("pe")), |f| f.ret_err(code(SCB_UNION_INVALID)));
        });
        f.if_(
            or(or(eq(v("tag"), u64c(2)), eq(v("tag"), u64c(4))), eq(v("tag"), u64c(6))),
            |f| f.set("id", tri(dcall("cd_id", v("ps"), v("pe")))),
        );
        f.if_(eq(v("tag"), u64c(3)), |f| {
            f.set("index", tri(dcall("cd_u32", v("ps"), v("pe"))));
        });
        f.if_(eq(v("tag"), u64c(5)), |f| {
            f.let_("pair", tri(dcall("cd_id_pair", v("ps"), v("pe"))));
            f.set("id", tget(v("pair"), 0));
            f.set("member", tget(v("pair"), 1));
        });
        f.if_(eq(v("tag"), u64c(7)), |f| {
            f.let_(
                "fref",
                tri(dcall1("cd_funref", v("ps"), v("pe"), add(v("depth"), u64c(1)))),
            );
            f.set("id", tget(v("fref"), 0));
            f.set("targs", tget(v("fref"), 1));
        });
        f.ret_ok(rec(
            IMM,
            vec![v("tag"), v("id"), v("member"), v("index"), v("targs")],
        ));
    });
    m.func(
        "cd_id_pair",
        &dparams(),
        r(t::tuple(vec![t::bytes(), t::bytes()])),
        |f| {
            fields(f, 2);
            f.ret_ok(tup(vec![field(0, "cd_id"), field(1, "cd_id")]));
        },
    );
    let mut p = dparams();
    p.push(("depth", t::u64()));
    m.func(
        "cd_funref",
        &p,
        r(t::tuple(vec![t::bytes(), ids.clone()])),
        |f| {
            f.if_(ge(v("depth"), u64c(64)), |f| f.ret_err(code(SCB_RESOURCE_LIMIT)));
            fields(f, 2);
            f.ret_ok(tup(vec![
                field(0, "cd_id"),
                field1(1, "cd_types", add(v("depth"), u64c(1))),
            ]));
        },
    );
    define_bodies(m);
    define_object(m);
}

fn m_vref() -> Ty {
    named(VREF)
}
fn m_sarg() -> Ty {
    named(SARG)
}
fn m_edge() -> Ty {
    named(EDGE)
}
fn m_scase() -> Ty {
    named(SCASE)
}
fn m_mem() -> Ty {
    named(MEM)
}

/// A model type by name (same identity derivation as `Module::ty`).
fn named(name: &str) -> Ty {
    Module::new(super::TOOLCHAIN_SEED).ty(name)
}

#[allow(clippy::too_many_lines)]
fn define_bodies(m: &mut Module) {
    // Workspace / Package / Namespace: validated, contents unused.
    m.func("cd_workspace", &dparams(), r(t::unit()), |f| {
        fields(f, 5);
        f.expr(field1(0, "cd_ids", boolc(true)));
        f.expr(field(1, "cd_id"));
        f.expr(field1(2, "cd_ids", boolc(true)));
        f.expr(field1(3, "cd_ids", boolc(true)));
        f.expr(field1(4, "cd_ids", boolc(true)));
        f.ret_ok(unitc());
    });
    m.func("cd_package", &dparams(), r(t::unit()), |f| {
        fields(f, 4);
        f.expr(field(0, "cd_id"));
        f.expr(field(1, "cd_id"));
        f.expr(field1(2, "cd_ids", boolc(true)));
        f.expr(field1(3, "cd_ids", boolc(true)));
        f.ret_ok(unitc());
    });
    m.func("cd_namespace", &dparams(), r(t::unit()), |f| {
        fields(f, 2);
        // Option<EntityId>.
        let (s, e) = fs(0);
        f.let_("os", s);
        f.let_("oe", e);
        f.var("q", v("os"));
        rd(f, "otag", "q", call("rt_rd_uvar", vec![v("b"), v("q"), v("oe"), u32c(32)]));
        rd(f, "ops", "q", call("rt_rd_payload", vec![v("b"), v("q"), v("oe")]));
        f.if_(ne(v("q"), v("oe")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
        f.if_else(
            eq(v("otag"), u64c(0)),
            |f| f.if_(ne(v("ops"), v("oe")), |f| f.ret_err(code(SCB_UNION_INVALID))),
            |f| {
                f.if_(ne(v("otag"), u64c(1)), |f| f.ret_err(code(SCB_UNION_INVALID)));
                f.expr(tri(dcall("cd_id", v("ops"), v("oe"))));
            },
        );
        f.expr(field1(1, "cd_ids", boolc(true)));
        f.ret_ok(unitc());
    });
    // TypeDef.
    m.func("cd_typedef", &dparams(), r(named(TDEF)), |f| {
        fields(f, 4);
        f.let_("tparams", field(0, "cd_tparams"));
        // TypeDefForm union: 1 Record<List<RecordField>>, 2 Variant<List<VariantCase>>.
        let (s, e) = fs(1);
        f.let_("fs0", s);
        f.let_("fe0", e);
        f.var("q", v("fs0"));
        rd(f, "ftag", "q", call("rt_rd_uvar", vec![v("b"), v("q"), v("fe0"), u32c(32)]));
        rd(f, "fps", "q", call("rt_rd_payload", vec![v("b"), v("q"), v("fe0")]));
        f.if_(ne(v("q"), v("fe0")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
        f.if_(or(lt(v("ftag"), u64c(1)), gt(v("ftag"), u64c(2))), |f| {
            f.ret_err(code(SCB_UNION_INVALID));
        });
        f.let_("is_rec", eq(v("ftag"), u64c(1)));
        // The member list (form at depth 2, list at 3, members at 4).
        f.var("q", v("fps"));
        rd(f, "count", "q", call("rt_rd_count", vec![v("b"), v("q"), v("fe0")]));
        f.var("members", vnew(m_mem(), vec![]));
        f.for_u64("i", u64c(0), v("count"), |f| {
            rd(f, "es", "q", call("rt_rd_payload", vec![v("b"), v("q"), v("fe0")]));
            f.set(
                "members",
                push(
                    v("members"),
                    tri(call(
                        "cd_member",
                        vec![v("b"), v("bs"), v("es"), v("q"), v("is_rec")],
                    )),
                ),
            );
        });
        f.if_(ne(v("q"), v("fe0")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
        f.let_("invariants", field1(2, "cd_ids", boolc(true)));
        f.let_("vis", field1(3, "cd_enum", u64c(4)));
        f.ret_ok(rec(
            TDEF,
            vec![
                slice(v("bs"), u64c(0), u64c(0)),
                v("tparams"),
                v("is_rec"),
                v("members"),
                v("invariants"),
                v("vis"),
            ],
        ));
    });
    let mut p = dparams();
    p.push(("is_rec", t::bool()));
    m.func("cd_member", &p, r(named(MEM)), |f| {
        f.if_else(
            v("is_rec"),
            |f| {
                fields(f, 3);
                f.ret_ok(rec(
                    MEM,
                    vec![
                        field(0, "cd_id"),
                        boolc(true),
                        field1(1, "cd_type", u64c(5)),
                        field1(2, "cd_enum", u64c(4)),
                    ],
                ));
            },
            |f| {
                fields(f, 2);
                f.let_("member", field(0, "cd_id"));
                // Option<TypeExpr>.
                let (s, e) = fs(1);
                f.let_("os", s);
                f.let_("oe", e);
                f.var("q", v("os"));
                rd(f, "otag", "q", call("rt_rd_uvar", vec![v("b"), v("q"), v("oe"), u32c(32)]));
                rd(f, "ops", "q", call("rt_rd_payload", vec![v("b"), v("q"), v("oe")]));
                f.if_(ne(v("q"), v("oe")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
                f.if_(eq(v("otag"), u64c(0)), |f| {
                    f.if_(ne(v("ops"), v("oe")), |f| f.ret_err(code(SCB_UNION_INVALID)));
                    f.ret_ok(rec(
                        MEM,
                        vec![v("member"), boolc(false), bytesc(&[]), u64c(0)],
                    ));
                });
                f.if_(ne(v("otag"), u64c(1)), |f| f.ret_err(code(SCB_UNION_INVALID)));
                f.ret_ok(rec(
                    MEM,
                    vec![
                        v("member"),
                        boolc(true),
                        tri(dcall1("cd_type", v("ops"), v("oe"), u64c(6))),
                        u64c(0),
                    ],
                ));
            },
        );
    });
    // Function.
    m.func("cd_function", &dparams(), r(named(FUN)), |f| {
        fields(f, 8);
        f.ret_ok(rec(
            FUN,
            vec![
                bytesc(&[]),
                field(0, "cd_tparams"),
                field1(1, "cd_ids", boolc(false)),
                field1(2, "cd_type", u64c(2)),
                field1(3, "cd_ids", boolc(true)),
                field(4, "cd_id"),
                field1(5, "cd_ids", boolc(false)),
                field1(6, "cd_ids", boolc(true)),
                field1(7, "cd_enum", u64c(4)),
            ],
        ));
    });
    // Parameter.
    m.func("cd_parameter", &dparams(), r(named(PRM)), |f| {
        fields(f, 4);
        f.ret_ok(rec(
            PRM,
            vec![
                bytesc(&[]),
                field(0, "cd_id"),
                field1(1, "cd_enum", u64c(2)),
                field(2, "cd_u32"),
                field1(3, "cd_type", u64c(2)),
            ],
        ));
    });
    // Block.
    m.func("cd_block", &dparams(), r(named(BLK)), |f| {
        fields(f, 5);
        f.ret_ok(rec(
            BLK,
            vec![
                bytesc(&[]),
                field(0, "cd_id"),
                field1(1, "cd_ids", boolc(false)),
                field1(2, "cd_ids", boolc(false)),
                field(3, "cd_term"),
                field1(4, "cd_enum", u64c(2)),
            ],
        ));
    });
    // Operation.
    m.func("cd_operation", &dparams(), r(named(OP)), |f| {
        fields(f, 6);
        f.ret_ok(rec(
            OP,
            vec![
                bytesc(&[]),
                field(0, "cd_id"),
                field(1, "cd_u32"),
                field(2, "cd_u32"),
                field(3, "cd_vrefs"),
                field1(4, "cd_types", u64c(2)),
                field1(5, "cd_imm", u64c(2)),
            ],
        ));
    });
    // Constant: ConstValue at depth 2, plus its type slice.
    m.func("cd_constant", &dparams(), r(named(CST)), |f| {
        fields(f, 1);
        let (s, e) = fs(0);
        f.let_("cs", s);
        f.let_("ce", e);
        f.expr(tri(call(
            "scb_walk",
            vec![v("b"), v("bs"), v("cs"), v("ce"), u64c(K_CONST), u64c(2)],
        )));
        // Field 1 of the (validated) ConstValue record is its type.
        f.var("q", v("cs"));
        rd(f, "n", "q", call("rt_rd_fields", vec![v("b"), v("q"), v("ce")]));
        rd(f, "tag", "q", call("rt_rd_uvar", vec![v("b"), v("q"), v("ce"), u32c(32)]));
        rd(f, "ts", "q", call("rt_rd_payload", vec![v("b"), v("q"), v("ce")]));
        f.ret_ok(rec(
            CST,
            vec![
                bytesc(&[]),
                slice(v("bs"), v("cs"), sub(v("ce"), v("cs"))),
                slice(v("bs"), v("ts"), sub(v("q"), v("ts"))),
            ],
        ));
    });
    // AdapterImport.
    m.func("cd_import", &dparams(), r(named(IMP)), |f| {
        fields(f, 6);
        f.ret_ok(rec(
            IMP,
            vec![
                bytesc(&[]),
                field(0, "cd_id"),
                field(1, "cd_u32"),
                field1(2, "cd_type", u64c(2)),
                field1(3, "cd_type", u64c(2)),
                field1(4, "cd_type", u64c(2)),
                field1(5, "cd_ids", boolc(true)),
            ],
        ));
    });
    // EntryPoint.
    m.func("cd_entry", &dparams(), r(named(EPT)), |f| {
        fields(f, 2);
        f.ret_ok(rec(
            EPT,
            vec![bytesc(&[]), field(0, "cd_id"), field1(1, "cd_enum", u64c(2))],
        ));
    });
}

/// Builds `Ent::<case>(record with id replaced)` for a decoded body.
fn with_id(type_name: &str, case: &str, value: E) -> E {
    // The first field of every model record is `id`; rebuild it with the
    // object's entity id.
    var_new(ENT, case, Some(call(&format!("cd_set_id_{type_name}"), vec![value, v("id")])))
}

#[allow(clippy::too_many_lines)]
fn define_object(m: &mut Module) {
    // Per-type id setters (records are immutable; rebuild with the id).
    for (type_name, fields_list) in [
        (TDEF, vec!["tparams", "is_rec", "members", "invariants", "vis"]),
        (FUN, vec!["tparams", "params", "result", "effects", "entry", "blocks", "contracts", "vis"]),
        (PRM, vec!["owner", "role", "ordinal", "ty"]),
        (BLK, vec!["function", "params", "ops", "term", "reach"]),
        (OP, vec!["block", "ordinal", "opcode", "operands", "results", "imm"]),
        (CST, vec!["value", "ty"]),
        (IMP, vec!["adapter", "abi", "req", "resp", "fail", "effects"]),
        (EPT, vec!["function", "exposure"]),
    ] {
        let ty = m.ty(type_name);
        let type_name_owned = type_name.to_owned();
        m.func(
            &format!("cd_set_id_{type_name}"),
            &[("x", ty.clone()), ("id", t::bytes())],
            ty,
            move |f| {
                let mut values = vec![v("id")];
                values.extend(fields_list.iter().map(|name| crate::seed::field(v("x"), name)));
                f.ret(rec(&type_name_owned, values));
            },
        );
    }
    // One complete entity object under `epoch`.
    m.func(
        "cd_object",
        &[("bs", t::bytes()), ("epoch", t::bytes())],
        r(m.ty(OBJ)),
        |f| {
            f.let_("b", b2v(v("bs")));
            f.let_("n", vlen(v("b")));
            f.if_(gt(v("n"), u64c(MAX_STANDALONE)), |f| {
                f.ret_err(code(SCB_RESOURCE_LIMIT));
            });
            f.if_(lt(v("n"), u64c(32)), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
            f.let_("e", sub(v("n"), u64c(32)));
            f.var("p", u64c(0));
            f.if_(lt(v("e"), u64c(8)), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
            if !fault("codec-magic") {
                f.if_(ne(slice(v("bs"), u64c(0), u64c(8)), bytesc(b"SLEYSCB1")), |f| {
                    f.ret_err(code(SCB_MAGIC_INVALID));
                });
            }
            f.set("p", u64c(8));
            rd(f, "version", "p", rcall("rt_rd_uvar", vec![u32c(64)]));
            f.if_(ne(v("version"), u64c(1)), |f| f.ret_err(code(SCB_VERSION_UNSUPPORTED)));
            rd(f, "contract", "p", rcall("rt_rd_uvar", vec![u32c(32)]));
            f.if_(ne(v("contract"), u64c(200)), |f| f.ret_err(code(SCB_CONTRACT_UNKNOWN)));
            f.if_(lt(sub(v("e"), v("p")), u64c(32)), |f| {
                f.ret_err(code(SCB_LENGTH_OVERFLOW));
            });
            f.if_(ne(slice(v("bs"), v("p"), u64c(32)), v("epoch")), |f| {
                f.ret_err(code(SCB_EPOCH_MISMATCH));
            });
            f.set("p", add(v("p"), u64c(32)));
            rd(f, "ps", "p", rcall("rt_rd_payload", vec![]));
            f.let_("pe", v("p"));
            f.if_(ne(v("p"), v("e")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
            f.let_(
                "oid",
                blake3_large(concat(vnew(
                    t::bytes(),
                    vec![bytesc(b"sley2.object.v1"), slice(v("bs"), u64c(0), v("e"))],
                ))),
            );
            f.if_(ne(slice(v("bs"), v("e"), u64c(32)), v("oid")), |f| {
                f.ret_err(code(SCB_DIGEST_MISMATCH));
            });
            // EntityObject record: 2..=4 fields, tags 1 and 2 required.
            f.set("p", v("ps"));
            f.let_("e", v("pe"));
            rd(f, "count", "p", rcall("rt_rd_fields", vec![]));
            f.if_(lt(v("count"), u64c(2)), |f| f.ret_err(code(SCB_FIELD_MISSING)));
            f.if_(gt(v("count"), u64c(4)), |f| f.ret_err(code(SCB_FIELD_UNKNOWN)));
            f.var("prev", u64c(0));
            f.var("id", bytesc(&[]));
            f.var("has_id", boolc(false));
            f.var("has_body", boolc(false));
            f.var("kind", u64c(0));
            f.var("ent", var_new(ENT, "W", None));
            f.for_u64("i", u64c(0), v("count"), |f| {
                rd(f, "tag", "p", rcall("rt_rd_uvar", vec![u32c(32)]));
                f.if_(and(gt(v("i"), u64c(0)), eq(v("tag"), v("prev"))), |f| {
                    f.ret_err(code(SCB_FIELD_DUPLICATE));
                });
                f.if_(and(gt(v("i"), u64c(0)), lt(v("tag"), v("prev"))), |f| {
                    f.ret_err(code(SCB_FIELD_ORDER));
                });
                f.set("prev", v("tag"));
                rd(f, "fstart", "p", rcall("rt_rd_payload", vec![]));
                f.if_(eq(v("tag"), u64c(1)), |f| {
                    f.set("id", tri(dcall("cd_id", v("fstart"), v("p"))));
                    f.set("has_id", boolc(true));
                });
                f.if_(eq(v("tag"), u64c(2)), |f| {
                    // Decoded in field order, like the native record.
                    f.let_(
                        "decoded",
                        tri(call(
                            "cd_body",
                            vec![v("b"), v("bs"), v("fstart"), v("p"), v("id")],
                        )),
                    );
                    f.set("kind", tget(v("decoded"), 0));
                    f.set("ent", tget(v("decoded"), 1));
                    f.set("has_body", boolc(true));
                });
                f.if_(eq(v("tag"), u64c(3)), |f| {
                    f.expr(tri(dcall("cd_label", v("fstart"), v("p"))));
                });
                f.if_(eq(v("tag"), u64c(4)), |f| {
                    f.expr(tri(dcall("cd_id", v("fstart"), v("p"))));
                });
                f.if_(or(eq(v("tag"), u64c(0)), gt(v("tag"), u64c(4))), |f| {
                    f.ret_err(code(SCB_FIELD_UNKNOWN));
                });
            });
            f.if_(ne(v("p"), v("e")), |f| f.ret_err(code(SCB_TRAILING_BYTES)));
            f.if_(or(not(v("has_id")), not(v("has_body"))), |f| {
                f.ret_err(code(SCB_FIELD_MISSING));
            });
            f.ret_ok(rec(OBJ, vec![v("id"), v("oid"), v("kind"), v("ent")]));
        },
    );
    // Label: Text, at most 1024 bytes, ASCII only (trivially NFC).
    m.func("cd_label", &dparams(), r(t::unit()), |f| {
        f.var("p", v("p"));
        rd(f, "len", "p", rcall("rt_rd_len", vec![u64c(16_777_216)]));
        f.let_("start", v("p"));
        f.if_(gt(add(v("p"), v("len")), v("e")), |f| f.ret_err(code(SCB_LENGTH_OVERFLOW)));
        f.set("p", add(v("p"), v("len")));
        f.for_u64("i", v("start"), v("p"), |f| {
            f.if_(ge(call("rt_b", vec![vget(v("b"), v("i"))]), u64c(128)), |f| {
                f.ret_err(code(TOOL_LABEL_OUT_OF_DOMAIN));
            });
        });
        finished(f);
        f.if_(gt(v("len"), u64c(1024)), |f| f.ret_err(code(SCB_RESOURCE_LIMIT)));
        f.ret_ok(unitc());
    });
    let mut p = dparams();
    p.push(("id", t::bytes()));
    m.func(
        "cd_body",
        &p,
        r(t::tuple(vec![t::u64(), named(ENT)])),
        |f| {
            union_header(f);
            f.if_(or(lt(v("tag"), u64c(1)), gt(v("tag"), u64c(18))), |f| {
                f.ret_err(code(SCB_UNION_INVALID));
            });
            f.let_("s", v("ps"));
            f.let_("t", v("pe"));
            let body = |name: &str| tri(call(name, vec![v("b"), v("bs"), v("s"), v("t")]));
            f.if_(eq(v("tag"), u64c(1)), move |f| {
                f.expr(body("cd_workspace"));
                f.ret_ok(tup(vec![v("tag"), var_new(ENT, "W", None)]));
            });
            f.if_(eq(v("tag"), u64c(2)), move |f| {
                f.expr(body("cd_package"));
                f.ret_ok(tup(vec![v("tag"), var_new(ENT, "K", None)]));
            });
            f.if_(eq(v("tag"), u64c(3)), move |f| {
                f.expr(body("cd_namespace"));
                f.ret_ok(tup(vec![v("tag"), var_new(ENT, "N", None)]));
            });
            for (tag, decoder, type_name, case) in [
                (4, "cd_typedef", TDEF, "T"),
                (5, "cd_function", FUN, "F"),
                (6, "cd_parameter", PRM, "P"),
                (7, "cd_block", BLK, "B"),
                (8, "cd_operation", OP, "O"),
                (9, "cd_constant", CST, "C"),
                (15, "cd_import", IMP, "I"),
                (16, "cd_entry", EPT, "E"),
            ] {
                f.if_(eq(v("tag"), u64c(tag)), move |f| {
                    f.let_("decoded", body(decoder));
                    f.ret_ok(tup(vec![v("tag"), with_id(type_name, case, v("decoded"))]));
                });
            }
            f.ret_err(code(TOOL_KIND_OUT_OF_DOMAIN));
        },
    );
}
