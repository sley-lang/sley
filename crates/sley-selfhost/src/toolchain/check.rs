//! `ck_*`: type-environment and function-graph validation (`TYPE_*`, `GRAPH_*`, `CFG_*`).
//!
//! - `ck_types(inv)` is exactly `sley_check::TypeEnvironment::new` over
//!   `inv.tdefs`: shapes, member duplicates, member well-formedness, the
//!   depth-first cycle walk, and the shared map-key walk.
//! - `ck_function(inv, fid)` is exactly `sley_check::cfg::validate_function_graph`
//!   over the function's owned inventory (the `sley-vm` lowering narrowing),
//!   assuming the type environment is valid (`ck_types` passed). `ck_all`
//!   runs `ck_types` and then every function with shared indices and memos.
//!
//! Native-order notes:
//!
//! - Type walks are emulated exactly with explicit stacks: the map-key and
//!   trait walks share one visit budget (1,048,576) and `(NamedType, depth)`
//!   caches per walk, like the native `TypeWalk`. Whole `check_type`
//!   outcomes are memoized per `(parameter count, type bytes)`; a fresh
//!   native walk per call is deterministic, so the memo is exact.
//! - Native `BTreeMap` iteration (sorted identities) is reproduced without
//!   sorting: each element's failure is independent, so the first failure
//!   in sorted order is the failing element with the smallest identity.
//! - Dominance uses Cooper–Harvey–Kennedy over a reverse postorder plus
//!   dominator-tree interval numbers. The native iterative bitset budget
//!   (50,000,000 word operations, 4,096 rounds) is reproduced exactly: the
//!   native round count is `L + 1`, where `L` is the largest
//!   `1 + dist_{G - m}(n)` over reachable `n` and non-dominators `m` with
//!   retreating (declared-order) edges costing 1 (edges from the entry
//!   cost 0). Back edges never lie on an acyclic entry path, so the round
//!   count is at most 2 plus the distinct targets of retreating non-back
//!   edges; that bound settles every function the seed assembler emits,
//!   and only pathological orderings run the exact 0-1 BFS computation.
//! - Facts the decoder already guarantees are not re-checked: identities
//!   strictly ascend (no duplicate definitions or cross-kind duplicates),
//!   and every `Set<EntityId>` (effects, contracts, invariants) is sorted
//!   and unique, so `TYPE_SET_ORDER` and the sorted-set
//!   `GRAPH_INVENTORY_MISMATCH` checks cannot fire.

use sley_ssmc::{ConstData, ConstValue, MapEntryConst};

use super::inv::{INV, KIND_OPERATION, KIND_PARAMETER};
use super::model::*;
use super::rt::{code, r};
use super::types::ty_bool;
use crate::seed::*;

/// `TYPE_DEPTH_LIMIT`.
pub const TYPE_DEPTH_LIMIT: u32 = 21_000;
/// `TYPE_WIDTH_INVALID`.
pub const TYPE_WIDTH_INVALID: u32 = 21_001;
/// `TYPE_PARAMETER_OUT_OF_SCOPE`.
pub const TYPE_PARAMETER_OUT_OF_SCOPE: u32 = 21_002;
/// `TYPE_ARGUMENT_LIMIT`.
pub const TYPE_ARGUMENT_LIMIT: u32 = 21_003;
/// `TYPE_ARGUMENT_ARITY`.
pub const TYPE_ARGUMENT_ARITY: u32 = 21_004;
/// `TYPE_DEFINITION_UNKNOWN`.
pub const TYPE_DEFINITION_UNKNOWN: u32 = 21_005;
/// `TYPE_DEFINITION_DUPLICATE`.
pub const TYPE_DEFINITION_DUPLICATE: u32 = 21_006;
/// `TYPE_DEFINITION_CYCLE`.
pub const TYPE_DEFINITION_CYCLE: u32 = 21_007;
/// `TYPE_MEMBER_DUPLICATE`.
pub const TYPE_MEMBER_DUPLICATE: u32 = 21_008;
/// `TYPE_SET_ORDER`.
pub const TYPE_SET_ORDER: u32 = 21_010;
/// `TYPE_NOT_ORDERABLE`.
pub const TYPE_NOT_ORDERABLE: u32 = 21_011;
/// `TYPE_NOT_HASHABLE`.
pub const TYPE_NOT_HASHABLE: u32 = 21_012;
/// `TYPE_NOT_PERSISTABLE`.
pub const TYPE_NOT_PERSISTABLE: u32 = 21_013;
/// `TYPE_RESOURCE_LIMIT`.
pub const TYPE_RESOURCE_LIMIT: u32 = 21_018;

/// `GRAPH_DUPLICATE_ENTITY`.
pub const GRAPH_DUPLICATE_ENTITY: u32 = 22_000;
/// `GRAPH_INVENTORY_MISMATCH`.
pub const GRAPH_INVENTORY_MISMATCH: u32 = 22_001;
/// `GRAPH_OWNER_MISMATCH`.
pub const GRAPH_OWNER_MISMATCH: u32 = 22_002;
/// `GRAPH_ORDINAL_MISMATCH`.
pub const GRAPH_ORDINAL_MISMATCH: u32 = 22_003;
/// `GRAPH_UNRESOLVED_REFERENCE`.
pub const GRAPH_UNRESOLVED_REFERENCE: u32 = 22_004;
/// `CFG_ENTRY_INVALID`.
pub const CFG_ENTRY_INVALID: u32 = 22_005;
/// `CFG_TARGET_INVALID`.
pub const CFG_TARGET_INVALID: u32 = 22_006;
/// `CFG_TARGET_ARGUMENTS`.
pub const CFG_TARGET_ARGUMENTS: u32 = 22_007;
/// `CFG_RETURN_TYPE`.
pub const CFG_RETURN_TYPE: u32 = 22_008;
/// `CFG_BOOL_REQUIRED`.
pub const CFG_BOOL_REQUIRED: u32 = 22_009;
/// `CFG_SWITCH_TYPE`.
pub const CFG_SWITCH_TYPE: u32 = 22_010;
/// `CFG_SWITCH_CASES`.
pub const CFG_SWITCH_CASES: u32 = 22_011;
/// `CFG_SWITCH_PAYLOAD`.
pub const CFG_SWITCH_PAYLOAD: u32 = 22_012;
/// `CFG_VALUE_UNRESOLVED`.
pub const CFG_VALUE_UNRESOLVED: u32 = 22_013;
/// `CFG_RESULT_INDEX`.
pub const CFG_RESULT_INDEX: u32 = 22_014;
/// `CFG_USE_BEFORE_DEFINITION`.
pub const CFG_USE_BEFORE_DEFINITION: u32 = 22_015;
/// `CFG_DOMINANCE`.
pub const CFG_DOMINANCE: u32 = 22_016;
/// `CFG_REACHABILITY`.
pub const CFG_REACHABILITY: u32 = 22_017;
/// `CFG_UNREACHABLE_VALUE`.
pub const CFG_UNREACHABLE_VALUE: u32 = 22_018;
/// `CFG_TRAP_PAYLOAD`.
pub const CFG_TRAP_PAYLOAD: u32 = 22_019;
/// `CFG_RESOURCE_LIMIT`.
pub const CFG_RESOURCE_LIMIT: u32 = 22_020;

/// Native limits.
const MAX_TYPE_DEPTH: u64 = 64;
const MAX_TYPE_ARGUMENTS: u64 = 1_024;
const MAX_MEMBERS: u64 = 65_535;
const MAX_WALK_VISITS: u64 = 1_048_576;
const MAX_DEFINITIONS: u64 = 1_000_000;
const MAX_CFG_BLOCKS: u64 = 4_096;
const MAX_CFG_EDGES: u64 = 16_384;
const MAX_CFG_OPERATIONS: u64 = 1_000_000;
const MAX_CFG_VALUES: u64 = 1_000_000;
const MAX_CFG_USES: u64 = 262_144;
const MAX_CFG_PARAMETERS: u64 = 65_535;
const MAX_DOM_WORK: u64 = 50_000_000;
/// "Infinite" distance / postorder sentinel.
const INF: u64 = 1 << 62;

/// Trait bits: equality 1, total order 2, canonical hash 4, persistable 8.
const TR_ALL: u64 = 15;
const TR_NO_ORDER: u64 = 13;

/// Walk state (`TypeWalk`): visits and the `(depth, NamedType)` caches.
pub const CK_W: &str = "CkW";
/// Memo: `check_type` outcomes, trap-payload traits, definition arities.
pub const CK_M: &str = "CkM";
/// Per-function inventory aggregates.
pub const CK_IX: &str = "CkIx";
/// Local parameter tables of one function.
pub const CK_P: &str = "CkP";
/// Local operation tables of one function.
pub const CK_O: &str = "CkO";
/// Reachability and dominator-tree numbers.
pub const CK_C: &str = "CkC";
/// One function's resolution context.
pub const CK_G: &str = "CkG";

fn fail_if(f: &mut FnB<'_>, cond: E, c: u32) {
    f.if_(cond, move |f| f.ret_err(code(c)));
}

fn any_tag(x: &str, tags: &[u64]) -> E {
    let mut out = eq(v(x), u64c(tags[0]));
    for tag in &tags[1..] {
        out = or(out, eq(v(x), u64c(*tag)));
    }
    out
}

fn inc(f: &mut FnB<'_>, name: &str) {
    f.set(name, add(v(name), u64c(1)));
}

/// `vec[idx] = val` for a `Vector<Bytes>`, appending when `idx ==
/// len(vec)`. Stack writes are calls, not splits: every split threads all
/// live variables through new blocks.
fn putb(f: &mut FnB<'_>, vec: &str, idx: E, val: E) {
    f.set(vec, call("ck_putb", vec![v(vec), idx, val]));
}

/// `putb` for a `Vector<UInt(64)>`.
fn putu(f: &mut FnB<'_>, vec: &str, idx: E, val: E) {
    f.set(vec, call("ck_putu", vec![v(vec), idx, val]));
}

fn upd(f: &mut FnB<'_>, vec: &str, idx: E, val: E) {
    f.set(vec, vset(v(vec), idx, val));
}

fn at(vec: &str, idx: E) -> E {
    vget(v(vec), idx)
}

/// `vec[base + k]`.
fn at2(vec: &str, base: E, k: u64) -> E {
    vget(v(vec), add(base, u64c(k)))
}

fn uint(value: u128) -> ConstValue {
    ConstValue {
        value_type: t::u64(),
        data: ConstData::UInt(value),
    }
}

/// `a & b` over 4-bit trait masks, as a 256-entry constant map.
fn tand_table() -> E {
    let mut entries: Vec<(ConstValue, ConstValue)> = (0..256_u128)
        .map(|k| (uint(k), uint((k / 16) & (k % 16))))
        .collect();
    entries.sort_by_key(|(k, _)| sley_mutate::encode_const_value(k).expect("canonical key"));
    E::Lit(ConstValue {
        value_type: t::map(t::u64(), t::u64()),
        data: ConstData::Map(
            entries
                .into_iter()
                .map(|(key, value)| MapEntryConst { key, value })
                .collect(),
        ),
    })
}

/// The `(depth, NamedType)` cache key.
fn walk_key(d: E, ty: E) -> E {
    concat(vnew(t::bytes(), vec![call("uvarb", vec![d]), ty]))
}

fn u() -> Ty {
    t::vec(t::u64())
}

fn named(name: &str) -> Ty {
    Module::new(super::TOOLCHAIN_SEED).ty(name)
}

/// Defines the `ck_*` functions.
pub fn define(m: &mut Module) {
    declare(m);
    define_types(m);
    define_walk(m);
    define_env(m);
    define_index(m);
    define_cfg(m);
    define_function(m);
    define_entries(m);
}

fn declare(m: &mut Module) {
    m.record(
        CK_W,
        &[
            ("visits", t::u64()),
            ("mkc", t::map(t::bytes(), t::bool())),
            ("trc", t::map(t::bytes(), t::u64())),
        ],
    );
    m.record(
        CK_M,
        &[
            // uvar(pc) || type -> check_type code (0 = ok).
            ("ct", t::map(t::bytes(), t::u32())),
            // type -> (traits code, traits mask).
            ("tr", t::map(t::bytes(), t::tuple(vec![t::u32(), t::u64()]))),
            // definition id -> type parameter count.
            ("ar", t::map(t::bytes(), t::u64())),
        ],
    );
    let agg = || t::map(t::bytes(), t::u64());
    m.record(
        CK_IX,
        &[
            ("b2f", t::map(t::bytes(), t::bytes())),
            ("nb", agg()),
            ("np", agg()),
            ("no", agg()),
            ("nv", agg()),
            ("nu", agg()),
            ("big", agg()),
        ],
    );
    m.record(
        CK_P,
        &[
            // parameter id -> local index k.
            ("pm", t::map(t::bytes(), t::u64())),
            ("pty", t::vec(t::bytes())),
            // stride 2 per parameter: role, owner block position (nfb for
            // the function).
            ("pi", u()),
            // stride 2 per block: first local parameter index, count.
            ("bp", u()),
        ],
    );
    m.record(
        CK_O,
        &[
            // operation id -> local index q.
            ("om", t::map(t::bytes(), t::u64())),
            // stride 6 per operation: block position, ordinal, first result
            // type, result count, first operand, operand count.
            ("oi", u()),
            // stride 2 per block: first local operation index, count.
            ("ob", u()),
            ("rt", t::vec(t::bytes())),
            ("od", t::vec(m.ty(VREF))),
        ],
    );
    m.record(CK_C, &[("reach", u()), ("dpre", u()), ("dpost", u())]);
    m.record(
        CK_G,
        &[
            ("bix", t::map(t::bytes(), t::u64())),
            ("p", m.ty(CK_P)),
            ("o", m.ty(CK_O)),
            ("c", m.ty(CK_C)),
        ],
    );
}

fn node_ty() -> Ty {
    t::tuple(vec![t::u64(), t::vec(t::bytes()), t::bytes(), t::u64()])
}

