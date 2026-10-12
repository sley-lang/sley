//! `ty_*`: parsing and constructing canonical SCB1 `TypeExpr` bytes.
//!
//! A type is its canonical encoding: `uvar(tag) || uvar(len) || payload`.
//! Payloads by tag: empty for `Unit Bool F32 F64 Bytes Text`; `uvar(width)`
//! for `SInt`/`UInt` (3/4); a `List<TypeExpr>` for `Tuple` (9); the record
//! `NamedType{1: id, 2: List<TypeExpr>}` for `Named` (10); one `TypeExpr`
//! for `Vector`/`Option`/`LocalCell` (11/13/18); the record `{1: TypeExpr,
//! 2: TypeExpr}` for `OrderedMap`/`Result` (12/14); `FunctionType{1:
//! List<TypeExpr>, 2: TypeExpr, 3: Set<EntityId>}` for `FunctionRef` (15);
//! 32 raw bytes for `AdapterHandle`/`CapabilityToken` (16/17); `uvar(u32)`
//! for `TypeParameter` (19); `uvar(kind)` for `BuiltinFailure` (20).
//!
//! Every function here assumes its input already passed `scb_walk`
//! (canonical, well framed); a malformed input traps.

use sley_ssmc::{BuiltinFailureKind, ConstData, ConstValue, IntegerWidth, TypeExpr};

use super::rt::rcall;
use crate::seed::*;

/// Canonical SCB1 bytes of one `TypeExpr` (seed side).
#[must_use]
pub fn scb_type_bytes(ty: &TypeExpr) -> Vec<u8> {
    let encoded = sley_mutate::encode_const_value(&ConstValue {
        value_type: ty.clone(),
        data: ConstData::Unit,
    })
    .expect("encodes");
    // ConstValue record: uvar(2) || uvar(1) || len(type) || type || ...
    let mut cursor = sley_scb1::ScbValueCursor::new(&encoded).expect("cursor");
    cursor.read_record_field_count().expect("count");
    cursor.read_uvar(32).expect("tag");
    cursor.read_sized_payload().expect("payload").to_vec()
}

/// A type-bytes literal.
#[must_use]
pub fn ty_lit(ty: &TypeExpr) -> E {
    bytesc(&scb_type_bytes(ty))
}

/// `UInt(64)` type bytes.
#[must_use]
pub fn ty_u64() -> E {
    ty_lit(&TypeExpr::UInt(IntegerWidth::from_bits(64)))
}

/// `Bool` type bytes.
#[must_use]
pub fn ty_bool() -> E {
    ty_lit(&TypeExpr::Bool)
}

/// `BuiltinFailure(kind)` type bytes.
#[must_use]
pub fn ty_failure(kind: BuiltinFailureKind) -> E {
    ty_lit(&TypeExpr::BuiltinFailure(kind))
}

fn head_ty() -> Ty {
    t::tuple(vec![t::u64(), t::bytes()])
}

