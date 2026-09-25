# Sley agent guide

A Sley program is a graph of typed entities (functions, blocks, operations,
types, constants, tests) in a repository. There are no source files. You
read the program as an AV1 view, and you change it by writing an AF1 frame:
JSON that `sley-agent` compiles into one candidate, which the Sley kernel
validates as a whole. Names like `percent.entry.zero` are local handles;
the kernel sees only identities.

## Commands

    sley-agent view [name...]      # AV1 view of the program or of named entities
    sley-agent find [text]         # list functions, types, constants, tests
    sley-agent try frame.json      # compile + validate + run tests; prints a handle (c1)
    sley-agent submit c1           # write final_candidate.hex (repeatable; the last wins)
    sley-agent status              # the current submission, its code and its tests
    sley-agent call f 1 2          # run f on the head (--on c1: on a candidate)
    sley-agent test c1             # run every TestCase in c1's state
    sley-agent explain c1          # why c1 was refused, with the affected code
    sley-agent view --after c1     # the functions c1 changes, as proposed
    sley-agent help af1|types|tests|opcodes|refusals

`try` takes a file, `-` for stdin, or inline JSON. A candidate is always
relative to the current head, so one frame carries your whole change: after
a refusal, fix the frame and run `try` again. Add `--json` to any command
for machine-readable output.

## Reading AV1

    fn percent(part: i64, whole: i64) -> Result<i64,MathError>   [a1b2c3d4]
      entry:
        zero = const k_0 (0)
        is_zero = eq whole, zero
        cond is_zero -> zero_whole, scale
      divide(scaled: i64):
        q = div scaled, whole
        switch q: Ok -> done($), Err -> overflow

Each operation line is `value = opcode operands`. A block may take
parameters. Terminators are `return v`, `br block(args)`,
`cond c -> then, else`, `switch v: Case -> block(args), ...` and
`trap unreachable`. In a switch, `$` is the case payload. A value from
another block prints as `block.value`.

## Writing a frame

This frame adds an error type, a function, and three tests:

```json
{"af1": 1,
 "types": [{"name": "MathError", "variant": ["ZeroWhole", "Overflow"]}],
 "fns": [{"fn": "percent", "params": [["part", "i64"], ["whole", "i64"]],
          "returns": "Result<i64,MathError>",
          "blocks": [
   {"name": "entry", "ops": [["zero", "const", 0], ["is_zero", "eq", "whole", "zero"]],
    "term": ["cond", "is_zero", "zero_whole", "scale"]},
   {"name": "zero_whole", "ops": [["e", "variant", "MathError.ZeroWhole"], ["r", "err", "e"]],
    "term": ["return", "r"]},
   {"name": "scale", "ops": [["hundred", "const", 100], ["p", "mul", "part", "hundred"]],
    "term": ["switch", "p", ["Ok", "divide", "$"], ["Err", "overflow"]]},
   {"name": "divide", "params": [["scaled", "i64"]], "ops": [["q", "div", "scaled", "whole"]],
    "term": ["switch", "q", ["Ok", "done", "$"], ["Err", "overflow"]]},
   {"name": "done", "params": [["v", "i64"]], "ops": [["r", "ok", "v"]], "term": ["return", "r"]},
   {"name": "overflow", "ops": [["e", "variant", "MathError.Overflow"], ["r", "err", "e"]],
    "term": ["return", "r"]}]}],
 "tests": [{"fn": "percent", "args": [1, 4], "expect": {"Ok": 25}},
           {"fn": "percent", "args": [1, 0], "expect": {"Err": "ZeroWhole"}},
           {"fn": "percent", "args": [9223372036854775807, 2], "expect": {"Err": "Overflow"}}]}
```

- An operation is `["name", "opcode", immediate?, operands...]`. The tool
  derives result types, ordinals, owner lists and identities.
- `fns` defines a whole function. If the name exists, the entities you name
  again keep their identities, and blocks or operations you leave out are
  deleted. The first block is the entry block.
- Operands are value names: parameters, block parameters, and results. Use
  `block.name` for a value from another block, but only if that block
  dominates the use. Otherwise pass the value as an edge argument into a
  block parameter: `["br", ["join", "x"]]`, `["cond", "c", ["t", "x"], "f"]`.
- Checked integer operations (`add`, `sub`, `mul`, `div`, `rem`, `neg`) return
  `Result<T,ArithmeticError>`. Branch on them with `switch`. `div` truncates
  toward zero. Comparisons (`eq`, `ne`, `lt`, `le`, `gt`, `ge`) return `bool`.
- `const` takes a constant name or a literal (`0`, `true`,
  `{"type": "u8", "value": 5}`). Literals reuse an equal constant.
- `call` takes the callee first: `["t", "call", "percent", "a", "b"]`.
- Result and variant values: `ok`, `err`, and `variant Type.Case`.

To change existing code, `edit` one operation or `patch` whole blocks:

```json
{"af1": 1, "edit": [{"fn": "percent", "replace_op": "scale.hundred", "with": ["const", 1000]}]}
```

```json
{"af1": 1, "patch": [{"fn": "percent", "blocks": {"done": {"params": [["v", "i64"]],
  "ops": [["one", "const", 1], ["w", "add", "v", "one"]],
  "term": ["switch", "w", ["Ok", "wrapped", "$"], ["Err", "overflow"]]},
  "wrapped": {"params": [["v", "i64"]], "ops": [["r", "ok", "v"]], "term": ["return", "r"]}}}]}
```

A patch block restates that block. Blocks you don't mention stay as they
are, and `null` deletes a block. `delete: ["name"]` removes a function,
type, constant or test, along with the tests that target a deleted function.

## Tests

Tests are TestCase entities: `{"fn", "args", "expect"}` plus an optional
`"name"`. `expect` is a value (`25`, `true`), `{"Ok": v}`, `{"Err": "Case"}`,
or `{"trap": "unreachable"}`. Resource limits default to the policy grant.
Every `try` runs the tests its candidate touches and shows the expected and
actual values for any test that fails.

## Refusals

A refused `try` prints the decision, the phase, the kernel's symbol, where
it applies, and a hint:

    c2: REFUSED ControlFlowError at phase 7 (control flow)
      symbol: CFG_DOMINANCE (22016)
      where: Function f: operand 0 of f.join.r uses `left.m` defined in block f.left, ...
      hint: A value is used in a block its definition does not dominate. ...

Fix what `where` names and run `try` again. For the full list, run
`sley-agent help refusals`.