/// Type-level helpers: node decoding, `check_type_inner`, substitution.
#[allow(clippy::too_many_lines)]
fn define_types(m: &mut Module) {
    for (name, elem) in [("ck_putb", t::bytes()), ("ck_putu", t::u64())] {
        m.func(
            name,
            &[("xs", t::vec(elem.clone())), ("i", t::u64()), ("x", elem.clone())],
            t::vec(elem),
            |f| {
                f.if_(lt(v("i"), vlen(v("xs"))), |f| f.ret(vset(v("xs"), v("i"), v("x"))));
                f.ret(push(v("xs"), v("x")));
            },
        );
    }
    // Writes `ys` at `xs[i..]` (appending past the end).
    m.func(
        "ck_putall",
        &[("xs", t::vec(t::bytes())), ("i", t::u64()), ("ys", t::vec(t::bytes()))],
        t::vec(t::bytes()),
        |f| {
            f.var("xs", v("xs"));
            f.for_u64("k", u64c(0), vlen(v("ys")), |f| {
                f.set(
                    "xs",
                    call("ck_putb", vec![v("xs"), add(v("i"), v("k")), at("ys", v("k"))]),
                );
            });
            f.ret(v("xs"));
        },
    );
    // Writes `n` copies of `x` at `xs[i..]`.
    m.func(
        "ck_putrep",
        &[("xs", u()), ("i", t::u64()), ("n", t::u64()), ("x", t::u64())],
        u(),
        |f| {
            f.var("xs", v("xs"));
            f.for_u64("k", u64c(0), v("n"), |f| {
                f.set("xs", call("ck_putu", vec![v("xs"), add(v("i"), v("k")), v("x")]));
            });
            f.ret(v("xs"));
        },
    );
    // Grows `xs` with zeros to at least `n` entries.
    m.func("ck_grow", &[("xs", u()), ("n", t::u64())], u(), |f| {
        f.var("xs", v("xs"));
        f.while_(lt(vlen(v("xs")), v("n")), |f| f.set("xs", push(v("xs"), u64c(0))));
        f.ret(v("xs"));
    });
    // Reversed.
    m.func("ck_rev", &[("xs", t::vec(t::bytes()))], t::vec(t::bytes()), |f| {
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("k", u64c(0), vlen(v("xs")), |f| {
            f.set("out", push(v("out"), at("xs", sub(sub(vlen(v("xs")), v("k")), u64c(1)))));
        });
        f.ret(v("out"));
    });
    // (tag, children, extra, number): children in the native recursion
    // order (Tuple items, Named arguments, Vector/Option/LocalCell element,
    // OrderedMap key/value, Result ok/error, FunctionRef parameters then
    // result). extra: the Named definition id or the FunctionRef effects
    // payload; number: an integer width or a type-parameter index.
    m.func("ck_node", &[("ty", t::bytes())], node_ty(), |f| {
        f.let_("parts", call("ty_parts", vec![v("ty")]));
        f.let_("tag", tget(v("parts"), 0));
        f.let_("pl", tget(v("parts"), 1));
        let out = |f: &mut FnB<'_>, kids: E, x: E, num: E| {
            f.ret(tup(vec![v("tag"), kids, x, num]));
        };
        let none = || vnew(t::bytes(), vec![]);
        f.if_(any_tag("tag", &[3, 4, 19]), move |f| {
            out(f, none(), bytesc(&[]), call("ty_uvar_payload", vec![v("pl")]));
        });
        f.if_(eq(v("tag"), u64c(9)), move |f| {
            out(f, call("ty_list", vec![v("pl")]), bytesc(&[]), u64c(0));
        });
        f.if_(any_tag("tag", &[11, 13, 18]), move |f| {
            out(f, vnew(t::bytes(), vec![v("pl")]), bytesc(&[]), u64c(0));
        });
        f.if_(any_tag("tag", &[12, 14]), move |f| {
            out(f, call("ty_fields", vec![v("pl")]), bytesc(&[]), u64c(0));
        });
        f.if_(eq(v("tag"), u64c(10)), move |f| {
            f.let_("fs", call("ty_fields", vec![v("pl")]));
            out(f, call("ty_list", vec![vget(v("fs"), u64c(1))]), vget(v("fs"), u64c(0)), u64c(0));
        });
        f.if_(eq(v("tag"), u64c(15)), move |f| {
            f.let_("fs", call("ty_fields", vec![v("pl")]));
            f.let_("l0", call("ty_list", vec![vget(v("fs"), u64c(0))]));
            out(f, push(v("l0"), vget(v("fs"), u64c(1))), vget(v("fs"), u64c(2)), u64c(0));
        });
        out(f, none(), bytesc(&[]), u64c(0));
    });
    // (nesting levels, contains a type parameter) over the native
    // recursion children.
    m.func(
        "ck_info",
        &[("ty", t::bytes())],
        t::tuple(vec![t::u64(), t::bool()]),
        |f| {
            // Stack of (type, depth) pairs.
            f.var("ts", vnew(t::bytes(), vec![v("ty")]));
            f.var("ds", vnew(t::u64(), vec![u64c(1)]));
            f.var("sp", u64c(1));
            f.var("nest", u64c(0));
            f.var("hp", boolc(false));
            f.while_(gt(v("sp"), u64c(0)), |f| {
                f.set("sp", sub(v("sp"), u64c(1)));
                f.let_("d", at("ds", v("sp")));
                f.set("nest", select(gt(v("d"), v("nest")), v("d"), v("nest")));
                f.let_("n", call("ck_node", vec![at("ts", v("sp"))]));
                f.set("hp", or(v("hp"), eq(tget(v("n"), 0), u64c(19))));
                f.let_("kids", tget(v("n"), 1));
                f.set("ts", call("ck_putall", vec![v("ts"), v("sp"), v("kids")]));
                f.set(
                    "ds",
                    call("ck_putrep", vec![v("ds"), v("sp"), vlen(v("kids")), add(v("d"), u64c(1))]),
                );
                f.set("sp", add(v("sp"), vlen(v("kids"))));
            });
            f.ret(tup(vec![v("nest"), v("hp")]));
        },
    );
    // One `check_type_inner` node: the node's own failure code (0 = ok).
    m.func(
        "ck_inner1",
        &[
            ("ar", t::map(t::bytes(), t::u64())),
            ("n", node_ty()),
            ("pc", t::u64()),
        ],
        t::u32(),
        |f| {
            f.let_("tag", tget(v("n"), 0));
            f.let_("nk", vlen(tget(v("n"), 1)));
            f.let_("num", tget(v("n"), 3));
            let give = |f: &mut FnB<'_>, cond: E, c: u32| f.if_(cond, move |f| f.ret(code(c)));
            give(
                f,
                and(any_tag("tag", &[3, 4]), not(any_tag("num", &[8, 16, 32, 64, 128]))),
                TYPE_WIDTH_INVALID,
            );
            give(f, and(eq(v("tag"), u64c(9)), gt(v("nk"), u64c(MAX_MEMBERS))), TYPE_RESOURCE_LIMIT);
            f.if_(eq(v("tag"), u64c(10)), move |f| {
                give(f, gt(v("nk"), u64c(MAX_TYPE_ARGUMENTS)), TYPE_ARGUMENT_LIMIT);
                f.match_opt(
                    mget_opt(v("ar"), tget(v("n"), 2)),
                    "arity",
                    move |f| give(f, ne(v("arity"), v("nk")), TYPE_ARGUMENT_ARITY),
                    |f| f.ret(code(TYPE_DEFINITION_UNKNOWN)),
                );
            });
            give(
                f,
                and(eq(v("tag"), u64c(15)), gt(v("nk"), u64c(MAX_MEMBERS + 1))),
                TYPE_RESOURCE_LIMIT,
            );
            give(
                f,
                and(eq(v("tag"), u64c(19)), ge(v("num"), v("pc"))),
                TYPE_PARAMETER_OUT_OF_SCOPE,
            );
            f.ret(u32c(0));
        },
    );
    // `check_type_inner(ty, pc, 1)`: pre-order, first failure.
    m.func(
        "ck_inner",
        &[
            ("ar", t::map(t::bytes(), t::u64())),
            ("ty", t::bytes()),
            ("pc", t::u64()),
        ],
        r(t::unit()),
        |f| {
            f.var("ts", vnew(t::bytes(), vec![v("ty")]));
            f.var("ds", vnew(t::u64(), vec![u64c(1)]));
            f.var("sp", u64c(1));
            f.while_(gt(v("sp"), u64c(0)), |f| {
                f.set("sp", sub(v("sp"), u64c(1)));
                f.let_("d", at("ds", v("sp")));
                fail_if(f, gt(v("d"), u64c(MAX_TYPE_DEPTH)), TYPE_DEPTH_LIMIT);
                f.let_("n", call("ck_node", vec![at("ts", v("sp"))]));
                f.let_("c", call("ck_inner1", vec![v("ar"), v("n"), v("pc")]));
                f.if_(ne(v("c"), u32c(0)), |f| f.ret_err(v("c")));
                // Children in reverse so the first child pops first.
                f.let_("kids", tget(v("n"), 1));
                f.set("ts", call("ck_putall", vec![v("ts"), v("sp"), call("ck_rev", vec![v("kids")])]));
                f.set(
                    "ds",
                    call("ck_putrep", vec![v("ds"), v("sp"), vlen(v("kids")), add(v("d"), u64c(1))]),
                );
                f.set("sp", add(v("sp"), vlen(v("kids"))));
            });
            f.ret_ok(unitc());
        },
    );
    // `items[base .. base + n]`.
    m.func(
        "ck_sub",
        &[("items", t::vec(t::bytes())), ("base", t::u64()), ("n", t::u64())],
        t::vec(t::bytes()),
        |f| {
            f.var("out", vnew(t::bytes(), vec![]));
            f.for_u64("k", u64c(0), v("n"), |f| {
                f.set("out", push(v("out"), at("items", add(v("base"), v("k")))));
            });
            f.ret(v("out"));
        },
    );
    // A three-field record payload `{1: a, 2: b, 3: c}`.
    m.func(
        "ck_rec3",
        &[("a", t::bytes()), ("b", t::bytes()), ("c", t::bytes())],
        t::bytes(),
        |f| {
            let mut parts = vec![call("uvarb", vec![u64c(3)])];
            for (tag, name) in [(1, "a"), (2, "b"), (3, "c")] {
                parts.push(call("uvarb", vec![u64c(tag)]));
                parts.push(call("uvarb", vec![call("rt_blen", vec![v(name)])]));
                parts.push(v(name));
            }
            f.ret(concat(vnew(t::bytes(), parts)));
        },
    );
    // Rebuilds a composite type from substituted children.
    m.func(
        "ck_rebuild",
        &[("tag", t::u64()), ("x", t::bytes()), ("items", t::vec(t::bytes()))],
        t::bytes(),
        |f| {
            f.let_("n", vlen(v("items")));
            let make = |payload: E| call("ty_make", vec![v("tag"), payload]);
            f.if_(eq(v("tag"), u64c(9)), |f| f.ret(call("ty_tuple", vec![v("items")])));
            f.if_(eq(v("tag"), u64c(10)), move |f| {
                f.ret(make(call(
                    "ty_rec2",
                    vec![v("x"), call("ty_list_bytes", vec![v("items")])],
                )));
            });
            f.if_(any_tag("tag", &[12, 14]), move |f| {
                f.ret(make(call("ty_rec2", vec![at("items", u64c(0)), at("items", u64c(1))])));
            });
            f.if_(eq(v("tag"), u64c(15)), move |f| {
                f.let_("np", sub(v("n"), u64c(1)));
                f.ret(make(call(
                    "ck_rec3",
                    vec![
                        call(
                            "ty_list_bytes",
                            vec![call("ck_sub", vec![v("items"), u64c(0), v("np")])],
                        ),
                        at("items", v("np")),
                        v("x"),
                    ],
                )));
            });
            // Vector, Option, LocalCell.
            f.ret(make(at("items", u64c(0))));
        },
    );
    // `substitute(ty, args, d)`: rebuilds canonical bytes; failures in
    // native pre-order (the depth check, then the parameter index).
    m.func(
        "ck_subst",
        &[("ty", t::bytes()), ("args", t::vec(t::bytes())), ("d", t::u64())],
        r(t::bytes()),
        |f| {
            // No arguments: only the depth check can fail (every member type
            // reached here is well formed under its definition's parameter
            // count, which equals the argument count). Nesting is at most
            // half the encoded length.
            f.if_(eq(vlen(v("args")), u64c(0)), |f| {
                f.if_(
                    le(
                        add(v("d"), div(call("rt_blen", vec![v("ty")]), u64c(2))),
                        u64c(MAX_TYPE_DEPTH + 1),
                    ),
                    |f| f.ret_ok(v("ty")),
                );
            });
            // Frames, stride 6: depth, step, first child, child count, first
            // output, tag. Plus the type and the extra bytes per frame.
            f.var("fr", vnew(t::u64(), vec![v("d"), u64c(0), u64c(0), u64c(0), u64c(0), u64c(0)]));
            f.var("st", vnew(t::bytes(), vec![v("ty")]));
            f.var("sx", vnew(t::bytes(), vec![bytesc(&[])]));
            f.var("sp", u64c(1));
            f.var("kb", vnew(t::bytes(), vec![]));
            f.var("kp", u64c(0));
            f.var("ob", vnew(t::bytes(), vec![]));
            f.var("op", u64c(0));
            f.loop_(|f| {
                f.let_("top", sub(v("sp"), u64c(1)));
                f.let_("o", mul(v("top"), u64c(6)));
                f.let_("i", at2("fr", v("o"), 1));
                f.var("emit", boolc(false));
                f.var("res", bytesc(&[]));
                f.if_else(
                    eq(v("i"), u64c(0)),
                    |f| {
                        fail_if(f, gt(at("fr", v("o")), u64c(MAX_TYPE_DEPTH)), TYPE_DEPTH_LIMIT);
                        f.let_("t", at("st", v("top")));
                        f.let_("n", call("ck_node", vec![v("t")]));
                        f.let_("tag", tget(v("n"), 0));
                        f.let_("kids", tget(v("n"), 1));
                        f.if_(eq(v("tag"), u64c(19)), |f| {
                            fail_if(
                                f,
                                ge(tget(v("n"), 3), vlen(v("args"))),
                                TYPE_PARAMETER_OUT_OF_SCOPE,
                            );
                        });
                        f.if_else(
                            any_tag("tag", &[9, 10, 11, 12, 13, 14, 15, 18]),
                            |f| {
                                upd(f, "fr", add(v("o"), u64c(1)), u64c(1));
                                upd(f, "fr", add(v("o"), u64c(2)), v("kp"));
                                upd(f, "fr", add(v("o"), u64c(3)), vlen(v("kids")));
                                upd(f, "fr", add(v("o"), u64c(4)), v("op"));
                                upd(f, "fr", add(v("o"), u64c(5)), v("tag"));
                                upd(f, "sx", v("top"), tget(v("n"), 2));
                                f.set("kb", call("ck_putall", vec![v("kb"), v("kp"), v("kids")]));
                                f.set("kp", add(v("kp"), vlen(v("kids"))));
                            },
                            |f| {
                                f.set(
                                    "res",
                                    select(
                                        eq(v("tag"), u64c(19)),
                                        at("args", tget(v("n"), 3)),
                                        v("t"),
                                    ),
                                );
                                f.set("emit", boolc(true));
                            },
                        );
                    },
                    |f| {
                        f.let_("j", sub(v("i"), u64c(1)));
                        f.let_("kn", at2("fr", v("o"), 3));
                        f.if_else(
                            lt(v("j"), v("kn")),
                            |f| {
                                upd(f, "fr", add(v("o"), u64c(1)), add(v("i"), u64c(1)));
                                f.let_("o2", add(v("o"), u64c(6)));
                                f.set("fr", call("ck_grow", vec![v("fr"), add(v("o2"), u64c(6))]));
                                upd(f, "fr", v("o2"), add(at("fr", v("o")), u64c(1)));
                                upd(f, "fr", add(v("o2"), u64c(1)), u64c(0));
                                putb(f, "st", v("sp"), at("kb", add(at2("fr", v("o"), 2), v("j"))));
                                putb(f, "sx", v("sp"), bytesc(&[]));
                                inc(f, "sp");
                            },
                            |f| {
                                f.let_("ob0", at2("fr", v("o"), 4));
                                f.set(
                                    "res",
                                    call(
                                        "ck_rebuild",
                                        vec![
                                            at2("fr", v("o"), 5),
                                            at("sx", v("top")),
                                            call("ck_sub", vec![v("ob"), v("ob0"), v("kn")]),
                                        ],
                                    ),
                                );
                                f.set("op", v("ob0"));
                                f.set("kp", at2("fr", v("o"), 2));
                                f.set("emit", boolc(true));
                            },
                        );
                    },
                );
                f.if_(v("emit"), |f| {
                    f.set("sp", v("top"));
                    f.if_(eq(v("sp"), u64c(0)), |f| f.ret_ok(v("res")));
                    putb(f, "ob", v("op"), v("res"));
                    inc(f, "op");
                });
            });
        },
    );
    // `a & b` over trait masks.
    m.func("ck_tand", &[("a", t::u64()), ("b", t::u64())], t::u64(), |f| {
        f.ret(mget(tand_table(), add(mul(v("a"), u64c(16)), v("b"))));
    });
    // `require_map_key_traits`.
    m.func("ck_req", &[("tr", t::u64())], r(t::unit()), |f| {
        fail_if(f, eq(call("ck_tand", vec![v("tr"), u64c(2)]), u64c(0)), TYPE_NOT_ORDERABLE);
        fail_if(f, eq(call("ck_tand", vec![v("tr"), u64c(1)]), u64c(0)), TYPE_NOT_ORDERABLE);
        fail_if(f, eq(call("ck_tand", vec![v("tr"), u64c(4)]), u64c(0)), TYPE_NOT_HASHABLE);
        fail_if(f, eq(call("ck_tand", vec![v("tr"), u64c(8)]), u64c(0)), TYPE_NOT_PERSISTABLE);
        f.ret_ok(unitc());
    });
    // The member types of a definition (record fields, payload cases).
    m.func("ck_mtypes", &[("td", m.ty(TDEF))], t::vec(t::bytes()), |f| {
        f.let_("ms", field(v("td"), "members"));
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("k", u64c(0), vlen(v("ms")), |f| {
            f.let_("mem", at("ms", v("k")));
            f.if_(field(v("mem"), "has_ty"), |f| {
                f.set("out", push(v("out"), field(v("mem"), "ty")));
            });
        });
        f.ret(v("out"));
    });
    // Every Named definition id reachable through the native recursion.
    m.func("ck_named_ids", &[("ty", t::bytes())], t::vec(t::bytes()), |f| {
        f.var("ts", vnew(t::bytes(), vec![v("ty")]));
        f.var("sp", u64c(1));
        f.var("out", vnew(t::bytes(), vec![]));
        f.while_(gt(v("sp"), u64c(0)), |f| {
            f.set("sp", sub(v("sp"), u64c(1)));
            f.let_("n", call("ck_node", vec![at("ts", v("sp"))]));
            f.if_(eq(tget(v("n"), 0), u64c(10)), |f| {
                f.set("out", push(v("out"), tget(v("n"), 2)));
            });
            f.let_("kids", tget(v("n"), 1));
            f.set("ts", call("ck_putall", vec![v("ts"), v("sp"), v("kids")]));
            f.set("sp", add(v("sp"), vlen(v("kids"))));
        });
        f.ret(v("out"));
    });
    // Stable ascending order of byte strings (insertion sort; small lists).
    m.func("ck_order", &[("ids", t::vec(t::bytes()))], u(), |f| {
        f.var("ix", vnew(t::u64(), vec![]));
        f.for_u64("i", u64c(0), vlen(v("ids")), |f| {
            f.set("ix", push(v("ix"), v("i")));
            f.var("j", v("i"));
            f.loop_(|f| {
                f.if_(eq(v("j"), u64c(0)), |f| f.break_());
                f.let_("pj", at("ix", sub(v("j"), u64c(1))));
                f.if_(le(at("ids", v("pj")), at("ids", v("i"))), |f| f.break_());
                upd(f, "ix", v("j"), v("pj"));
                f.set("j", sub(v("j"), u64c(1)));
            });
            upd(f, "ix", v("j"), v("i"));
        });
        f.ret(v("ix"));
    });
    // Sorted unique byte strings.
    m.func("ck_sortu", &[("ids", t::vec(t::bytes()))], t::vec(t::bytes()), |f| {
        f.let_("ix", call("ck_order", vec![v("ids")]));
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("k", u64c(0), vlen(v("ix")), |f| {
            f.let_("x", at("ids", at("ix", v("k"))));
            f.if_(
                or(
                    eq(v("k"), u64c(0)),
                    ne(at("ids", at("ix", select(gt(v("k"), u64c(0)), sub(v("k"), u64c(1)), u64c(0)))), v("x")),
                ),
                |f| f.set("out", push(v("out"), v("x"))),
            );
        });
        f.ret(v("out"));
    });
    // `n` copies of `x`.
    m.func("ck_fill", &[("n", t::u64()), ("x", t::u64())], u(), |f| {
        f.var("out", vnew(t::u64(), vec![]));
        f.for_u64("k", u64c(0), v("n"), |f| f.set("out", push(v("out"), v("x"))));
        f.ret(v("out"));
    });
}

