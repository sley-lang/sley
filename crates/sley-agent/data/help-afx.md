# AF1-X, the authoring dialect (reference)

An AF1 frame with `"afx": 1` may use shorter forms inside function blocks
(`fns` and `patch`). `try` expands them into plain AF1, then compiles that
exactly like any AF1 frame, so the kernel still judges the whole candidate.
`edit.with` stays plain AF1: restate the block with `patch` instead. A frame
without `"afx": 1` is plain AF1 and refuses these forms.

## Operands

After an operation's immediate, and in `return`, `cond`, `switch`, `br`,
`ok`, `fail` and a trap's payload, an operand may be:

    "x"                           a value name, as in AF1
    3, -7, 2.5, true              a literal; the context gives its type
    {"type": "u8", "value": 3}    a literal with its type stated
    ["add", "a", 1]               a nested operation (no name)

Strings are always names. A bare number takes its type from the other
operand of a same-type operation, the callee's parameter, the target
block's parameter, the function result (`return`, `ok`, a `fail` payload),
a variant payload or a record field; with none of these, state the type.
Nested operations run left to right, depth first, before the operation
that uses them. In `cond`/`switch` target arguments and in an exit's
payload only names and literals may appear (they run on one path only).
A trap is `["trap"]`, `["trap", code]` or `["trap", code, payload]`, its
code a word (`unreachable`, ...): anything else is refused, never dropped.

## Checked operations

    ["q", "div?Zero", "a", "b"]   q is the Ok value; a failure goes to Zero
    ["q", "div?", "a", "b"]       a failure is returned unchanged

The suffix names a case of the function's error type (`Result<T,Error>`,
`Error` a variant) or a handler block, never both. A case with a payload
receives the failure payload when the types match; a case without one drops
it. A handler block's first parameter receives the payload (a handler
without parameters drops it); its other parameters are derived. A bare `?`
needs a Result function with the same error type, or an Option operation
in an Option function. `?` works on any operation that gives a Result or an
Option: checked arithmetic, `call`, `vec_get`, `vec_set`, `map_get`, `variant_get`.

## Exits and terminators

    ["!Negative", "if", ["lt", "a", 0]]      leave through a case or a handler block
    ["!Code", "if", ["gt", "a", 9], "a"]     with the payload the case takes
    ["ok", v]                                terminator: return Ok(v)
    ["fail", "Case"] or ["fail", "Case", p]  terminator: return Err(Case)
    ["fail"]                                 terminator: return None (Option functions)

## Names across blocks

A plain name is an earlier value of the same block, else a function
parameter, else the nearest definition of that name among the blocks every
path to this point passes through; when that is an operation result, and
no other block defines the name on a path from there to here, it is
qualified for you. Parameters and checked values of other blocks are not
visible: declare a parameter (and when one shadows an earlier result of the
same name, or another block redefines it on the way, write `block.name` to
mean that result). `b.x` is block `b`'s own `x`, exactly as in plain AF1.

When an edge passes fewer arguments than its target takes, each missing
trailing argument is the value of the parameter's name visible at the edge,
when its type fits. Two exceptions: an edge back into a loop (into a block
the edge is inside of) takes no argument by name, so write them all; and a
switch case whose value carries a payload passes it only with `$`, so
write `$` where the target takes it.

```json
{"af1": 1, "afx": 1,
 "types": [{"name": "PackError", "variant": ["NoItems", "Overflow"]}],
 "fns": [{"fn": "pack_bytes", "params": [["count", "i64"], ["size", "i64"]],
          "returns": "Result<i64,PackError>",
          "blocks": [{"name": "entry",
                      "ops": [["!NoItems", "if", ["lt", "count", 1]],
                              ["bytes", "mul?Overflow", "count", "size"]],
                      "term": ["ok", "bytes"]}]}],
 "test_tables": [{"name": "t_pack", "fn": "pack_bytes",
                  "cases": [{"args": [2, 5], "expect": {"Ok": 10}},
                            {"args": [0, 5], "expect": {"Err": "NoItems"}},
                            {"args": [9223372036854775807, 2], "expect": {"Err": "Overflow"}}]}]}
```