/// Defines the `ty_*` functions.
#[allow(clippy::too_many_lines)]
pub fn define(m: &mut Module) {
    // (tag, payload) of one type.
    m.func("ty_parts", &[("ty", t::bytes())], head_ty(), |f| {
        f.let_("b", b2v(v("ty")));
        f.var("p", u64c(0));
        f.let_("e", vlen(v("b")));
        f.let_("#rd", unwrap(rcall("rt_rd_uvar", vec![u32c(32)])));
        f.let_("tag", tget(v("#rd"), 0));
        f.set("p", tget(v("#rd"), 1));
        f.let_("#rd2", unwrap(rcall("rt_rd_payload", vec![])));
        f.let_("start", tget(v("#rd2"), 0));
        f.ret(tup(vec![
            v("tag"),
            slice(v("ty"), v("start"), sub(tget(v("#rd2"), 1), v("start"))),
        ]));
    });
    m.func("ty_tag", &[("ty", t::bytes())], t::u64(), |f| {
        f.ret(tget(call("ty_parts", vec![v("ty")]), 0));
    });
    m.func("ty_payload", &[("ty", t::bytes())], t::bytes(), |f| {
        f.ret(tget(call("ty_parts", vec![v("ty")]), 1));
    });
    // Elements of a List<TypeExpr> payload.
    m.func("ty_list", &[("x", t::bytes())], t::vec(t::bytes()), |f| {
        f.let_("b", b2v(v("x")));
        f.var("p", u64c(0));
        f.let_("e", vlen(v("b")));
        f.let_("#c", unwrap(rcall("rt_rd_count", vec![])));
        f.set("p", tget(v("#c"), 1));
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("i", u64c(0), tget(v("#c"), 0), |f| {
            f.let_("#r", unwrap(rcall("rt_rd_payload", vec![])));
            f.let_("s", tget(v("#r"), 0));
            f.set("p", tget(v("#r"), 1));
            f.set("out", push(v("out"), slice(v("x"), v("s"), sub(v("p"), v("s")))));
        });
        f.ret(v("out"));
    });
    // Field payloads of a record payload, in tag order.
    m.func("ty_fields", &[("x", t::bytes())], t::vec(t::bytes()), |f| {
        f.let_("b", b2v(v("x")));
        f.var("p", u64c(0));
        f.let_("e", vlen(v("b")));
        f.let_("#c", unwrap(rcall("rt_rd_fields", vec![])));
        f.set("p", tget(v("#c"), 1));
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("i", u64c(0), tget(v("#c"), 0), |f| {
            f.let_("#t", unwrap(rcall("rt_rd_uvar", vec![u32c(32)])));
            f.set("p", tget(v("#t"), 1));
            f.let_("#r", unwrap(rcall("rt_rd_payload", vec![])));
            f.let_("s", tget(v("#r"), 0));
            f.set("p", tget(v("#r"), 1));
            f.set("out", push(v("out"), slice(v("x"), v("s"), sub(v("p"), v("s")))));
        });
        f.ret(v("out"));
    });
    // A uvar payload (integer width, type parameter, failure kind).
    m.func("ty_uvar_payload", &[("x", t::bytes())], t::u64(), |f| {
        f.let_("b", b2v(v("x")));
        f.let_("p", u64c(0));
        f.let_("e", vlen(v("b")));
        f.ret(tget(unwrap(rcall("rt_rd_uvar", vec![u32c(64)])), 0));
    });
    // Constructors.
    m.func(
        "ty_make",
        &[("tag", t::u64()), ("payload", t::bytes())],
        t::bytes(),
        |f| {
            f.ret(concat(vnew(
                t::bytes(),
                vec![
                    call("uvarb", vec![v("tag")]),
                    call("uvarb", vec![call("rt_blen", vec![v("payload")])]),
                    v("payload"),
                ],
            )));
        },
    );
    // A two-field record payload `{1: a, 2: b}`.
    m.func(
        "ty_rec2",
        &[("a", t::bytes()), ("b", t::bytes())],
        t::bytes(),
        |f| {
            f.ret(concat(vnew(
                t::bytes(),
                vec![
                    call("uvarb", vec![u64c(2)]),
                    call("uvarb", vec![u64c(1)]),
                    call("uvarb", vec![call("rt_blen", vec![v("a")])]),
                    v("a"),
                    call("uvarb", vec![u64c(2)]),
                    call("uvarb", vec![call("rt_blen", vec![v("b")])]),
                    v("b"),
                ],
            )));
        },
    );
    // A List<TypeExpr> payload.
    m.func("ty_list_bytes", &[("items", t::vec(t::bytes()))], t::bytes(), |f| {
        f.var("parts", vnew(t::bytes(), vec![call("uvarb", vec![vlen(v("items"))])]));
        f.for_u64("i", u64c(0), vlen(v("items")), |f| {
            f.let_("item", vget(v("items"), v("i")));
            f.set(
                "parts",
                push(v("parts"), call("uvarb", vec![call("rt_blen", vec![v("item")])])),
            );
            f.set("parts", push(v("parts"), v("item")));
        });
        f.ret(concat(v("parts")));
    });
    m.func("ty_vec", &[("x", t::bytes())], t::bytes(), |f| {
        f.ret(call("ty_make", vec![u64c(11), v("x")]));
    });
    m.func("ty_opt", &[("x", t::bytes())], t::bytes(), |f| {
        f.ret(call("ty_make", vec![u64c(13), v("x")]));
    });
    m.func("ty_cell", &[("x", t::bytes())], t::bytes(), |f| {
        f.ret(call("ty_make", vec![u64c(18), v("x")]));
    });
    m.func(
        "ty_res",
        &[("ok", t::bytes()), ("err", t::bytes())],
        t::bytes(),
        |f| {
            f.ret(call(
                "ty_make",
                vec![u64c(14), call("ty_rec2", vec![v("ok"), v("err")])],
            ));
        },
    );
    m.func(
        "ty_map",
        &[("k", t::bytes()), ("v", t::bytes())],
        t::bytes(),
        |f| {
            f.ret(call(
                "ty_make",
                vec![u64c(12), call("ty_rec2", vec![v("k"), v("v")])],
            ));
        },
    );
    m.func("ty_tuple", &[("items", t::vec(t::bytes()))], t::bytes(), |f| {
        f.ret(call(
            "ty_make",
            vec![u64c(9), call("ty_list_bytes", vec![v("items")])],
        ));
    });
    // A non-generic Named instantiation of `def`.
    m.func("ty_named0", &[("def", t::bytes())], t::bytes(), |f| {
        f.ret(call(
            "ty_make",
            vec![
                u64c(10),
                call(
                    "ty_rec2",
                    vec![v("def"), call("ty_list_bytes", vec![vnew(t::bytes(), vec![])])],
                ),
            ],
        ));
    });
    // Accessors on composite types.
    m.func("ty_inner", &[("ty", t::bytes())], t::bytes(), |f| {
        f.ret(call("ty_payload", vec![v("ty")]));
    });
    m.func("ty_first", &[("ty", t::bytes())], t::bytes(), |f| {
        f.ret(vget(call("ty_fields", vec![call("ty_payload", vec![v("ty")])]), u64c(0)));
    });
    m.func("ty_second", &[("ty", t::bytes())], t::bytes(), |f| {
        f.ret(vget(call("ty_fields", vec![call("ty_payload", vec![v("ty")])]), u64c(1)));
    });
    m.func("ty_tuple_items", &[("ty", t::bytes())], t::vec(t::bytes()), |f| {
        f.ret(call("ty_list", vec![call("ty_payload", vec![v("ty")])]));
    });
    // (definition id, arguments) of a Named type.
    m.func(
        "ty_named_parts",
        &[("ty", t::bytes())],
        t::tuple(vec![t::bytes(), t::vec(t::bytes())]),
        |f| {
            f.let_("fs", call("ty_fields", vec![call("ty_payload", vec![v("ty")])]));
            f.ret(tup(vec![
                vget(v("fs"), u64c(0)),
                call("ty_list", vec![vget(v("fs"), u64c(1))]),
            ]));
        },
    );
    // Integer width of SInt/UInt.
    m.func("ty_width", &[("ty", t::bytes())], t::u64(), |f| {
        f.ret(call("ty_uvar_payload", vec![call("ty_payload", vec![v("ty")])]));
    });
}

#[cfg(test)]
mod tests {
    use super::scb_type_bytes;
    use sley_ssmc::{IntegerWidth, TypeExpr};

    #[test]
    fn literal_shapes() {
        assert_eq!(scb_type_bytes(&TypeExpr::Unit), vec![1, 0]);
        assert_eq!(
            scb_type_bytes(&TypeExpr::UInt(IntegerWidth::from_bits(64))),
            vec![4, 1, 64]
        );
    }
}