/// The exact native `TypeWalk`: `check_map_keys_with` (mode 1) and
/// `traits_inner_with` (mode 2) over one shared state.
#[allow(clippy::too_many_lines)]
fn define_walk(m: &mut Module) {
    let inv_ty = m.ty(INV);
    let bmap = || t::map(t::bytes(), t::bool());
    let umap = || t::map(t::bytes(), t::u64());
    // Frame entry after the depth check and visit charge: (action, traits,
    // tag, children then Named arguments, member count). Action 1 returns
    // `traits`; action 2 installs the children.
    m.func(
        "ck_wk_enter",
        &[
            ("inv", inv_ty.clone()),
            ("mkc", bmap()),
            ("trc", umap()),
            ("t", t::bytes()),
            ("mo", t::u64()),
            ("dd", t::u64()),
        ],
        r(t::tuple(vec![t::u64(), t::u64(), t::u64(), t::vec(t::bytes()), t::u64()])),
        |f| {
            f.let_("n", call("ck_node", vec![v("t")]));
            f.let_("tag", tget(v("n"), 0));
            f.let_("kids", tget(v("n"), 1));
            let done = |f: &mut FnB<'_>, traits: E| {
                f.ret_ok(tup(vec![u64c(1), traits, v("tag"), vnew(t::bytes(), vec![]), u64c(0)]));
            };
            let setup = |f: &mut FnB<'_>| {
                f.ret_ok(tup(vec![u64c(2), u64c(0), v("tag"), v("kids"), vlen(v("kids"))]));
            };
            let named = |f: &mut FnB<'_>| {
                f.match_opt(
                    call("inv_tdef", vec![v("inv"), tget(v("n"), 2)]),
                    "td",
                    |f| {
                        f.let_("mt", call("ck_mtypes", vec![v("td")]));
                        f.var("mem", v("mt"));
                        f.for_u64("k", u64c(0), vlen(v("kids")), |f| {
                            f.set("mem", push(v("mem"), at("kids", v("k"))));
                        });
                        f.ret_ok(tup(vec![u64c(2), u64c(0), v("tag"), v("mem"), vlen(v("mt"))]));
                    },
                    |f| f.ret_err(code(TYPE_DEFINITION_UNKNOWN)),
                );
            };
            f.if_(eq(v("mo"), u64c(1)), move |f| {
                f.if_(eq(v("tag"), u64c(10)), move |f| {
                    f.for_u64("k", u64c(0), vlen(v("kids")), move |f| {
                        f.if_(tget(call("ck_info", vec![at("kids", v("k"))]), 1), move |f| {
                            done(f, u64c(0));
                        });
                    });
                    f.if_(mhas(v("mkc"), walk_key(v("dd"), v("t"))), move |f| done(f, u64c(0)));
                    named(f);
                });
                f.if_(any_tag("tag", &[9, 11, 12, 13, 14, 15, 18]), setup);
                done(f, u64c(0));
            });
            f.if_(any_tag("tag", &[1, 2, 3, 4, 7, 8, 20]), move |f| done(f, u64c(TR_ALL)));
            f.if_(any_tag("tag", &[5, 6, 15]), move |f| done(f, u64c(TR_NO_ORDER)));
            f.if_(any_tag("tag", &[16, 17]), move |f| done(f, u64c(1)));
            fail_if(f, eq(v("tag"), u64c(19)), TYPE_PARAMETER_OUT_OF_SCOPE);
            f.if_(eq(v("tag"), u64c(10)), move |f| {
                f.match_opt(
                    mget_opt(v("trc"), walk_key(v("dd"), v("t"))),
                    "cached",
                    move |f| done(f, v("cached")),
                    named,
                );
            });
            f.if_(any_tag("tag", &[9, 11, 12, 13, 14]), setup);
            // LocalCell.
            done(f, u64c(0));
        },
    );
    // Frame continuation with `j` children done: (action, traits, acc,
    // child type, child mode, cache). Action 1 returns `traits`; action 2
    // enters the child. Cache 1 records the map-key key, 2 the traits key.
    m.func(
        "ck_wk_next",
        &[
            ("mo", t::u64()),
            ("tag", t::u64()),
            ("j", t::u64()),
            ("kn", t::u64()),
            ("an", t::u64()),
            ("base", t::u64()),
            ("acc", t::u64()),
            ("ret", t::u64()),
            ("dd", t::u64()),
            ("kb", t::vec(t::bytes())),
        ],
        r(t::tuple(vec![t::u64(), t::u64(), t::u64(), t::bytes(), t::u64(), t::u64()])),
        |f| {
            let pop = |f: &mut FnB<'_>, traits: E, acc: E, cache: E| {
                f.ret_ok(tup(vec![u64c(1), traits, acc, bytesc(&[]), u64c(0), cache]));
            };
            let child = |f: &mut FnB<'_>, ty: E, mode: E, acc: E| {
                f.ret_ok(tup(vec![u64c(2), v("ret"), acc, ty, mode, u64c(0)]));
            };
            // The j-th member, substituted for Named definitions.
            let member = |f: &mut FnB<'_>| {
                f.let_("raw", at("kb", add(v("base"), v("j"))));
                f.if_(ne(v("tag"), u64c(10)), |f| {
                    f.ret_ok(tup(vec![u64c(2), v("ret"), v("a"), v("raw"), v("mo"), u64c(0)]));
                });
                f.let_(
                    "sub",
                    tri(call(
                        "ck_subst",
                        vec![
                            v("raw"),
                            call("ck_sub", vec![v("kb"), add(v("base"), v("kn")), v("an")]),
                            add(v("dd"), u64c(1)),
                        ],
                    )),
                );
                f.ret_ok(tup(vec![u64c(2), v("ret"), v("a"), v("sub"), v("mo"), u64c(0)]));
            };
            // `kb[base]` is read only by frames that have children.
            f.if_(and(eq(v("mo"), u64c(1)), eq(v("tag"), u64c(12))), move |f| {
                f.if_(lt(v("j"), u64c(2)), move |f| {
                    child(f, at("kb", add(v("base"), v("j"))), u64c(1), v("acc"));
                });
                f.if_(eq(v("j"), u64c(2)), move |f| {
                    fail_if(
                        f,
                        tget(call("ck_info", vec![at("kb", v("base"))]), 1),
                        TYPE_NOT_ORDERABLE,
                    );
                    child(f, at("kb", v("base")), u64c(2), v("acc"));
                });
                f.expr(tri(call("ck_req", vec![v("ret")])));
                pop(f, v("ret"), v("acc"), u64c(0));
            });
            f.if_(and(eq(v("mo"), u64c(2)), eq(v("tag"), u64c(11))), move |f| {
                f.if_(eq(v("j"), u64c(0)), move |f| child(f, at("kb", v("base")), u64c(2), v("acc")));
                pop(f, call("ck_tand", vec![v("ret"), u64c(TR_NO_ORDER)]), v("acc"), u64c(0));
            });
            f.if_(and(eq(v("mo"), u64c(2)), eq(v("tag"), u64c(12))), move |f| {
                f.if_(eq(v("j"), u64c(0)), move |f| child(f, at("kb", v("base")), u64c(2), v("acc")));
                f.if_(eq(v("j"), u64c(1)), move |f| {
                    f.expr(tri(call("ck_req", vec![v("ret")])));
                    child(f, at("kb", add(v("base"), u64c(1))), u64c(2), v("ret"));
                });
                pop(
                    f,
                    call(
                        "ck_tand",
                        vec![call("ck_tand", vec![v("acc"), v("ret")]), u64c(TR_NO_ORDER)],
                    ),
                    v("acc"),
                    u64c(0),
                );
            });
            // Map-key walks ignore `a`; trait walks combine children.
            f.let_(
                "a",
                select(
                    or(eq(v("j"), u64c(0)), eq(v("mo"), u64c(1))),
                    v("acc"),
                    call("ck_tand", vec![v("acc"), v("ret")]),
                ),
            );
            f.if_(lt(v("j"), v("kn")), member);
            pop(
                f,
                v("a"),
                v("a"),
                select(eq(v("tag"), u64c(10)), v("mo"), u64c(0)),
            );
        },
    );
    m.func(
        "ck_walk",
        &[
            ("inv", inv_ty),
            ("w", m.ty(CK_W)),
            ("ty", t::bytes()),
            ("mode", t::u64()),
            ("d", t::u64()),
        ],
        r(t::tuple(vec![m.ty(CK_W), t::u64()])),
        |f| {
            f.var("visits", field(v("w"), "visits"));
            f.var("mkc", field(v("w"), "mkc"));
            f.var("trc", field(v("w"), "trc"));
            // Frames, stride 8: mode, depth, tag, step, acc, first child,
            // member count, argument count; plus the frame type.
            f.var(
                "fr",
                vnew(
                    t::u64(),
                    vec![v("mode"), v("d"), u64c(0), u64c(0), u64c(0), u64c(0), u64c(0), u64c(0)],
                ),
            );
            f.var("ft", vnew(t::bytes(), vec![v("ty")]));
            f.var("sp", u64c(1));
            // Children (then Named arguments) of live frames.
            f.var("kb", vnew(t::bytes(), vec![]));
            f.var("kp", u64c(0));
            f.var("ret", u64c(0));
            f.loop_(|f| {
                f.let_("top", sub(v("sp"), u64c(1)));
                f.let_("o", mul(v("top"), u64c(8)));
                f.let_("mo", at("fr", v("o")));
                f.let_("dd", at2("fr", v("o"), 1));
                f.let_("i", at2("fr", v("o"), 3));
                // 0 nothing, 1 return, 2 enter a child.
                f.var("act", u64c(0));
                f.var("ct", bytesc(&[]));
                f.var("cm", u64c(0));
                f.if_else(
                    eq(v("i"), u64c(0)),
                    |f| {
                        fail_if(f, gt(v("dd"), u64c(MAX_TYPE_DEPTH)), TYPE_DEPTH_LIMIT);
                        inc(f, "visits");
                        fail_if(f, gt(v("visits"), u64c(MAX_WALK_VISITS)), TYPE_RESOURCE_LIMIT);
                        f.let_(
                            "e",
                            tri(call(
                                "ck_wk_enter",
                                vec![
                                    v("inv"),
                                    v("mkc"),
                                    v("trc"),
                                    at("ft", v("top")),
                                    v("mo"),
                                    v("dd"),
                                ],
                            )),
                        );
                        f.if_else(
                            eq(tget(v("e"), 0), u64c(1)),
                            |f| {
                                f.set("ret", tget(v("e"), 1));
                                f.set("act", u64c(1));
                            },
                            |f| {
                                f.let_("mem", tget(v("e"), 3));
                                upd(f, "fr", add(v("o"), u64c(2)), tget(v("e"), 2));
                                upd(f, "fr", add(v("o"), u64c(3)), u64c(1));
                                upd(f, "fr", add(v("o"), u64c(4)), u64c(TR_ALL));
                                upd(f, "fr", add(v("o"), u64c(5)), v("kp"));
                                upd(f, "fr", add(v("o"), u64c(6)), tget(v("e"), 4));
                                upd(
                                    f,
                                    "fr",
                                    add(v("o"), u64c(7)),
                                    sub(vlen(v("mem")), tget(v("e"), 4)),
                                );
                                f.set("kb", call("ck_putall", vec![v("kb"), v("kp"), v("mem")]));
                                f.set("kp", add(v("kp"), vlen(v("mem"))));
                            },
                        );
                    },
                    |f| {
                        f.let_(
                            "s",
                            tri(call(
                                "ck_wk_next",
                                vec![
                                    v("mo"),
                                    at2("fr", v("o"), 2),
                                    sub(v("i"), u64c(1)),
                                    at2("fr", v("o"), 6),
                                    at2("fr", v("o"), 7),
                                    at2("fr", v("o"), 5),
                                    at2("fr", v("o"), 4),
                                    v("ret"),
                                    v("dd"),
                                    v("kb"),
                                ],
                            )),
                        );
                        f.set("ret", tget(v("s"), 1));
                        upd(f, "fr", add(v("o"), u64c(4)), tget(v("s"), 2));
                        f.let_("cache", tget(v("s"), 5));
                        f.if_(gt(v("cache"), u64c(0)), |f| {
                            f.let_("key", walk_key(v("dd"), at("ft", v("top"))));
                            f.if_else(
                                eq(v("cache"), u64c(1)),
                                |f| f.set("mkc", mins(v("mkc"), v("key"), boolc(true))),
                                |f| f.set("trc", mins(v("trc"), v("key"), v("ret"))),
                            );
                        });
                        f.set("act", tget(v("s"), 0));
                        f.set("ct", tget(v("s"), 3));
                        f.set("cm", tget(v("s"), 4));
                        f.if_(eq(v("act"), u64c(1)), |f| f.set("kp", at2("fr", v("o"), 5)));
                    },
                );
                f.if_(eq(v("act"), u64c(2)), |f| {
                    upd(f, "fr", add(v("o"), u64c(3)), add(v("i"), u64c(1)));
                    f.let_("o2", add(v("o"), u64c(8)));
                    f.set("fr", call("ck_grow", vec![v("fr"), add(v("o2"), u64c(8))]));
                    upd(f, "fr", v("o2"), v("cm"));
                    upd(f, "fr", add(v("o2"), u64c(1)), add(v("dd"), u64c(1)));
                    upd(f, "fr", add(v("o2"), u64c(3)), u64c(0));
                    putb(f, "ft", v("sp"), v("ct"));
                    inc(f, "sp");
                });
                f.if_(eq(v("act"), u64c(1)), |f| {
                    f.set("sp", v("top"));
                    f.if_(eq(v("sp"), u64c(0)), |f| {
                        f.ret_ok(tup(vec![
                            rec(CK_W, vec![v("visits"), v("mkc"), v("trc")]),
                            v("ret"),
                        ]));
                    });
                });
            });
        },
    );
}