```json
{"af1": 1, "afx": 1,
 "types": [{"name": "DivError", "variant": ["Zero", ["Math", "ArithmeticError"]]}],
 "fns": [{"fn": "ratio", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,DivError>",
          "blocks": [{"name": "entry", "ops": [["!Zero", "if", ["eq", "b", 0]]],
                      "term": ["ok", ["div?Math", ["mul?Math", "a", 100], "b"]]}]},
         {"fn": "second", "params": [["v", "Vec<i64>"]], "returns": "Option<i64>",
          "blocks": [{"name": "entry", "ops": [["x", "vec_get?", "v", {"type": "u64", "value": 1}]],
                      "term": ["return", ["some", ["add?wrap", "x", 1]]]},
                     {"name": "wrap", "term": ["fail"]}]}],
 "test_tables": [{"name": "t_ratio", "fn": "ratio",
                  "cases": [{"args": [3, 4], "expect": {"Ok": 75}},
                            {"args": [3, 0], "expect": {"Err": "Zero"}},
                            {"args": [9223372036854775807, 3], "expect": {"Err": {"Math": {"ArithmeticError": "Overflow"}}}}]},
                 {"name": "t_second", "fn": "second",
                  "cases": [{"args": [[4, 5]], "expect": {"Some": 6}}, {"args": [[4]], "expect": "None"}]}]}
```

```json
{"af1": 1, "afx": 1,
 "types": [{"name": "SumError", "variant": ["Negative", "Overflow"]}],
 "fns": [{"fn": "sum_to", "params": [["n", "i64"]], "returns": "Result<i64,SumError>",
          "blocks": [
   {"name": "entry", "ops": [["!Negative", "if", ["lt", "n", 0]]], "term": ["br", "loop", 0, 1]},
   {"name": "loop", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["more", "le", "i", "n"]],
    "term": ["cond", "more", "body", "done"]},
   {"name": "body", "params": [["acc", "i64"], ["i", "i64"]],
    "ops": [["next", "add?Overflow", "acc", "i"]], "term": ["br", "loop", "next", ["add?Overflow", "i", 1]]},
   {"name": "done", "params": [["acc", "i64"]], "term": ["ok", "acc"]}]}],
 "test_tables": [{"name": "t_sum", "fn": "sum_to",
                  "cases": [{"args": [4], "expect": {"Ok": 10}}, {"args": [0], "expect": {"Ok": 0}},
                            {"args": [-1], "expect": {"Err": "Negative"}}]}]}
```

In `sum_to`, `["br", "loop", 0, 1]` types its literals from `loop`'s
parameters, and `cond` passes `acc` and `i` to `body` and `acc` to `done`
without writing them.

## Test tables

    "test_tables": [{"name": "t_f", "fn": "f", "defaults": {"limits": {"fuel": 10000}},
                     "cases": [{"args": [...], "expect": ..., "name": "optional", "limits": {...}}]}]

Each row becomes one AF1 test named by its `name`, else `<table>_<i>` (`i`
from 0). Row limits override the table's defaults; the AF1 test rules then
apply unchanged. `try --on` replaces a table by its name.

A row may name a live test only when the same table made it, as it is
now, in this draft (`try --on`, `fill`); then the row updates it.
Otherwise it is refused: give the row a "name", or delete the live test
explicitly ("delete": ["t_0"]). Restating a table without a row it made
deletes that row's live test.

## Generated names

    n__a<k>          operand k of operation n (nesting appends: n__a0__a1)
    <b>__t<k>        operand k of block b's terminator
    <b>__if<i>       block b's i-th exit: its condition, and the block after it
    x__r             the Result or Option of checked operation x
    <b>__<x>         the block after checked operation x in block b
    <b>__ok          the value an ok terminator returns
    __fail_<Case>, __err, __none    exits shared by a function

