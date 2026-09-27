# AF1-X, the authoring dialect (reference)

An AF1 frame with `"afx": 1` may use shorter forms inside function blocks
(`fns` and `patch`). `try` expands them into plain AF1, then compiles that
exactly like any AF1 frame, so the kernel still judges the whole candidate.
`edit.with` stays plain AF1: restate the block with `patch` instead. A frame
without `"afx": 1` is plain AF1 and refuses these forms.

## Operands

After an operation's immediate, and in `return`, `cond`, `switch`, `br`,
`trap`, `ok` and `fail`, an operand may be:

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
Option: checked arithmetic, `call`, `vec_get`, `map_get`, `map`.

## Exits and terminators

    ["!Negative", "if", ["lt", "a", 0]]      leave through a case or a handler block
    ["!Code", "if", ["gt", "a", 9], "a"]     with the payload the case takes
    ["ok", v]                                terminator: return Ok(v)
    ["fail", "Case"] or ["fail", "Case", p]  terminator: return Err(Case)
    ["fail"]                                 terminator: return None (Option functions)

## Names across blocks

A plain name is an earlier value of the same block, else a function
parameter, else the one operation result of that name in a block that
dominates this point (it is qualified for you). Parameters of other blocks
and checked values of other blocks are not visible: declare a parameter.
When an edge passes fewer arguments than its target takes, each missing
trailing argument is the value of the parameter's name visible at the edge,
when its type fits. An edge back into a loop never passes the loop block's
own value: write that argument.

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

## Generated names

    n__a<k>          operand k of operation n (nesting appends: n__a0__a1)
    <b>__t<k>        operand k of block b's terminator
    <b>__if<i>       block b's i-th exit: its condition, and the block after it
    x__r             the Result or Option of checked operation x
    <b>__<x>         the block after checked operation x in block b
    <b>__ok          the value an ok terminator returns
    __fail_<Case>, __err, __none    exits shared by a function

A name that is taken gets `_2`, `_3`, ... Do not use `__` in your own
names. A refusal names the authored position, with the expanded one in
brackets: `/fns/0/blocks/0/ops/1: ... [expanded /fns/0/blocks/1/ops/0]`.

## Refusals

    AGENT_X_PROPAGATION       no single, type-correct failure route for ? or !
    AGENT_X_SCOPE             a name is ambiguous, not available, or not passed
    AGENT_X_LIMIT             nesting (32), operations (4096) or generated blocks (1024)
    AGENT_TEST_TABLE_INVALID  a malformed, duplicated or colliding table row
    AGENT_RIPPLE_INTENT_UNKNOWN  ripple is not enabled in this build