/// `check_type`, memoized `check_type`, closed traits, instantiation, and
/// `TypeEnvironment::new`.
#[allow(clippy::too_many_lines)]
fn define_env(m: &mut Module) {
    let inv_ty = m.ty(INV);
    let memo = m.ty(CK_M);
    let fresh_walk = || {
        rec(
            CK_W,
            vec![
                u64c(0),
                mnew(t::bytes(), t::bool(), vec![]),
                mnew(t::bytes(), t::u64(), vec![]),
            ],
        )
    };
    // `TypeEnvironment::check_type(ty, pc)`.
    m.func(
        "ck_check_type",
        &[
            ("inv", inv_ty.clone()),
            ("ar", t::map(t::bytes(), t::u64())),
            ("ty", t::bytes()),
            ("pc", t::u64()),
        ],
        r(t::unit()),
        move |f| {
            f.expr(tri(call("ck_inner", vec![v("ar"), v("ty"), v("pc")])));
            f.expr(tri(call(
                "ck_walk",
                vec![v("inv"), fresh_walk(), v("ty"), u64c(1), u64c(1)],
            )));
            f.ret_ok(unitc());
        },
    );
    // Memoized `check_type`: (memo, code).
    m.func(
        "ck_ctype",
        &[
            ("inv", inv_ty.clone()),
            ("m", memo.clone()),
            ("ty", t::bytes()),
            ("pc", t::u64()),
        ],
        t::tuple(vec![memo.clone(), t::u32()]),
        |f| {
            f.let_("key", concat(vnew(t::bytes(), vec![call("uvarb", vec![v("pc")]), v("ty")])));
            f.match_opt(
                mget_opt(field(v("m"), "ct"), v("key")),
                "c",
                |f| f.ret(tup(vec![v("m"), v("c")])),
                |_| {},
            );
            f.match_res(
                call(
                    "ck_check_type",
                    vec![v("inv"), field(v("m"), "ar"), v("ty"), v("pc")],
                ),
                "_",
                |f| f.ret(call("ck_mput", vec![v("m"), v("key"), u32c(0)])),
                "e",
                |f| f.ret(call("ck_mput", vec![v("m"), v("key"), v("e")])),
            );
        },
    );
    m.func(
        "ck_mput",
        &[("m", memo.clone()), ("key", t::bytes()), ("c", t::u32())],
        t::tuple(vec![memo.clone(), t::u32()]),
        |f| {
            f.ret(tup(vec![
                rec(
                    CK_M,
                    vec![
                        mins(field(v("m"), "ct"), v("key"), v("c")),
                        field(v("m"), "tr"),
                        field(v("m"), "ar"),
                    ],
                ),
                v("c"),
            ]));
        },
    );
    // `TypeEnvironment::traits(ty)`: (memo, code, mask).
    m.func(
        "ck_trait",
        &[("inv", inv_ty.clone()), ("m", memo.clone()), ("ty", t::bytes())],
        t::tuple(vec![memo.clone(), t::u32(), t::u64()]),
        move |f| {
            f.match_opt(
                mget_opt(field(v("m"), "tr"), v("ty")),
                "x",
                |f| f.ret(tup(vec![v("m"), tget(v("x"), 0), tget(v("x"), 1)])),
                |_| {},
            );
            f.let_("ctr", call("ck_ctype", vec![v("inv"), v("m"), v("ty"), u64c(0)]));
            f.let_("m1", tget(v("ctr"), 0));
            f.var("x", tup(vec![tget(v("ctr"), 1), u64c(0)]));
            f.if_(eq(tget(v("ctr"), 1), u32c(0)), move |f| {
                f.match_res(
                    call("ck_walk", vec![v("inv"), fresh_walk(), v("ty"), u64c(2), u64c(1)]),
                    "w",
                    |f| f.set("x", tup(vec![u32c(0), tget(v("w"), 1)])),
                    "e",
                    |f| f.set("x", tup(vec![v("e"), u64c(0)])),
                );
            });
            f.ret(tup(vec![
                rec(
                    CK_M,
                    vec![
                        field(v("m1"), "ct"),
                        mins(field(v("m1"), "tr"), v("ty"), v("x")),
                        field(v("m1"), "ar"),
                    ],
                ),
                tget(v("x"), 0),
                tget(v("x"), 1),
            ]));
        },
    );
    // `instantiate_in_scope(payload, args, pc)`: (memo, code, type).
    m.func(
        "ck_inst",
        &[
            ("inv", inv_ty.clone()),
            ("m", memo.clone()),
            ("payload", t::bytes()),
            ("args", t::vec(t::bytes())),
            ("pc", t::u64()),
        ],
        t::tuple(vec![memo.clone(), t::u32(), t::bytes()]),
        |f| {
            f.if_(gt(vlen(v("args")), u64c(MAX_TYPE_ARGUMENTS)), |f| {
                f.ret(tup(vec![v("m"), code(TYPE_ARGUMENT_LIMIT), bytesc(&[])]));
            });
            f.var("m", v("m"));
            f.for_u64("k", u64c(0), vlen(v("args")), |f| {
                f.let_("x", call("ck_ctype", vec![v("inv"), v("m"), at("args", v("k")), v("pc")]));
                f.set("m", tget(v("x"), 0));
                f.if_(ne(tget(v("x"), 1), u32c(0)), |f| {
                    f.ret(tup(vec![v("m"), tget(v("x"), 1), bytesc(&[])]));
                });
            });
            f.match_res(
                call("ck_subst", vec![v("payload"), v("args"), u64c(1)]),
                "sub",
                |f| {
                    f.let_("x", call("ck_ctype", vec![v("inv"), v("m"), v("sub"), v("pc")]));
                    f.ret(tup(vec![tget(v("x"), 0), tget(v("x"), 1), v("sub")]));
                },
                "e",
                |f| f.ret(tup(vec![v("m"), v("e"), bytesc(&[])])),
            );
        },
    );
    // A fresh memo with the definition arities.
    m.func("ck_memo", &[("inv", inv_ty.clone())], memo.clone(), |f| {
        f.let_("tds", field(v("inv"), "tdefs"));
        f.var("ar", mnew(t::bytes(), t::u64(), vec![]));
        f.for_u64("k", u64c(0), vlen(v("tds")), |f| {
            f.let_("td", at("tds", v("k")));
            f.set("ar", mins(v("ar"), field(v("td"), "id"), vlen(field(v("td"), "tparams"))));
        });
        f.ret(rec(
            CK_M,
            vec![
                mnew(t::bytes(), t::u32(), vec![]),
                mnew(t::bytes(), t::tuple(vec![t::u32(), t::u64()]), vec![]),
                v("ar"),
            ],
        ));
    });
    // Sorted unique Named dependencies of one definition.
    m.func("ck_deps", &[("td", m.ty(TDEF))], t::vec(t::bytes()), |f| {
        f.let_("mt", call("ck_mtypes", vec![v("td")]));
        f.var("all", vnew(t::bytes(), vec![]));
        f.for_u64("k", u64c(0), vlen(v("mt")), |f| {
            f.let_("ids", call("ck_named_ids", vec![at("mt", v("k"))]));
            f.for_u64("q", u64c(0), vlen(v("ids")), |f| {
                f.set("all", push(v("all"), at("ids", v("q"))));
            });
        });
        f.ret(call("ck_sortu", vec![v("all")]));
    });
    // `reject_definition_cycles`: the native depth-first walk.
    m.func("ck_cycles", &[("inv", inv_ty.clone())], r(t::unit()), |f| {
        f.let_("tds", field(v("inv"), "tdefs"));
        // 1 active, 2 complete.
        f.var("visits", mnew(t::bytes(), t::u64(), vec![]));
        // Frames, stride 4: depth, step, first dependency, count; plus id.
        f.var("sfr", vnew(t::u64(), vec![]));
        f.var("sid", vnew(t::bytes(), vec![]));
        f.var("db", vnew(t::bytes(), vec![]));
        f.var("dp", u64c(0));
        f.var("sp", u64c(0));
        let frame = |f: &mut FnB<'_>, id: E, d: E| {
            f.let_("#o", mul(v("sp"), u64c(4)));
            f.set("sfr", call("ck_grow", vec![v("sfr"), add(v("#o"), u64c(4))]));
            upd(f, "sfr", v("#o"), d);
            upd(f, "sfr", add(v("#o"), u64c(1)), u64c(0));
            putb(f, "sid", v("sp"), id);
            inc(f, "sp");
        };
        f.for_u64("r0", u64c(0), vlen(v("tds")), move |f| {
            frame(f, field(at("tds", v("r0")), "id"), u64c(1));
            f.while_(gt(v("sp"), u64c(0)), move |f| {
                f.let_("top", sub(v("sp"), u64c(1)));
                f.let_("o", mul(v("top"), u64c(4)));
                f.let_("id", at("sid", v("top")));
                f.let_("d", at("sfr", v("o")));
                f.let_("i", at2("sfr", v("o"), 1));
                f.if_else(
                    eq(v("i"), u64c(0)),
                    |f| {
                        fail_if(f, gt(v("d"), u64c(MAX_TYPE_DEPTH)), TYPE_DEPTH_LIMIT);
                        f.let_("st", call("ck_mget0", vec![v("visits"), v("id")]));
                        fail_if(f, eq(v("st"), u64c(1)), TYPE_DEFINITION_CYCLE);
                        f.if_else(
                            eq(v("st"), u64c(2)),
                            |f| f.set("sp", v("top")),
                            |f| {
                                f.set("visits", mins(v("visits"), v("id"), u64c(1)));
                                f.let_(
                                    "deps",
                                    call("ck_deps", vec![unwrap(call("inv_tdef", vec![v("inv"), v("id")]))]),
                                );
                                upd(f, "sfr", add(v("o"), u64c(1)), u64c(1));
                                upd(f, "sfr", add(v("o"), u64c(2)), v("dp"));
                                upd(f, "sfr", add(v("o"), u64c(3)), vlen(v("deps")));
                                f.set("db", call("ck_putall", vec![v("db"), v("dp"), v("deps")]));
                                f.set("dp", add(v("dp"), vlen(v("deps"))));
                            },
                        );
                    },
                    move |f| {
                        f.let_("j", sub(v("i"), u64c(1)));
                        f.if_else(
                            lt(v("j"), at2("sfr", v("o"), 3)),
                            move |f| {
                                f.let_("dep", at("db", add(at2("sfr", v("o"), 2), v("j"))));
                                fail_if(
                                    f,
                                    not(is_some(call("inv_tdef", vec![v("inv"), v("dep")]))),
                                    TYPE_DEFINITION_UNKNOWN,
                                );
                                upd(f, "sfr", add(v("o"), u64c(1)), add(v("i"), u64c(1)));
                                frame(f, v("dep"), add(v("d"), u64c(1)));
                            },
                            |f| {
                                f.set("visits", mins(v("visits"), v("id"), u64c(2)));
                                f.set("dp", at2("sfr", v("o"), 2));
                                f.set("sp", v("top"));
                            },
                        );
                    },
                );
            });
        });
        f.ret_ok(unitc());
    });
    // One definition's shape (`check_definition_shape`).
    m.func(
        "ck_shape",
        &[("ar", t::map(t::bytes(), t::u64())), ("td", m.ty(TDEF))],
        r(t::unit()),
        |f| {
            f.let_("tp", field(v("td"), "tparams"));
            fail_if(f, gt(vlen(v("tp")), u64c(MAX_TYPE_ARGUMENTS)), TYPE_ARGUMENT_LIMIT);
            f.for_u64("q", u64c(0), vlen(v("tp")), |f| {
                fail_if(f, ne(at("tp", v("q")), v("q")), TYPE_PARAMETER_OUT_OF_SCOPE);
            });
            f.let_("ms", field(v("td"), "members"));
            fail_if(f, gt(vlen(v("ms")), u64c(MAX_MEMBERS)), TYPE_RESOURCE_LIMIT);
            f.var("seen", mnew(t::bytes(), t::bool(), vec![]));
            f.for_u64("q", u64c(0), vlen(v("ms")), |f| {
                f.let_("mem", at("ms", v("q")));
                fail_if(f, mhas(v("seen"), field(v("mem"), "member")), TYPE_MEMBER_DUPLICATE);
                f.set("seen", mins(v("seen"), field(v("mem"), "member"), boolc(true)));
                f.if_(field(v("mem"), "has_ty"), |f| {
                    f.expr(tri(call(
                        "ck_inner",
                        vec![v("ar"), field(v("mem"), "ty"), vlen(v("tp"))],
                    )));
                });
            });
            f.ret_ok(unitc());
        },
    );
    // `TypeEnvironment::new(inv.tdefs)`.
    m.func("ck_types", &[("inv", inv_ty)], r(t::unit()), move |f| {
        f.let_("tds", field(v("inv"), "tdefs"));
        fail_if(f, gt(vlen(v("tds")), u64c(MAX_DEFINITIONS)), TYPE_RESOURCE_LIMIT);
        f.for_u64("k", u64c(1), vlen(v("tds")), |f| {
            fail_if(
                f,
                eq(
                    field(at("tds", v("k")), "id"),
                    field(at("tds", sub(v("k"), u64c(1))), "id"),
                ),
                TYPE_DEFINITION_DUPLICATE,
            );
        });
        f.let_("ar", field(call("ck_memo", vec![v("inv")]), "ar"));
        f.for_u64("k", u64c(0), vlen(v("tds")), |f| {
            f.expr(tri(call("ck_shape", vec![v("ar"), at("tds", v("k"))])));
        });
        f.expr(tri(call("ck_cycles", vec![v("inv")])));
        // Map keys over one shared walk.
        f.var("w", fresh_walk());
        f.for_u64("k", u64c(0), vlen(v("tds")), |f| {
            f.let_("mt", call("ck_mtypes", vec![at("tds", v("k"))]));
            f.for_u64("q", u64c(0), vlen(v("mt")), |f| {
                f.set(
                    "w",
                    tget(
                        tri(call(
                            "ck_walk",
                            vec![v("inv"), v("w"), at("mt", v("q")), u64c(1), u64c(1)],
                        )),
                        0,
                    ),
                );
            });
        });
        f.ret_ok(unitc());
    });
}