A name that is taken gets `_2`, `_3`, ... `__` is reserved for generated
names in blocks that use these forms (and in their function's parameters):
a plain block, for example one a plain frame defined and a follow-up
restates, keeps the names AF1 allows. A refusal names the authored position, with the expanded one in
brackets: `/fns/0/blocks/0/ops/1: ... [expanded /fns/0/blocks/1/ops/0]`.

## Ripple

    "ripple": [{"arity": "f"}, {"arity": "f", "value": 0, "frame_calls": "old"},
               {"guard": "g", "arg": "p", "in": ["f", ...], "mode": "preserve"},
               {"guard": "g", "arg": "p", "in": ["f", ...], "mode": "entry", "handler": "b"}]

A ripple intent states a change once, and `try` derives the edits it
implies into the frame. Intents apply in order, after the frame's own
definitions, and the kernel judges the whole candidate as usual. An intent
the head already reflects (a committed revision derived it) is reported as
`already applied` and derives nothing, so a follow-up such as tests on
`try --on dN --rebase` needs no change to it. In `try --on`, a follow-up's
intent replaces the draft's intent of the same kind and target (`arity` of
the same function, `guard` with the same checker and `arg`); others are
added after them. See the
derived edits with `sley-agent draft <d> --expanded`; `ripple.json` in the
draft lists the changed entities and the boundaries met. A decision ripple
cannot make is an `AGENT_RIPPLE_*` obligation that points into the intent.

`{"arity": f}`: the frame restates the parameters of the live function `f`
(in `fns`, or in a `patch` with `params`). Every call of `f` in live code
and every TestCase of `f` get their arguments by parameter name: a kept
parameter keeps its argument, a removed one drops it, and a new one takes
`"value"` (a literal of its type) or is an obligation. A call or test the
frame itself writes (in this revision or an earlier one) is kept when it
passes the new number of arguments and rewritten by name when it passes the
old number; when both numbers are the same (a reorder or a rename), say how
they are written: `"frame_calls": "old"` rewrites them by name, `"new"` keeps
them, and without it each is an obligation. Ripple stops
(`AGENT_RIPPLE_EXPORTED_BOUNDARY`) when `f` is an entry point, a package
export (or in a namespace a package exports), a global's initializer, or
named by a contract or policy binding, and at a call it would rewrite in
another namespace, live or restated by the frame. A use of `f` as a value
(`fnref`, or a constant or TestCase holding it) is an obligation.

`{"guard": g, "arg": p, "in": [f, ...]}`: `g` is a checker `P -> Result<P,E>`,
defined in the same frame or live. `"mode": "preserve"` (the default)
replaces, in each live `f`, an inline check of parameter `p` that runs the
same operations as `g` in the same order, at the end of a block, and fails
with the same errors, by a call of `g` at that place; anything else is
`AGENT_RIPPLE_GUARD_ORDER`. Constants and functions are compared as the
frame defines them. `"mode": "entry"` calls `g(p)` once when `f` starts,
before anything else `f` does: when `g` fails, `f` fails with `g`'s error on
every path, including paths that returned early, trapped or failed another
way before reaching a check of `p`; when `g` succeeds, `f` runs as before
and every use of `p` reads the checked value (a block the error also
reaches keeps `p`). The error is returned unchanged when `f` returns
`Result<_,E>`; otherwise name the block of `f` that takes `(e: E)` with
`"handler"`, or it is an obligation. Entry is refused when `f` already
evaluates `g` (on any value, directly or through the functions it calls),
when `g` calls `f`, or when `f` performs effects. A checker has at most 64
blocks, and preserve makes at most 1024 comparisons per function
(`AGENT_RIPPLE_LIMIT`).

`effect`, `member`, `retype`, `move` and `prune` are not enabled in this
build (`AGENT_RIPPLE_INTENT_UNKNOWN`).

Given these live functions:

