# AF1-X quick reference

Author one JSON object with `"af1":1,"afx":1`. `fns` defines functions;
`types` defines named records/variants; `test_tables` supplies example tests.
`sley-agent try frame.json` expands and validates it. Use `call NAME --on c1`
to run a candidate. See `help guide` for drafts, repair and submission.

A function has `fn`, `params` (pairs of name and type), `returns`, and
`blocks`. The first block is the entry. A block has `name`, optional
`params`, optional `ops`, and `term`. Names beginning with `__` are reserved.

Types: `i8/i16/i32/i64/i128`, unsigned `u8` through `u128`, `bool`, `text`,
`Vec<T>`, `Option<T>`, `Result<T,E>`, or a named type. Define a record with
`{"name":"Point","record":[["x","i64"],["y","i64"]]}`; read a field with
`["x","field","Point.x","p"]`. Define an error variant with
`{"name":"Error","variant":["BadInput","Overflow"]}`.

An operation is `["name","opcode",operands...]`. An operand is a value
name, a literal, or a nested operation without a result name (`["lt","x",0]`).
Strings are names; a text literal uses `{"type":"text","value":"hello"}`.
Numbers infer their type from operands, the destination or function result;
use `{"type":"u64","value":0}` when no context determines it.

- `eq ne lt le gt ge` return `bool`; `eq` compares text exactly.
- `not and or` operate on booleans.
- `add sub mul div rem neg shl shr` are checked integer operations returning
  `Result<T,ArithmeticError>`. `div` truncates toward zero.
- `vec_len` returns `u64`; `vec_get` takes a `u64` index and returns `Option<T>`.
- `op?handler` unwraps Ok/Some or branches to a named handler block on failure.
  A handler with no parameters drops the failure payload. A handler may trap
  with `["trap"]`. `op?Case` instead returns a named error case from a Result
  function. Bare `op?` propagates a matching Result/Option failure.

Terminators: `["return",value]`, `["ok",value]`, `["fail","Case"]`,
`["br","target",args...]`, `["cond",condition,["yes",args...],["no",args...]]`,
or `["switch",value,["Ok","next","$"],["Err","failure"]]`.
A switch covers every case; `$` is its payload. Use Some/None for Option.
Nested operations are allowed in `return`, `ok` and `br` arguments; a
`cond`/`switch` target argument must be a name or literal.

Function parameters are visible throughout. Block parameters and values
unwrapped with `?` stay in their block: pass them explicitly on outgoing
edges. Ordinary operation results can be referenced as `block.name` where
that block dominates the use. Always pass every argument on a loop back edge.

This fold multiplies a vector, returns 1 for an empty vector, and propagates
arithmetic overflow. Explicit edge arguments show where loop state goes.

```json
{"af1":1,"afx":1,
 "fns":[{"fn":"product","params":[["items","Vec<i64>"]],
 "returns":"Result<i64,ArithmeticError>","blocks":[
  {"name":"entry","ops":[["n","vec_len","items"]],
   "term":["br","loop",0,1]},
  {"name":"loop","params":[["i","u64"],["acc","i64"]],
   "term":["cond",["lt","i","entry.n"],["body","i","acc"],["done","acc"]]},
  {"name":"body","params":[["i","u64"],["acc","i64"]],
   "ops":[["x","vec_get?bounds","items","i"],
          ["next","mul?","acc","x"],["j","add?bounds","i",1]],
   "term":["br","loop","j","next"]},
  {"name":"done","params":[["acc","i64"]],"term":["ok","acc"]},
  {"name":"bounds","term":["trap"]}]}],
 "test_tables":[{"name":"product_cases","fn":"product","cases":[
  {"args":[[]],"expect":{"Ok":1}},
  {"args":[[2,-3,4]],"expect":{"Ok":-24}},
  {"args":[[9223372036854775807,2]],"expect":{"Err":{"ArithmeticError":"Overflow"}}}]}]}
```

The full references are `help afx`, `help af1`, `help types` and `help opcodes`.