/// Per-function inventory aggregates (one pass over the inventory).
#[allow(clippy::too_many_lines)]
fn define_index(m: &mut Module) {
    let inv_ty = m.ty(INV);
    let agg = || t::map(t::bytes(), t::u64());
    m.func(
        "ck_madd",
        &[("map", agg()), ("key", t::bytes()), ("d", t::u64())],
        agg(),
        |f| {
            f.ret(mins(
                v("map"),
                v("key"),
                add(call("ck_mget0", vec![v("map"), v("key")]), v("d")),
            ));
        },
    );
    m.func("ck_mget0", &[("map", agg()), ("key", t::bytes())], t::u64(), |f| {
        f.match_opt(mget_opt(v("map"), v("key")), "x", |f| f.ret(v("x")), |_| {});
        f.ret(u64c(0));
    });
    // Terminator value uses (`terminator_use_count`).
    m.func("ck_tuses", &[("term", m.ty(TERM))], t::u64(), |f| {
        f.let_("kind", field(v("term"), "kind"));
        f.let_("es", field(v("term"), "edges"));
        f.var("n", select(any_tag("kind", &[1, 3, 4]), u64c(1), u64c(0)));
        f.if_(and(eq(v("kind"), u64c(5)), field(v("term"), "has_val")), |f| {
            f.set("n", u64c(1));
        });
        f.for_u64("k", u64c(0), vlen(v("es")), |f| {
            f.set("n", add(v("n"), vlen(field(at("es", v("k")), "args"))));
        });
        f.let_("cs", field(v("term"), "cases"));
        f.for_u64("k", u64c(0), vlen(v("cs")), |f| {
            f.set("n", add(v("n"), vlen(field(at("cs", v("k")), "args"))));
        });
        f.ret(v("n"));
    });
    m.func("ck_index", &[("inv", inv_ty)], m.ty(CK_IX), |f| {
        f.var("b2f", mnew(t::bytes(), t::bytes(), vec![]));
        for name in ["nb", "np", "no", "nv", "nu", "big"] {
            f.var(name, mnew(t::bytes(), t::u64(), vec![]));
        }
        let madd = |f: &mut FnB<'_>, map: &str, key: E, d: E| {
            f.set(map, call("ck_madd", vec![v(map), key, d]));
        };
        f.let_("bs", field(v("inv"), "blks"));
        f.for_u64("k", u64c(0), vlen(v("bs")), move |f| {
            f.let_("b", at("bs", v("k")));
            f.let_("fn", field(v("b"), "function"));
            f.set("b2f", mins(v("b2f"), field(v("b"), "id"), v("fn")));
            madd(f, "nb", v("fn"), u64c(1));
            madd(f, "nu", v("fn"), call("ck_tuses", vec![field(v("b"), "term")]));
            madd(
                f,
                "big",
                v("fn"),
                select(gt(vlen(field(v("b"), "params")), u64c(MAX_CFG_PARAMETERS)), u64c(1), u64c(0)),
            );
        });
        f.let_("ps", field(v("inv"), "prms"));
        f.for_u64("k", u64c(0), vlen(v("ps")), move |f| {
            f.let_("ow", field(at("ps", v("k")), "owner"));
            f.let_(
                "fn",
                select(mhas(v("b2f"), v("ow")), mget(v("b2f"), v("ow")), v("ow")),
            );
            madd(f, "np", v("fn"), u64c(1));
            madd(f, "nv", v("fn"), u64c(1));
        });
        f.let_("os", field(v("inv"), "ops"));
        f.for_u64("k", u64c(0), vlen(v("os")), move |f| {
            f.let_("o", at("os", v("k")));
            f.if_(mhas(v("b2f"), field(v("o"), "block")), move |f| {
                f.let_("fn", mget(v("b2f"), field(v("o"), "block")));
                f.let_("nr", vlen(field(v("o"), "results")));
                f.let_("nd", vlen(field(v("o"), "operands")));
                madd(f, "no", v("fn"), u64c(1));
                madd(f, "nv", v("fn"), v("nr"));
                madd(f, "nu", v("fn"), v("nd"));
                madd(
                    f,
                    "big",
                    v("fn"),
                    select(
                        or(
                            gt(v("nr"), u64c(MAX_CFG_PARAMETERS)),
                            gt(v("nd"), u64c(MAX_CFG_PARAMETERS)),
                        ),
                        u64c(1),
                        u64c(0),
                    ),
                );
            });
        });
        f.ret(rec(
            CK_IX,
            vec![v("b2f"), v("nb"), v("np"), v("no"), v("nv"), v("nu"), v("big")],
        ));
    });
}

/// Successors, reachability, the dominator budget, and dominators.
#[allow(clippy::too_many_lines)]
fn define_cfg(m: &mut Module) {
    let edges = || t::tuple(vec![u(), u(), u()]);
    // `build_successors`: (first successor, count, successors) per block.
    m.func(
        "ck_succ",
        &[
            ("lb", t::vec(m.ty(BLK))),
            ("bix", t::map(t::bytes(), t::u64())),
        ],
        r(edges()),
        |f| {
            f.var("sb", vnew(t::u64(), vec![]));
            f.var("sn", vnew(t::u64(), vec![]));
            f.var("sc", vnew(t::u64(), vec![]));
            f.for_u64("i", u64c(0), vlen(v("lb")), |f| {
                f.let_("term", field(at("lb", v("i")), "term"));
                f.let_("es", field(v("term"), "edges"));
                f.let_("cs", field(v("term"), "cases"));
                // Return and trap terminators carry no edges.
                f.let_(
                    "tg",
                    select(
                        any_tag_e(field(v("term"), "kind"), &[2, 3]),
                        call("ck_targets", vec![v("es")]),
                        call("ck_ctargets", vec![v("cs")]),
                    ),
                );
                f.set("sb", push(v("sb"), vlen(v("sc"))));
                f.set("sn", push(v("sn"), vlen(v("tg"))));
                fail_if(f, gt(add(vlen(v("sc")), vlen(v("tg"))), u64c(MAX_CFG_EDGES)), CFG_RESOURCE_LIMIT);
                f.for_u64("k", u64c(0), vlen(v("tg")), |f| {
                    f.match_opt(
                        mget_opt(v("bix"), at("tg", v("k"))),
                        "pos",
                        |f| f.set("sc", push(v("sc"), v("pos"))),
                        |f| f.ret_err(code(CFG_TARGET_INVALID)),
                    );
                });
            });
            f.ret_ok(tup(vec![v("sb"), v("sn"), v("sc")]));
        },
    );
    m.func("ck_targets", &[("es", t::vec(m.ty(EDGE)))], t::vec(t::bytes()), |f| {
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("k", u64c(0), vlen(v("es")), |f| {
            f.set("out", push(v("out"), field(at("es", v("k")), "target")));
        });
        f.ret(v("out"));
    });
    m.func("ck_ctargets", &[("cs", t::vec(m.ty(SCASE)))], t::vec(t::bytes()), |f| {
        f.var("out", vnew(t::bytes(), vec![]));
        f.for_u64("k", u64c(0), vlen(v("cs")), |f| {
            f.set("out", push(v("out"), field(at("cs", v("k")), "target")));
        });
        f.ret(v("out"));
    });
    let sargs = || vec![("sb", u()), ("sn", u()), ("sc", u())];
    // Breadth-first reachability: (marks, discovery order).
    m.func(
        "ck_bfs",
        &[sargs(), vec![("ep", t::u64()), ("nfb", t::u64())]].concat(),
        t::tuple(vec![u(), u()]),
        |f| {
            f.var("reach", call("ck_fill", vec![v("nfb"), u64c(0)]));
            upd(f, "reach", v("ep"), u64c(1));
            f.var("q", vnew(t::u64(), vec![v("ep")]));
            f.var("head", u64c(0));
            f.while_(lt(v("head"), vlen(v("q"))), |f| {
                f.let_("x", at("q", v("head")));
                inc(f, "head");
                f.for_u64("k", u64c(0), at("sn", v("x")), |f| {
                    f.let_("s", at("sc", add(at("sb", v("x")), v("k"))));
                    f.if_(eq(at("reach", v("s")), u64c(0)), |f| {
                        upd(f, "reach", v("s"), u64c(1));
                        f.set("q", push(v("q"), v("s")));
                    });
                });
            });
            f.ret(tup(vec![v("reach"), v("q")]));
        },
    );
    // Reachable predecessor counts (with multiplicity).
    m.func(
        "ck_rp",
        &[sargs(), vec![("q", u()), ("nfb", t::u64())]].concat(),
        u(),
        |f| {
            f.var("rp", call("ck_fill", vec![v("nfb"), u64c(0)]));
            f.for_u64("qi", u64c(0), vlen(v("q")), |f| {
                f.let_("x", at("q", v("qi")));
                f.for_u64("k", u64c(0), at("sn", v("x")), |f| {
                    f.let_("s", at("sc", add(at("sb", v("x")), v("k"))));
                    upd(f, "rp", v("s"), add(at("rp", v("s")), u64c(1)));
                });
            });
            f.ret(v("rp"));
        },
    );
    // Postorder numbers and the postorder list of the reachable graph.
    m.func(
        "ck_post",
        &[sargs(), vec![("ep", t::u64()), ("nfb", t::u64())]].concat(),
        t::tuple(vec![u(), u()]),
        |f| {
            f.var("po", call("ck_fill", vec![v("nfb"), u64c(INF)]));
            f.var("vis", call("ck_fill", vec![v("nfb"), u64c(0)]));
            upd(f, "vis", v("ep"), u64c(1));
            f.var("order", vnew(t::u64(), vec![]));
            // Frames, stride 2: block, next successor.
            f.var("st", vnew(t::u64(), vec![v("ep"), u64c(0)]));
            f.var("sp", u64c(1));
            f.while_(gt(v("sp"), u64c(0)), |f| {
                f.let_("o", mul(sub(v("sp"), u64c(1)), u64c(2)));
                f.let_("x", at("st", v("o")));
                f.let_("k", at2("st", v("o"), 1));
                f.if_else(
                    lt(v("k"), at("sn", v("x"))),
                    |f| {
                        upd(f, "st", add(v("o"), u64c(1)), add(v("k"), u64c(1)));
                        f.let_("s", at("sc", add(at("sb", v("x")), v("k"))));
                        f.if_(eq(at("vis", v("s")), u64c(0)), |f| {
                            upd(f, "vis", v("s"), u64c(1));
                            putu(f, "st", add(v("o"), u64c(2)), v("s"));
                            putu(f, "st", add(v("o"), u64c(3)), u64c(0));
                            inc(f, "sp");
                        });
                    },
                    |f| {
                        upd(f, "po", v("x"), vlen(v("order")));
                        f.set("order", push(v("order"), v("x")));
                        f.set("sp", sub(v("sp"), u64c(1)));
                    },
                );
            });
            f.ret(tup(vec![v("po"), v("order")]));
        },
    );
    // Flattened reachable predecessor lists: (first predecessor, list).
    m.func(
        "ck_preds",
        &[sargs(), vec![("rp", u()), ("q", u()), ("nfb", t::u64())]].concat(),
        t::tuple(vec![u(), u()]),
        |f| {
            f.var("pb", vnew(t::u64(), vec![]));
            f.var("tot", u64c(0));
            f.for_u64("i", u64c(0), v("nfb"), |f| {
                f.set("pb", push(v("pb"), v("tot")));
                f.set("tot", add(v("tot"), at("rp", v("i"))));
            });
            f.var("pcur", v("pb"));
            f.var("pf", call("ck_fill", vec![v("tot"), u64c(0)]));
            f.for_u64("qi", u64c(0), vlen(v("q")), |f| {
                f.let_("x", at("q", v("qi")));
                f.for_u64("k", u64c(0), at("sn", v("x")), |f| {
                    f.let_("s", at("sc", add(at("sb", v("x")), v("k"))));
                    upd(f, "pf", at("pcur", v("s")), v("x"));
                    upd(f, "pcur", v("s"), add(at("pcur", v("s")), u64c(1)));
                });
            });
            f.ret(tup(vec![v("pb"), v("pf")]));
        },
    );
    // Cooper–Harvey–Kennedy: iterate to the fixed point in reverse
    // postorder (`nfb` marks an undefined immediate dominator).
    m.func(
        "ck_idom",
        &[
            ("rp", u()),
            ("pb", u()),
            ("pf", u()),
            ("po", u()),
            ("order", u()),
            ("ep", t::u64()),
            ("nfb", t::u64()),
        ],
        u(),
        |f| {
            f.var("idom", call("ck_fill", vec![v("nfb"), v("nfb")]));
            upd(f, "idom", v("ep"), v("ep"));
            f.var("changed", boolc(true));
            f.while_(v("changed"), |f| {
                f.set("changed", boolc(false));
                f.for_u64("ri", u64c(0), vlen(v("order")), |f| {
                    f.let_("b", at("order", sub(sub(vlen(v("order")), v("ri")), u64c(1))));
                    f.var("nw", v("nfb"));
                    f.for_u64("k", u64c(0), select(eq(v("b"), v("ep")), u64c(0), at("rp", v("b"))), |f| {
                        f.let_("p", at("pf", add(at("pb", v("b")), v("k"))));
                        f.if_(ne(at("idom", v("p")), v("nfb")), |f| {
                            f.var("a", v("p"));
                            f.var("c", select(eq(v("nw"), v("nfb")), v("p"), v("nw")));
                            f.while_(ne(v("a"), v("c")), |f| {
                                f.while_(lt(at("po", v("a")), at("po", v("c"))), |f| {
                                    f.set("a", at("idom", v("a")));
                                });
                                f.while_(lt(at("po", v("c")), at("po", v("a"))), |f| {
                                    f.set("c", at("idom", v("c")));
                                });
                            });
                            f.set("nw", v("a"));
                        });
                    });
                    f.if_(and(ne(v("b"), v("ep")), ne(at("idom", v("b")), v("nw"))), |f| {
                        upd(f, "idom", v("b"), v("nw"));
                        f.set("changed", boolc(true));
                    });
                });
            });
            f.ret(v("idom"));
        },
    );
    // Dominator-tree (pre, post) interval numbers.
    m.func(
        "ck_dtree",
        &[("idom", u()), ("q", u()), ("ep", t::u64()), ("nfb", t::u64())],
        t::tuple(vec![u(), u()]),
        |f| {
            f.var("cc", call("ck_fill", vec![v("nfb"), u64c(0)]));
            f.for_u64("qi", u64c(1), vlen(v("q")), |f| {
                f.let_("pa", at("idom", at("q", v("qi"))));
                upd(f, "cc", v("pa"), add(at("cc", v("pa")), u64c(1)));
            });
            f.var("cb", vnew(t::u64(), vec![]));
            f.var("ct", u64c(0));
            f.for_u64("i", u64c(0), v("nfb"), |f| {
                f.set("cb", push(v("cb"), v("ct")));
                f.set("ct", add(v("ct"), at("cc", v("i"))));
            });
            f.var("ccur", v("cb"));
            f.var("ch", call("ck_fill", vec![v("ct"), u64c(0)]));
            f.for_u64("qi", u64c(1), vlen(v("q")), |f| {
                f.let_("x", at("q", v("qi")));
                f.let_("pa", at("idom", v("x")));
                upd(f, "ch", at("ccur", v("pa")), v("x"));
                upd(f, "ccur", v("pa"), add(at("ccur", v("pa")), u64c(1)));
            });
            f.var("dpre", call("ck_fill", vec![v("nfb"), u64c(INF)]));
            f.var("dpost", call("ck_fill", vec![v("nfb"), u64c(INF)]));
            f.var("pre", u64c(1));
            f.var("post", u64c(0));
            upd(f, "dpre", v("ep"), u64c(0));
            // Frames, stride 2: block, next child.
            f.var("st", vnew(t::u64(), vec![v("ep"), u64c(0)]));
            f.var("sp", u64c(1));
            f.while_(gt(v("sp"), u64c(0)), |f| {
                f.let_("o", mul(sub(v("sp"), u64c(1)), u64c(2)));
                f.let_("x", at("st", v("o")));
                f.let_("k", at2("st", v("o"), 1));
                f.if_else(
                    lt(v("k"), at("cc", v("x"))),
                    |f| {
                        upd(f, "st", add(v("o"), u64c(1)), add(v("k"), u64c(1)));
                        f.let_("c", at("ch", add(at("cb", v("x")), v("k"))));
                        upd(f, "dpre", v("c"), v("pre"));
                        inc(f, "pre");
                        putu(f, "st", add(v("o"), u64c(2)), v("c"));
                        putu(f, "st", add(v("o"), u64c(3)), u64c(0));
                        inc(f, "sp");
                    },
                    |f| {
                        upd(f, "dpost", v("x"), v("post"));
                        inc(f, "post");
                        f.set("sp", sub(v("sp"), u64c(1)));
                    },
                );
            });
            f.ret(tup(vec![v("dpre"), v("dpost")]));
        },
    );
    // The native round count minus one (`L`), stopping early once
    // `L + 1 > maxr`.
    m.func(
        "ck_dom_l",
        &[
            sargs(),
            vec![
                ("q", u()),
                ("ep", t::u64()),
                ("nfb", t::u64()),
                ("maxr", t::u64()),
            ],
        ]
        .concat(),
        t::u64(),
        |f| {
            f.var("big_l", u64c(0));
            f.var("mi", vlen(v("q")));
            f.while_(gt(v("mi"), u64c(1)), |f| {
                f.set("mi", sub(v("mi"), u64c(1)));
                f.let_(
                    "dist",
                    call(
                        "ck_dist",
                        vec![v("sb"), v("sn"), v("sc"), at("q", v("mi")), v("ep"), v("nfb")],
                    ),
                );
                f.for_u64("qi", u64c(1), vlen(v("q")), |f| {
                    f.let_("dn", add(at("dist", at("q", v("qi"))), u64c(1)));
                    f.set(
                        "big_l",
                        select(
                            and(lt(v("dn"), u64c(INF)), gt(v("dn"), v("big_l"))),
                            v("dn"),
                            v("big_l"),
                        ),
                    );
                });
                f.if_(gt(add(v("big_l"), u64c(1)), v("maxr")), |f| f.ret(v("big_l")));
            });
            f.ret(v("big_l"));
        },
    );
    // 0-1 BFS distances from the entry avoiding `mm` (retreating edges
    // cost 1; edges from the entry and forward edges cost 0).
    m.func(
        "ck_dist",
        &[sargs(), vec![("mm", t::u64()), ("ep", t::u64()), ("nfb", t::u64())]].concat(),
        u(),
        |f| {
            f.var("dist", call("ck_fill", vec![v("nfb"), u64c(INF)]));
            upd(f, "dist", v("ep"), u64c(0));
            // Distance 0 blocks every relaxation into the removed block.
            upd(f, "dist", v("mm"), u64c(0));
            f.var("cur", vnew(t::u64(), vec![v("ep")]));
            f.var("level", u64c(0));
            f.while_(gt(vlen(v("cur")), u64c(0)), |f| {
                f.var("nxt", vnew(t::u64(), vec![]));
                f.var("ssp", vlen(v("cur")));
                f.while_(gt(v("ssp"), u64c(0)), |f| {
                    f.set("ssp", sub(v("ssp"), u64c(1)));
                    f.let_("x", at("cur", v("ssp")));
                    f.if_(eq(at("dist", v("x")), v("level")), |f| {
                        f.let_("b0", at("sb", v("x")));
                        f.for_u64("k", u64c(0), at("sn", v("x")), |f| {
                            f.let_("s", at("sc", add(v("b0"), v("k"))));
                            f.let_("zero", or(eq(v("x"), v("ep")), lt(v("x"), v("s"))));
                            f.let_("nd", select(v("zero"), v("level"), add(v("level"), u64c(1))));
                            f.if_(lt(v("nd"), at("dist", v("s"))), |f| {
                                upd(f, "dist", v("s"), v("nd"));
                                f.if_else(
                                    v("zero"),
                                    |f| {
                                        putu(f, "cur", v("ssp"), v("s"));
                                        inc(f, "ssp");
                                    },
                                    |f| f.set("nxt", push(v("nxt"), v("s"))),
                                );
                            });
                        });
                    });
                });
                inc(f, "level");
                f.set("cur", v("nxt"));
            });
            // The removed block and the entry never count.
            upd(f, "dist", v("mm"), u64c(INF));
            upd(f, "dist", v("ep"), u64c(INF));
            f.ret(v("dist"));
        },
    );
    // The native dominator budget (`compute_dominators` work and rounds).
    m.func(
        "ck_budget",
        &[
            sargs(),
            vec![
                ("rp", u()),
                ("q", u()),
                ("dpre", u()),
                ("dpost", u()),
                ("ep", t::u64()),
                ("nfb", t::u64()),
            ],
        ]
        .concat(),
        r(t::unit()),
        |f| {
            // Native rounds are at most 2 plus the number of distinct
            // targets of retreating (declared-order) edges that can lie on an
            // acyclic entry path: not from the entry and not back edges (a
            // back edge's target dominates its source).
            f.var("rtf", call("ck_fill", vec![v("nfb"), u64c(0)]));
            f.var("nt", u64c(0));
            f.var("wsum", u64c(0));
            f.for_u64("qi", u64c(1), vlen(v("q")), |f| {
                f.let_("x", at("q", v("qi")));
                f.set("wsum", add(v("wsum"), add(at("rp", v("x")), u64c(1))));
                f.for_u64("k", u64c(0), at("sn", v("x")), |f| {
                    f.let_("s", at("sc", add(at("sb", v("x")), v("k"))));
                    f.let_(
                        "back",
                        and(
                            le(at("dpre", v("s")), at("dpre", v("x"))),
                            le(at("dpost", v("x")), at("dpost", v("s"))),
                        ),
                    );
                    f.if_(
                        and(
                            le(v("s"), v("x")),
                            and(eq(at("rtf", v("s")), u64c(0)), not(v("back"))),
                        ),
                        |f| {
                            upd(f, "rtf", v("s"), u64c(1));
                            inc(f, "nt");
                        },
                    );
                });
            });
            f.let_("work", mul(div(add(v("nfb"), u64c(63)), u64c(64)), v("wsum")));
            f.if_(eq(v("work"), u64c(0)), |f| f.ret_ok(unitc()));
            f.let_("byw", div(u64c(MAX_DOM_WORK), v("work")));
            f.let_(
                "maxr",
                select(lt(v("byw"), u64c(MAX_CFG_BLOCKS)), v("byw"), u64c(MAX_CFG_BLOCKS)),
            );
            f.let_("rr", add(vlen(v("q")), u64c(1)));
            f.let_("tb", add(v("nt"), u64c(2)));
            f.if_(le(select(lt(v("tb"), v("rr")), v("tb"), v("rr")), v("maxr")), |f| {
                f.ret_ok(unitc());
            });
            f.let_(
                "big_l",
                call(
                    "ck_dom_l",
                    vec![v("sb"), v("sn"), v("sc"), v("q"), v("ep"), v("nfb"), v("maxr")],
                ),
            );
            fail_if(f, gt(add(v("big_l"), u64c(1)), v("maxr")), CFG_RESOURCE_LIMIT);
            f.ret_ok(unitc());
        },
    );
    // `build_successors`, reachability, `validate_reachability`, the
    // dominator budget, and the dominator tree.
    m.func(
        "ck_fn_cfg",
        &[
            ("lb", t::vec(m.ty(BLK))),
            ("bix", t::map(t::bytes(), t::u64())),
            ("ep", t::u64()),
        ],
        r(m.ty(CK_C)),
        |f| {
            fail_if(f, ne(field(at("lb", v("ep")), "reach"), u64c(1)), CFG_ENTRY_INVALID);
            f.let_("nfb", vlen(v("lb")));
            f.let_("e", tri(call("ck_succ", vec![v("lb"), v("bix")])));
            f.let_("sb", tget(v("e"), 0));
            f.let_("sn", tget(v("e"), 1));
            f.let_("sc", tget(v("e"), 2));
            let s3 = || vec![v("sb"), v("sn"), v("sc")];
            f.let_("rq", call("ck_bfs", [s3(), vec![v("ep"), v("nfb")]].concat()));
            f.let_("reach", tget(v("rq"), 0));
            f.let_("q", tget(v("rq"), 1));
            f.for_u64("i", u64c(0), v("nfb"), |f| {
                fail_if(
                    f,
                    ne(
                        field(at("lb", v("i")), "reach"),
                        select(eq(at("reach", v("i")), u64c(1)), u64c(1), u64c(2)),
                    ),
                    CFG_REACHABILITY,
                );
            });
            f.let_("rp", call("ck_rp", [s3(), vec![v("q"), v("nfb")]].concat()));
            f.let_("po", call("ck_post", [s3(), vec![v("ep"), v("nfb")]].concat()));
            f.let_("pp", call("ck_preds", [s3(), vec![v("rp"), v("q"), v("nfb")]].concat()));
            f.let_(
                "idom",
                call(
                    "ck_idom",
                    vec![
                        v("rp"),
                        tget(v("pp"), 0),
                        tget(v("pp"), 1),
                        tget(v("po"), 0),
                        tget(v("po"), 1),
                        v("ep"),
                        v("nfb"),
                    ],
                ),
            );
            f.let_("dt", call("ck_dtree", vec![v("idom"), v("q"), v("ep"), v("nfb")]));
            f.expr(tri(call(
                "ck_budget",
                [
                    s3(),
                    vec![
                        v("rp"),
                        v("q"),
                        tget(v("dt"), 0),
                        tget(v("dt"), 1),
                        v("ep"),
                        v("nfb"),
                    ],
                ]
                .concat(),
            )));
            f.ret_ok(rec(CK_C, vec![v("reach"), tget(v("dt"), 0), tget(v("dt"), 1)]));
        },
    );
}

