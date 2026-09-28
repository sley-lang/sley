# Sley agent guide

A Sley program is a graph of typed entities. Read it with `view`; change
it with a JSON frame that `sley-agent` expands, compiles and validates.

    sley-agent find                  # functions, types, constants, tests
    sley-agent view --focus f --x    # f compactly, with its context
    sley-agent try frame.json        # validate + run tests: candidate c1, draft d1
    sley-agent try --on d1 more.json # a follow-up on top of d1: tests, one fix
    sley-agent fill d1 fix.json --revision 1   # repair only what was refused
    sley-agent submit d1             # d1's latest revision, Valid and tested

## Writing a frame

```json
{"af1": 1, "afx": 1,
 "types": [{"name": "ShapeError", "variant": ["BadSide", "Overflow"]}],
 "fns": [{"fn": "area", "params": [["w", "i64"], ["h", "i64"]],
          "returns": "Result<i64,ShapeError>",
          "blocks": [{"name": "entry",
            "ops": [["!BadSide", "if", ["lt", "w", 1]],
                    ["!BadSide", "if", ["lt", "h", 1]],
                    ["a", "mul?Overflow", "w", "h"]],
            "term": ["ok", "a"]}]}],
 "test_tables": [{"name": "area_cases", "fn": "area", "cases": [
   {"args": [3, 4], "expect": {"Ok": 12}},
   {"args": [0, 4], "expect": {"Err": "BadSide"}},
   {"args": [9223372036854775807, 2], "expect": {"Err": "Overflow"}}]}]}
```

- An operation is `["name", "opcode", operands...]`. Operands are names,
  literals (`1`, `true`) or nested operations (`["lt", "w", 1]`), run left
  to right.
- `op?Case` unwraps a checked result (`add sub mul div rem neg`, a `call`
  returning Result or Option); a failure returns `Err(Case)`. `op?` passes
  the failure on; `op?block` goes to a handler block.
  Plain arithmetic returns Result: unwrap each step before using it in
  another operation, e.g. `add?Overflow` then `div?Overflow`.
- `["!Case", "if", cond]` returns `Err(Case)` when `cond` is true.
- Terminators: `["ok", v]`, `["fail", "Case"]`, `["return", v]`,
  `["br", ["join", "x"]]`, `["cond", "c", ["t", "x"], "f"]`,
  `["switch", "v", ["Ok", "next", "$", "x"], ...]` (`$` is the payload).
- A name is a value of its block, a parameter, or the nearest earlier
  operation result. Block parameters and `?` values stay in their block:
  pass them on edges. An omitted trailing edge argument takes the value of
  its name, except on loop edges and for `$` (see `help afx`).
- `__` is reserved. Rows of `test_tables` are `{"args", "expect"}`;
  `expect` is a value, `{"Ok": v}`, `{"Err": "Case"}` or `{"trap": "unreachable"}`.

## Changing code

State only the change and layer it on the draft (`try --on d1 more.json`):

```json
{"af1": 1, "afx": 1, "edit": [{"fn": "area", "replace_op": "entry.a", "with": ["mul?Overflow", "h", "w"]}]}
```

`edit` replaces one operation, `patch` restates blocks, `fns` redefines.
For a function the draft does not define, `edit` takes plain AF1: use
`patch`. Earlier tests stay.

## Refusals and repair

A refused `try` keeps a draft and lists problem pointers. Repair them:

```json
{"set": [{"at": "/fns/0/blocks/0/term", "value": ["ok", "a"]}]}
```

`fill d1 fix.json --revision 2` replaces exactly those subtrees and tries
again. `draft d1` shows its state and obligations; `--expanded` the plain
frame derived from yours. A kernel refusal prints the phase, symbol,
`where:`, `authored:` and a hint. `submit` refuses a change no test covers
(`--untested` overrides).
More: `sley-agent help afx|af1|drafts|tests|search|types|opcodes|refusals`.