```json
{"af1": 1, "afx": 1,
 "types": [{"name": "OrderError", "variant": ["InvalidQuantity", "InvalidPrice", "Overflow"]}],
 "fns": [{"fn": "line_total", "params": [["quantity", "i64"], ["price", "i64"]], "returns": "Result<i64,OrderError>",
          "blocks": [{"name": "entry",
                      "ops": [["!InvalidPrice", "if", ["lt", "price", 0]],
                              ["!InvalidQuantity", "if", ["lt", "quantity", 1]],
                              ["total", "mul?Overflow", "quantity", "price"]],
                      "term": ["ok", "total"]}]},
         {"fn": "order_total", "params": [["quantity", "i64"], ["price", "i64"]], "returns": "Result<i64,OrderError>",
          "blocks": [{"name": "entry", "ops": [["t", "call?", "line_total", "quantity", "price"]], "term": ["ok", "t"]}]}]}
```

this frame adds a `fee` parameter to `line_total`; the call in `order_total`
passes `0` for it:

```json
{"af1": 1, "afx": 1,
 "patch": [{"fn": "line_total", "params": [["quantity", "i64"], ["price", "i64"], ["fee", "i64"]],
            "blocks": {"entry": {"ops": [["!InvalidPrice", "if", ["lt", "price", 0]],
                                         ["!InvalidQuantity", "if", ["lt", "quantity", 1]],
                                         ["total", "mul?Overflow", "quantity", "price"]],
                                 "term": ["ok", ["add?Overflow", "total", "fee"]]}}}],
 "ripple": [{"arity": "line_total", "value": 0}],
 "test_tables": [{"name": "t_fee", "fn": "line_total", "cases": [{"args": [2, 5, 1], "expect": {"Ok": 11}}]},
                 {"name": "t_order", "fn": "order_total", "cases": [{"args": [2, 5], "expect": {"Ok": 10}}]}]}
```

and this one moves the quantity check of `line_total` into a checker, at
the same place (the price check still comes first):

```json
{"af1": 1, "afx": 1,
 "fns": [{"fn": "check_quantity", "params": [["q", "i64"]], "returns": "Result<i64,OrderError>",
          "blocks": [{"name": "entry", "ops": [["!InvalidQuantity", "if", ["lt", "q", 1]]], "term": ["ok", "q"]}]}],
 "ripple": [{"guard": "check_quantity", "arg": "quantity", "in": ["line_total"]}],
 "test_tables": [{"name": "t_line", "fn": "line_total",
                  "cases": [{"args": [0, -1], "expect": {"Err": "InvalidPrice"}},
                            {"args": [0, 5], "expect": {"Err": "InvalidQuantity"}},
                            {"args": [2, 5], "expect": {"Ok": 10}}]}]}
```

## Refusals

    AGENT_X_PROPAGATION       no single, type-correct failure route for ? or !
    AGENT_X_SCOPE             a name is ambiguous, not available, or not passed
    AGENT_X_LIMIT             nesting (32), operations (4096) or generated blocks (1024)
    AGENT_TEST_TABLE_INVALID  a malformed, duplicated or colliding table row, or one
                              naming a live test its table did not make
    AGENT_RIPPLE_INTENT_UNKNOWN     an unknown intent, or one not enabled in this build
    AGENT_RIPPLE_TARGET_KIND        not a live function the intent can change
    AGENT_RIPPLE_HOLE_UNFILLED      a value or route ripple cannot derive
    AGENT_RIPPLE_EXPORTED_BOUNDARY  a use outside the program's calls, or in another namespace
    AGENT_RIPPLE_LIMIT              32 intents, 256 call sites and tests per intent, 64 guarded
                                    functions, 64 checker blocks, 1024 preserve comparisons
    AGENT_RIPPLE_GUARD_SHAPE        the checker is not P -> Result<P,E>
    AGENT_RIPPLE_GUARD_ORDER        no identical check to replace, or an order that cannot be kept