fn any_tag_e(x: E, tags: &[u64]) -> E {
    let mut out = eq(x.clone(), u64c(tags[0]));
    for tag in &tags[1..] {
        out = or(out, eq(x.clone(), u64c(*tag)));
    }
    out
}

/// Inventory, resolution, terminators, and the entry points.
#[allow(clippy::too_many_lines)]
fn define_function(m: &mut Module) {
    let inv_ty = m.ty(INV);
    let memo = m.ty(CK_M);
    let bix_ty = || t::map(t::bytes(), t::u64());
    let checked = || t::tuple(vec![named(CK_M), t::u32(), t::u64()]);
    // One declared parameter: (memo, code, inventory position).
    m.func(
        "ck_prm1",
        &[
            ("inv", inv_ty.clone()),
            ("m", memo.clone()),
            ("id", t::bytes()),
            ("fid", t::bytes()),
            ("bix", bix_ty()),
            ("owner", t::bytes()),
            ("role", t::u64()),
            ("ord", t::u64()),
            ("pc", t::u64()),
        ],
        checked(),
        |f| {
            let bad = |f: &mut FnB<'_>, c: u32| f.ret(tup(vec![v("m"), code(c), u64c(0)]));
            f.match_opt(
                call(
                    "inv_find",
                    vec![field(v("inv"), "index"), v("id"), u64c(KIND_PARAMETER)],
                ),
                "pos",
                move |f| {
                    f.let_("p", vget(field(v("inv"), "prms"), v("pos")));
                    f.let_("ow", field(v("p"), "owner"));
                    f.if_(not(or(eq(v("ow"), v("fid")), mhas(v("bix"), v("ow")))), move |f| {
                        bad(f, GRAPH_UNRESOLVED_REFERENCE);
                    });
                    f.if_(
                        or(ne(v("ow"), v("owner")), ne(field(v("p"), "role"), v("role"))),
                        move |f| bad(f, GRAPH_OWNER_MISMATCH),
                    );
                    f.if_(ne(field(v("p"), "ordinal"), v("ord")), move |f| {
                        bad(f, GRAPH_ORDINAL_MISMATCH);
                    });
                    f.let_(
                        "x",
                        call("ck_ctype", vec![v("inv"), v("m"), field(v("p"), "ty"), v("pc")]),
                    );
                    f.ret(tup(vec![tget(v("x"), 0), tget(v("x"), 1), v("pos")]));
                },
                move |f| bad(f, GRAPH_UNRESOLVED_REFERENCE),
            );
        },
    );
    // One declared operation: (memo, code, inventory position).
    m.func(
        "ck_op1",
        &[
            ("inv", inv_ty.clone()),
            ("m", memo.clone()),
            ("id", t::bytes()),
            ("bid", t::bytes()),
            ("bix", bix_ty()),
            ("ord", t::u64()),
            ("pc", t::u64()),
        ],
        checked(),
        |f| {
            let bad = |f: &mut FnB<'_>, c: u32| f.ret(tup(vec![v("m"), code(c), u64c(0)]));
            f.match_opt(
                call(
                    "inv_find",
                    vec![field(v("inv"), "index"), v("id"), u64c(KIND_OPERATION)],
                ),
                "pos",
                move |f| {
                    f.let_("o", vget(field(v("inv"), "ops"), v("pos")));
                    f.if_(not(mhas(v("bix"), field(v("o"), "block"))), move |f| {
                        bad(f, GRAPH_UNRESOLVED_REFERENCE);
                    });
                    f.if_(ne(field(v("o"), "block"), v("bid")), move |f| {
                        bad(f, GRAPH_OWNER_MISMATCH);
                    });
                    f.if_(ne(field(v("o"), "ordinal"), v("ord")), move |f| {
                        bad(f, GRAPH_ORDINAL_MISMATCH);
                    });
                    f.var("m", v("m"));
                    f.let_("rs", field(v("o"), "results"));
                    f.for_u64("k", u64c(0), vlen(v("rs")), |f| {
                        f.let_("x", call("ck_ctype", vec![v("inv"), v("m"), at("rs", v("k")), v("pc")]));
                        f.set("m", tget(v("x"), 0));
                        f.if_(ne(tget(v("x"), 1), u32c(0)), |f| {
                            f.ret(tup(vec![v("m"), tget(v("x"), 1), v("pos")]));
                        });
                    });
                    f.ret(tup(vec![v("m"), u32c(0), v("pos")]));
                },
                move |f| bad(f, GRAPH_UNRESOLVED_REFERENCE),
            );
        },
    );
    // Duplicate check over a declared id list (`index_declared_ids`).
    m.func("ck_dups", &[("ids", t::vec(t::bytes()))], r(t::unit()), |f| {
        f.var("seen", mnew(t::bytes(), t::bool(), vec![]));
        f.for_u64("k", u64c(0), vlen(v("ids")), |f| {
            f.let_("id", at("ids", v("k")));
            fail_if(f, mhas(v("seen"), v("id")), GRAPH_DUPLICATE_ENTITY);
            f.set("seen", mins(v("seen"), v("id"), boolc(true)));
        });
        f.ret_ok(unitc());
    });
    // The failing element with the smallest identity (native sorted
    // iteration): (best code, best id) after one more element.
    m.func(
        "ck_best",
        &[
            ("bc", t::u32()),
            ("bid", t::bytes()),
            ("c", t::u32()),
            ("id", t::bytes()),
        ],
        t::tuple(vec![t::u32(), t::bytes()]),
        |f| {
            f.if_(
                and(ne(v("c"), u32c(0)), or(eq(v("bc"), u32c(0)), lt(v("id"), v("bid")))),
                |f| f.ret(tup(vec![v("c"), v("id")])),
            );
            f.ret(tup(vec![v("bc"), v("bid")]));
        },
    );
    // `validate_parameter_inventory`.
    m.func(
        "ck_fn_prm",
        &[
            ("inv", inv_ty.clone()),
            ("m", memo.clone()),
            ("fid", t::bytes()),
            ("fps", t::vec(t::bytes())),
            ("bix", bix_ty()),
            ("lb", t::vec(m.ty(BLK))),
            ("pc", t::u64()),
            ("np", t::u64()),
        ],
        r(t::tuple(vec![memo.clone(), m.ty(CK_P)])),
        |f| {
            f.var("m", v("m"));
            f.var("pm", mnew(t::bytes(), t::u64(), vec![]));
            f.var("pty", vnew(t::bytes(), vec![]));
            f.var("pi", vnew(t::u64(), vec![]));
            f.var("bp", vnew(t::u64(), vec![]));
            f.let_("nfb", vlen(v("lb")));
            // Owners: position nfb is the function itself.
            f.for_u64("i", u64c(0), add(v("nfb"), u64c(1)), |f| {
                f.let_("isf", eq(v("i"), u64c(0)));
                f.let_("bi", select(v("isf"), u64c(0), sub(v("i"), u64c(1))));
                f.let_(
                    "ids",
                    select(v("isf"), v("fps"), field(at("lb", v("bi")), "params")),
                );
                f.let_(
                    "owner",
                    select(v("isf"), v("fid"), field(at("lb", v("bi")), "id")),
                );
                f.let_("role", select(v("isf"), u64c(1), u64c(2)));
                f.let_("own", select(v("isf"), v("nfb"), v("bi")));
                f.if_(not(v("isf")), |f| {
                    f.expr(tri(call("ck_dups", vec![v("ids")])));
                    f.set("bp", push(push(v("bp"), vlen(v("pty"))), vlen(v("ids"))));
                });
                f.var("best", tup(vec![u32c(0), bytesc(&[])]));
                f.for_u64("j", u64c(0), vlen(v("ids")), |f| {
                    f.let_("id", at("ids", v("j")));
                    f.let_(
                        "x",
                        call(
                            "ck_prm1",
                            vec![
                                v("inv"),
                                v("m"),
                                v("id"),
                                v("fid"),
                                v("bix"),
                                v("owner"),
                                v("role"),
                                v("j"),
                                v("pc"),
                            ],
                        ),
                    );
                    f.set("m", tget(v("x"), 0));
                    f.set(
                        "best",
                        call(
                            "ck_best",
                            vec![tget(v("best"), 0), tget(v("best"), 1), tget(v("x"), 1), v("id")],
                        ),
                    );
                    f.if_(eq(tget(v("x"), 1), u32c(0)), |f| {
                        f.set("pm", mins(v("pm"), v("id"), vlen(v("pty"))));
                        f.set(
                            "pty",
                            push(
                                v("pty"),
                                field(vget(field(v("inv"), "prms"), tget(v("x"), 2)), "ty"),
                            ),
                        );
                        f.set("pi", push(push(v("pi"), v("role")), v("own")));
                    });
                });
                f.if_(ne(tget(v("best"), 0), u32c(0)), |f| f.ret_err(tget(v("best"), 0)));
            });
            fail_if(f, ne(vlen(v("pty")), v("np")), GRAPH_INVENTORY_MISMATCH);
            f.ret_ok(tup(vec![v("m"), rec(CK_P, vec![v("pm"), v("pty"), v("pi"), v("bp")])]));
        },
    );
    // `validate_operation_inventory`.
    m.func(
        "ck_fn_ops",
        &[
            ("inv", inv_ty.clone()),
            ("m", memo.clone()),
            ("bix", bix_ty()),
            ("lb", t::vec(m.ty(BLK))),
            ("pc", t::u64()),
            ("no", t::u64()),
        ],
        r(t::tuple(vec![memo.clone(), m.ty(CK_O)])),
        |f| {
            f.var("m", v("m"));
            f.var("om", mnew(t::bytes(), t::u64(), vec![]));
            f.var("oi", vnew(t::u64(), vec![]));
            f.var("ob", vnew(t::u64(), vec![]));
            f.var("rt", vnew(t::bytes(), vec![]));
            f.var("od", vnew(named(VREF), vec![]));
            f.for_u64("i", u64c(0), vlen(v("lb")), |f| {
                f.let_("blk", at("lb", v("i")));
                f.let_("ids", field(v("blk"), "ops"));
                f.expr(tri(call("ck_dups", vec![v("ids")])));
                f.set("ob", push(push(v("ob"), div(vlen(v("oi")), u64c(6))), vlen(v("ids"))));
                f.var("best", tup(vec![u32c(0), bytesc(&[])]));
                f.for_u64("j", u64c(0), vlen(v("ids")), |f| {
                    f.let_("id", at("ids", v("j")));
                    f.let_(
                        "x",
                        call(
                            "ck_op1",
                            vec![
                                v("inv"),
                                v("m"),
                                v("id"),
                                field(v("blk"), "id"),
                                v("bix"),
                                v("j"),
                                v("pc"),
                            ],
                        ),
                    );
                    f.set("m", tget(v("x"), 0));
                    f.set(
                        "best",
                        call(
                            "ck_best",
                            vec![tget(v("best"), 0), tget(v("best"), 1), tget(v("x"), 1), v("id")],
                        ),
                    );
                    f.if_(eq(tget(v("x"), 1), u32c(0)), |f| {
                        f.let_("o", vget(field(v("inv"), "ops"), tget(v("x"), 2)));
                        f.let_("rs", field(v("o"), "results"));
                        f.let_("ds", field(v("o"), "operands"));
                        f.set("om", mins(v("om"), v("id"), div(vlen(v("oi")), u64c(6))));
                        for val in [
                            v("i"),
                            v("j"),
                            vlen(v("rt")),
                            vlen(v("rs")),
                            vlen(v("od")),
                            vlen(v("ds")),
                        ] {
                            f.set("oi", push(v("oi"), val));
                        }
                        f.for_u64("k", u64c(0), vlen(v("rs")), |f| {
                            f.set("rt", push(v("rt"), at("rs", v("k"))));
                        });
                        f.for_u64("k", u64c(0), vlen(v("ds")), |f| {
                            f.set("od", push(v("od"), at("ds", v("k"))));
                        });
                    });
                });
                f.if_(ne(tget(v("best"), 0), u32c(0)), |f| f.ret_err(tget(v("best"), 0)));
            });
            fail_if(f, ne(div(vlen(v("oi")), u64c(6)), v("no")), GRAPH_INVENTORY_MISMATCH);
            f.ret_ok(tup(vec![
                v("m"),
                rec(CK_O, vec![v("om"), v("oi"), v("ob"), v("rt"), v("od")]),
            ]));
        },
    );
    // `resolve_value`: (code, type).
    m.func(
        "ck_res",
        &[
            ("g", m.ty(CK_G)),
            ("vr", m.ty(VREF)),
            ("use", t::u64()),
            ("ord", t::u64()),
            ("has_ord", t::bool()),
        ],
        t::tuple(vec![t::u32(), t::bytes()]),
        |f| {
            let bad = |f: &mut FnB<'_>, c: u32| f.ret(tup(vec![code(c), bytesc(&[])]));
            f.let_("reach", field(field(v("g"), "c"), "reach"));
            f.let_("id", field(v("vr"), "id"));
            f.let_("unreach", eq(at("reach", v("use")), u64c(0)));
            f.if_(eq(field(v("vr"), "kind"), u64c(1)), move |f| {
                f.let_("p", field(v("g"), "p"));
                f.match_opt(
                    mget_opt(field(v("p"), "pm"), v("id")),
                    "k",
                    move |f| {
                        f.let_("pi", field(v("p"), "pi"));
                        f.if_(
                            or(
                                eq(at("pi", mul(v("k"), u64c(2))), u64c(1)),
                                eq(at2("pi", mul(v("k"), u64c(2)), 1), v("use")),
                            ),
                            |f| f.ret(tup(vec![u32c(0), vget(field(v("p"), "pty"), v("k"))])),
                        );
                        f.if_(v("unreach"), move |f| bad(f, CFG_UNREACHABLE_VALUE));
                        if crate::seed::fault("check-dominance") {
                            f.ret(tup(vec![u32c(0), vget(field(v("p"), "pty"), v("k"))]));
                        } else {
                            bad(f, CFG_DOMINANCE);
                        }
                    },
                    move |f| bad(f, CFG_VALUE_UNRESOLVED),
                );
            });
            f.let_("o", field(v("g"), "o"));
            f.match_opt(
                mget_opt(field(v("o"), "om"), v("id")),
                "q",
                move |f| {
                    f.let_("oi", field(v("o"), "oi"));
                    f.let_("q6", mul(v("q"), u64c(6)));
                    f.let_("idx", field(v("vr"), "idx"));
                    f.if_(ge(v("idx"), at2("oi", v("q6"), 3)), move |f| bad(f, CFG_RESULT_INDEX));
                    f.let_("ty", vget(field(v("o"), "rt"), add(at2("oi", v("q6"), 2), v("idx"))));
                    f.let_("db", at("oi", v("q6")));
                    f.if_(eq(v("db"), v("use")), move |f| {
                        f.if_(
                            and(v("has_ord"), ge(at2("oi", v("q6"), 1), v("ord"))),
                            move |f| bad(f, CFG_USE_BEFORE_DEFINITION),
                        );
                        f.ret(tup(vec![u32c(0), v("ty")]));
                    });
                    f.if_(v("unreach"), move |f| bad(f, CFG_UNREACHABLE_VALUE));
                    f.let_("dpre", field(field(v("g"), "c"), "dpre"));
                    f.let_("dpost", field(field(v("g"), "c"), "dpost"));
                    f.if_(
                        or(
                            eq(at("reach", v("db")), u64c(0)),
                            or(
                                gt(at("dpre", v("db")), at("dpre", v("use"))),
                                gt(at("dpost", v("use")), at("dpost", v("db"))),
                            ),
                        ),
                        move |f| {
                            if !crate::seed::fault("check-dominance") {
                                bad(f, CFG_DOMINANCE);
                            }
                        },
                    );
                    f.ret(tup(vec![u32c(0), v("ty")]));
                },
                move |f| bad(f, CFG_VALUE_UNRESOLVED),
            );
        },
    );
    // Edge arguments against a target's parameters (`validate_target_edge`
    // and the switch-case loop). Payload arguments use `pay`.
    m.func(
        "ck_args",
        &[
            ("g", m.ty(CK_G)),
            ("src", t::u64()),
            ("target", t::bytes()),
            ("args", t::vec(m.ty(SARG))),
            ("has", t::bool()),
            ("pay", t::bytes()),
        ],
        r(t::unit()),
        |f| {
            f.let_("tp", mget(field(v("g"), "bix"), v("target")));
            f.let_("p", field(v("g"), "p"));
            f.let_("bp", field(v("p"), "bp"));
            fail_if(
                f,
                ne(vlen(v("args")), at2("bp", mul(v("tp"), u64c(2)), 1)),
                CFG_TARGET_ARGUMENTS,
            );
            f.let_("pb", at("bp", mul(v("tp"), u64c(2))));
            f.for_u64("j", u64c(0), vlen(v("args")), |f| {
                f.let_("a", at("args", v("j")));
                f.var("ty", v("pay"));
                f.if_else(
                    field(v("a"), "pay"),
                    |f| fail_if(f, not(v("has")), CFG_SWITCH_PAYLOAD),
                    |f| {
                        f.let_(
                            "x",
                            call(
                                "ck_res",
                                vec![v("g"), field(v("a"), "val"), v("src"), u64c(0), boolc(false)],
                            ),
                        );
                        f.if_(ne(tget(v("x"), 0), u32c(0)), |f| f.ret_err(tget(v("x"), 0)));
                        f.set("ty", tget(v("x"), 1));
                    },
                );
                fail_if(
                    f,
                    ne(v("ty"), vget(field(v("p"), "pty"), add(v("pb"), v("j")))),
                    CFG_TARGET_ARGUMENTS,
                );
            });
            f.ret_ok(unitc());
        },
    );
    // A target edge's arguments as switch arguments.
    m.func("ck_eargs", &[("e", m.ty(EDGE))], t::vec(m.ty(SARG)), |f| {
        f.let_("args", field(v("e"), "args"));
        f.var("out", vnew(named(SARG), vec![]));
        f.for_u64("k", u64c(0), vlen(v("args")), |f| {
            f.set("out", push(v("out"), rec(SARG, vec![boolc(false), at("args", v("k"))])));
        });
        f.ret(v("out"));
    });
    // Canonical case-key bytes: 1 || member id, or 2 || built-in tag.
    m.func("ck_ckey", &[("cs", m.ty(SCASE))], t::bytes(), |f| {
        f.ret(select(
            field(v("cs"), "member"),
            concat(vnew(t::bytes(), vec![bytesc(&[1]), field(v("cs"), "key")])),
            concat(vnew(
                t::bytes(),
                vec![bytesc(&[2]), call("uvarb", vec![field(v("cs"), "builtin")])],
            )),
        ));
    });
    // `expected_switch_cases`: (memo, code, keys, has payload, payloads).
    m.func(
        "ck_cases",
        &[
            ("inv", inv_ty.clone()),
            ("m", memo.clone()),
            ("sel", t::bytes()),
            ("pc", t::u64()),
        ],
        t::tuple(vec![
            memo.clone(),
            t::u32(),
            t::vec(t::bytes()),
            t::vec(t::bool()),
            t::vec(t::bytes()),
        ]),
        |f| {
            f.let_("parts", call("ty_parts", vec![v("sel")]));
            f.let_("tag", tget(v("parts"), 0));
            f.let_("pl", tget(v("parts"), 1));
            let fail = |f: &mut FnB<'_>, c: E| {
                f.ret(tup(vec![
                    v("m"),
                    c,
                    vnew(t::bytes(), vec![]),
                    vnew(t::bool(), vec![]),
                    vnew(t::bytes(), vec![]),
                ]));
            };
            let pair = |f: &mut FnB<'_>, k0: u8, k1: u8, has0: bool, t0: E, t1: E| {
                f.ret(tup(vec![
                    v("m"),
                    u32c(0),
                    vnew(t::bytes(), vec![bytesc(&[2, k0]), bytesc(&[2, k1])]),
                    vnew(t::bool(), vec![boolc(has0), boolc(true)]),
                    vnew(t::bytes(), vec![t0, t1]),
                ]));
            };
            f.if_(eq(v("tag"), u64c(13)), move |f| pair(f, 1, 2, false, bytesc(&[]), v("pl")));
            f.if_(eq(v("tag"), u64c(14)), move |f| {
                f.let_("fs", call("ty_fields", vec![v("pl")]));
                pair(f, 3, 4, true, at("fs", u64c(0)), at("fs", u64c(1)));
            });
            f.if_(ne(v("tag"), u64c(10)), move |f| fail(f, code(CFG_SWITCH_TYPE)));
            f.let_("np", call("ty_named_parts", vec![v("sel")]));
            f.let_("tdo", call("inv_tdef", vec![v("inv"), tget(v("np"), 0)]));
            f.if_(not(is_some(v("tdo"))), move |f| fail(f, code(TYPE_DEFINITION_UNKNOWN)));
            f.let_("td", unwrap(v("tdo")));
            f.if_(field(v("td"), "is_rec"), move |f| fail(f, code(CFG_SWITCH_TYPE)));
            f.let_("ms", field(v("td"), "members"));
            f.var("m", v("m"));
            f.var("ids", vnew(t::bytes(), vec![]));
            f.var("rh", vnew(t::bool(), vec![]));
            f.var("rty", vnew(t::bytes(), vec![]));
            f.for_u64("k", u64c(0), vlen(v("ms")), move |f| {
                f.let_("mem", at("ms", v("k")));
                f.set("ids", push(v("ids"), field(v("mem"), "member")));
                f.set("rh", push(v("rh"), field(v("mem"), "has_ty")));
                f.var("ty", bytesc(&[]));
                f.if_(field(v("mem"), "has_ty"), move |f| {
                    f.let_(
                        "y",
                        call(
                            "ck_inst",
                            vec![v("inv"), v("m"), field(v("mem"), "ty"), tget(v("np"), 1), v("pc")],
                        ),
                    );
                    f.set("m", tget(v("y"), 0));
                    f.if_(ne(tget(v("y"), 1), u32c(0)), move |f| fail(f, tget(v("y"), 1)));
                    f.set("ty", tget(v("y"), 2));
                });
                f.set("rty", push(v("rty"), v("ty")));
            });
            f.let_("ord", call("ck_order", vec![v("ids")]));
            f.var("keys", vnew(t::bytes(), vec![]));
            f.var("has", vnew(t::bool(), vec![]));
            f.var("tys", vnew(t::bytes(), vec![]));
            f.for_u64("k", u64c(0), vlen(v("ord")), |f| {
                f.let_("o", at("ord", v("k")));
                f.set(
                    "keys",
                    push(v("keys"), concat(vnew(t::bytes(), vec![bytesc(&[1]), at("ids", v("o"))]))),
                );
                f.set("has", push(v("has"), at("rh", v("o"))));
                f.set("tys", push(v("tys"), at("rty", v("o"))));
            });
            f.ret(tup(vec![v("m"), u32c(0), v("keys"), v("has"), v("tys")]));
        },
    );
    // `validate_terminator` (with `validate_switch`).
    m.func(
        "ck_term",
        &[
            ("inv", inv_ty.clone()),
            ("g", m.ty(CK_G)),
            ("m", memo.clone()),
            ("result", t::bytes()),
            ("term", m.ty(TERM)),
            ("src", t::u64()),
            ("pc", t::u64()),
        ],
        r(memo.clone()),
        |f| {
            f.let_("kind", field(v("term"), "kind"));
            f.let_("es", field(v("term"), "edges"));
            f.if_(and(eq(v("kind"), u64c(5)), not(field(v("term"), "has_val"))), |f| {
                f.ret_ok(v("m"));
            });
            f.if_(ne(v("kind"), u64c(2)), |f| {
                f.let_(
                    "x",
                    call(
                        "ck_res",
                        vec![v("g"), field(v("term"), "val"), v("src"), u64c(0), boolc(false)],
                    ),
                );
                f.if_(ne(tget(v("x"), 0), u32c(0)), |f| f.ret_err(tget(v("x"), 0)));
                f.let_("ty", tget(v("x"), 1));
                fail_if(f, and(eq(v("kind"), u64c(1)), ne(v("ty"), v("result"))), CFG_RETURN_TYPE);
                fail_if(f, and(eq(v("kind"), u64c(3)), ne(v("ty"), ty_bool())), CFG_BOOL_REQUIRED);
                f.if_(eq(v("kind"), u64c(5)), |f| {
                    fail_if(f, tget(call("ck_info", vec![v("ty")]), 1), CFG_TRAP_PAYLOAD);
                    f.let_("tr", call("ck_trait", vec![v("inv"), v("m"), v("ty")]));
                    f.if_(ne(tget(v("tr"), 1), u32c(0)), |f| f.ret_err(tget(v("tr"), 1)));
                    fail_if(
                        f,
                        eq(call("ck_tand", vec![tget(v("tr"), 2), u64c(8)]), u64c(0)),
                        CFG_TRAP_PAYLOAD,
                    );
                    f.ret_ok(tget(v("tr"), 0));
                });
                f.if_(eq(v("kind"), u64c(4)), |f| {
                    f.let_("cx", call("ck_cases", vec![v("inv"), v("m"), v("ty"), v("pc")]));
                    f.if_(ne(tget(v("cx"), 1), u32c(0)), |f| f.ret_err(tget(v("cx"), 1)));
                    f.let_("keys", tget(v("cx"), 2));
                    f.let_("cases", field(v("term"), "cases"));
                    fail_if(f, ne(vlen(v("cases")), vlen(v("keys"))), CFG_SWITCH_CASES);
                    f.for_u64("k", u64c(0), vlen(v("cases")), |f| {
                        fail_if(
                            f,
                            ne(call("ck_ckey", vec![at("cases", v("k"))]), at("keys", v("k"))),
                            CFG_SWITCH_CASES,
                        );
                    });
                    f.for_u64("k", u64c(0), vlen(v("cases")), |f| {
                        f.let_("cs", at("cases", v("k")));
                        f.expr(tri(call(
                            "ck_args",
                            vec![
                                v("g"),
                                v("src"),
                                field(v("cs"), "target"),
                                field(v("cs"), "args"),
                                vget(tget(v("cx"), 3), v("k")),
                                vget(tget(v("cx"), 4), v("k")),
                            ],
                        )));
                    });
                    f.ret_ok(tget(v("cx"), 0));
                });
            });
            // Branch (one edge) and cond_branch (both edges).
            f.for_u64("k", u64c(0), vlen(v("es")), |f| {
                f.let_("e", at("es", v("k")));
                f.expr(tri(call(
                    "ck_args",
                    vec![
                        v("g"),
                        v("src"),
                        field(v("e"), "target"),
                        call("ck_eargs", vec![v("e")]),
                        boolc(false),
                        bytesc(&[]),
                    ],
                )));
            });
            f.ret_ok(v("m"));
        },
    );
    // `validate_function_graph` with shared indices and memo.
    m.func(
        "ck_fn",
        &[
            ("inv", inv_ty.clone()),
            ("ix", m.ty(CK_IX)),
            ("m", memo.clone()),
            ("fid", t::bytes()),
        ],
        r(memo.clone()),
        |f| {
            f.let_("funo", call("inv_fun", vec![v("inv"), v("fid")]));
            fail_if(f, not(is_some(v("funo"))), GRAPH_UNRESOLVED_REFERENCE);
            f.let_("fun", unwrap(v("funo")));
            fn_body(f);
        },
    );
    // The declared blocks after the limit and ordinal checks: (block
    // positions, block records).
    m.func(
        "ck_fn_blocks",
        &[
            ("inv", inv_ty.clone()),
            ("fid", t::bytes()),
            ("fbl", t::vec(t::bytes())),
            ("nb", t::u64()),
        ],
        r(t::tuple(vec![bix_ty(), t::vec(m.ty(BLK))])),
        |f| {
            f.var("bix", mnew(t::bytes(), t::u64(), vec![]));
            f.for_u64("i", u64c(0), vlen(v("fbl")), |f| {
                f.let_("id", at("fbl", v("i")));
                fail_if(f, mhas(v("bix"), v("id")), GRAPH_DUPLICATE_ENTITY);
                f.set("bix", mins(v("bix"), v("id"), v("i")));
            });
            fail_if(f, ne(vlen(v("fbl")), v("nb")), GRAPH_INVENTORY_MISMATCH);
            f.var("lb", vnew(named(BLK), vec![]));
            f.for_u64("i", u64c(0), vlen(v("fbl")), |f| {
                f.let_("bo", call("inv_blk", vec![v("inv"), at("fbl", v("i"))]));
                fail_if(
                    f,
                    or(
                        not(is_some(v("bo"))),
                        ne(field(unwrap(v("bo")), "function"), v("fid")),
                    ),
                    GRAPH_INVENTORY_MISMATCH,
                );
                f.set("lb", push(v("lb"), unwrap(v("bo"))));
            });
            f.ret_ok(tup(vec![v("bix"), v("lb")]));
        },
    );
    m.func(
        "ck_function",
        &[("inv", inv_ty.clone()), ("fid", t::bytes())],
        r(t::unit()),
        |f| {
            f.expr(tri(call(
                "ck_fn",
                vec![
                    v("inv"),
                    call("ck_index", vec![v("inv")]),
                    call("ck_memo", vec![v("inv")]),
                    v("fid"),
                ],
            )));
            f.ret_ok(unitc());
        },
    );
    // `ck_types`, then every function in identity order.
    m.func("ck_all", &[("inv", inv_ty)], r(t::unit()), |f| {
        f.expr(tri(call("ck_types", vec![v("inv")])));
        f.let_("ix", call("ck_index", vec![v("inv")]));
        f.var("m", call("ck_memo", vec![v("inv")]));
        f.let_("fns", field(v("inv"), "fns"));
        f.for_u64("k", u64c(0), vlen(v("fns")), |f| {
            f.set(
                "m",
                tri(call(
                    "ck_fn",
                    vec![v("inv"), v("ix"), v("m"), field(at("fns", v("k")), "id")],
                )),
            );
        });
        f.ret_ok(unitc());
    });
}

/// The body of `ck_fn` once `fun` is bound.
#[allow(clippy::too_many_lines)]
fn fn_body(f: &mut FnB<'_>) {
    f.let_("fbl", field(v("fun"), "blocks"));
    f.let_("nfb", vlen(v("fbl")));
    f.let_("fps", field(v("fun"), "params"));
    f.let_("tps", field(v("fun"), "tparams"));
    let agg = |name: &str| call("ck_mget0", vec![field(v("ix"), name), v("fid")]);
    f.let_("nb", agg("nb"));
    // `check_top_level_limits`.
    fail_if(
        f,
        or(
            or(
                or(gt(v("nfb"), u64c(MAX_CFG_BLOCKS)), gt(v("nb"), u64c(MAX_CFG_BLOCKS))),
                or(
                    gt(vlen(v("fps")), u64c(MAX_CFG_PARAMETERS)),
                    gt(vlen(v("tps")), u64c(MAX_TYPE_ARGUMENTS)),
                ),
            ),
            or(
                or(gt(agg("no"), u64c(MAX_CFG_OPERATIONS)), gt(agg("big"), u64c(0))),
                or(gt(agg("nv"), u64c(MAX_CFG_VALUES)), gt(agg("nu"), u64c(MAX_CFG_USES))),
            ),
        ),
        CFG_RESOURCE_LIMIT,
    );
    // `validate_type_parameters`.
    f.for_u64("j", u64c(0), vlen(v("tps")), |f| {
        fail_if(f, ne(at("tps", v("j")), v("j")), GRAPH_ORDINAL_MISMATCH);
    });
    f.let_("pc", vlen(v("tps")));
    f.let_(
        "bl",
        tri(call("ck_fn_blocks", vec![v("inv"), v("fid"), v("fbl"), v("nb")])),
    );
    f.let_("bix", tget(v("bl"), 0));
    f.let_("lb", tget(v("bl"), 1));
    f.let_("epo", mget_opt(v("bix"), field(v("fun"), "entry")));
    fail_if(f, not(is_some(v("epo"))), CFG_ENTRY_INVALID);
    f.let_("ep", unwrap(v("epo")));
    f.expr(tri(call("ck_dups", vec![v("fps")])));
    f.let_(
        "pr",
        tri(call(
            "ck_fn_prm",
            vec![
                v("inv"),
                v("m"),
                v("fid"),
                v("fps"),
                v("bix"),
                v("lb"),
                v("pc"),
                agg("np"),
            ],
        )),
    );
    f.let_(
        "orr",
        tri(call(
            "ck_fn_ops",
            vec![v("inv"), tget(v("pr"), 0), v("bix"), v("lb"), v("pc"), agg("no")],
        )),
    );
    f.let_(
        "rc",
        call(
            "ck_ctype",
            vec![v("inv"), tget(v("orr"), 0), field(v("fun"), "result"), v("pc")],
        ),
    );
    f.if_(ne(tget(v("rc"), 1), u32c(0)), |f| f.ret_err(tget(v("rc"), 1)));
    f.let_("cfg", tri(call("ck_fn_cfg", vec![v("lb"), v("bix"), v("ep")])));
    f.let_(
        "g",
        rec(CK_G, vec![v("bix"), tget(v("pr"), 1), tget(v("orr"), 1), v("cfg")]),
    );
    f.expr(tri(call("ck_operands", vec![v("g"), v("nfb")])));
    f.var("m", tget(v("rc"), 0));
    f.let_("result", field(v("fun"), "result"));
    f.for_u64("i", u64c(0), v("nfb"), |f| {
        f.set(
            "m",
            tri(call(
                "ck_term",
                vec![
                    v("inv"),
                    v("g"),
                    v("m"),
                    v("result"),
                    field(at("lb", v("i")), "term"),
                    v("i"),
                    v("pc"),
                ],
            )),
        );
    });
    f.ret_ok(v("m"));
}

/// Operand resolution and the test entries.
fn define_entries(m: &mut Module) {
    // Operation operands in block/ordinal order.
    m.func(
        "ck_operands",
        &[("g", m.ty(CK_G)), ("nfb", t::u64())],
        r(t::unit()),
        |f| {
            f.let_("o", field(v("g"), "o"));
            f.let_("ob", field(v("o"), "ob"));
            f.let_("oi", field(v("o"), "oi"));
            f.let_("od", field(v("o"), "od"));
            f.for_u64("i", u64c(0), v("nfb"), |f| {
                f.let_("q0", at("ob", mul(v("i"), u64c(2))));
                f.for_u64("qq", u64c(0), at2("ob", mul(v("i"), u64c(2)), 1), |f| {
                    f.let_("q6", mul(add(v("q0"), v("qq")), u64c(6)));
                    f.let_("d0", at2("oi", v("q6"), 4));
                    f.for_u64("k", u64c(0), at2("oi", v("q6"), 5), |f| {
                        f.let_(
                            "x",
                            call(
                                "ck_res",
                                vec![
                                    v("g"),
                                    at("od", add(v("d0"), v("k"))),
                                    v("i"),
                                    at2("oi", v("q6"), 1),
                                    boolc(true),
                                ],
                            ),
                        );
                        f.if_(ne(tget(v("x"), 0), u32c(0)), |f| f.ret_err(tget(v("x"), 0)));
                    });
                });
            });
            f.ret_ok(unitc());
        },
    );
    // Test entries (never reached by the build driver).
    let objs = || ("objects", t::vec(t::bytes()));
    let epoch = || ("epoch", t::bytes());
    // The load alone (a baseline for instruction counts).
    m.func("t_ck_load", &[objs(), epoch()], r(t::u64()), |f| {
        f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
        f.ret_ok(vlen(field(v("inv"), "fns")));
    });
    m.func("t_ck_types", &[objs(), epoch()], r(t::unit()), |f| {
        f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
        f.ret(call("ck_types", vec![v("inv")]));
    });
    m.func(
        "t_ck_function",
        &[objs(), epoch(), ("fid", t::bytes())],
        r(t::unit()),
        |f| {
            f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
            f.ret(call("ck_function", vec![v("inv"), v("fid")]));
        },
    );
    m.func("t_ck_all", &[objs(), epoch()], r(t::unit()), |f| {
        f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
        f.ret(call("ck_all", vec![v("inv")]));
    });
    // Loads once: the `ck_types` code, then (when it passes) one `ck_fn`
    // code per listed function.
    m.func(
        "t_ck_both",
        &[objs(), epoch(), ("fids", t::vec(t::bytes()))],
        r(t::tuple(vec![t::u32(), t::vec(t::u32())])),
        |f| {
            f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
            f.match_res(
                call("ck_types", vec![v("inv")]),
                "_",
                |_| {},
                "e",
                |f| f.ret_ok(tup(vec![v("e"), vnew(t::u32(), vec![])])),
            );
            f.let_("ix", call("ck_index", vec![v("inv")]));
            f.var("m", call("ck_memo", vec![v("inv")]));
            f.var("out", vnew(t::u32(), vec![]));
            f.for_u64("k", u64c(0), vlen(v("fids")), |f| {
                f.match_res(
                    call("ck_fn", vec![v("inv"), v("ix"), v("m"), at("fids", v("k"))]),
                    "m2",
                    |f| {
                        f.set("m", v("m2"));
                        f.set("out", push(v("out"), u32c(0)));
                    },
                    "e",
                    |f| f.set("out", push(v("out"), v("e"))),
                );
            });
            f.ret_ok(tup(vec![u32c(0), v("out")]));
        },
    );
    // Loads once, then checks each listed function: one code per function.
    m.func(
        "t_ck_each",
        &[objs(), epoch(), ("fids", t::vec(t::bytes()))],
        r(t::vec(t::u32())),
        |f| {
            f.let_("inv", tri(call("drv_load", vec![v("objects"), v("epoch")])));
            f.let_("ix", call("ck_index", vec![v("inv")]));
            f.var("m", call("ck_memo", vec![v("inv")]));
            f.var("out", vnew(t::u32(), vec![]));
            f.for_u64("k", u64c(0), vlen(v("fids")), |f| {
                f.match_res(
                    call("ck_fn", vec![v("inv"), v("ix"), v("m"), at("fids", v("k"))]),
                    "m2",
                    |f| {
                        f.set("m", v("m2"));
                        f.set("out", push(v("out"), u32c(0)));
                    },
                    "e",
                    |f| f.set("out", push(v("out"), v("e"))),
                );
            });
            f.ret_ok(v("out"));
        },
    );
}
