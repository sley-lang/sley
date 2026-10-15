# ADR-0054: structured function bodies and checked integer conversion in AF1-X

Status: decisions 1–3 accepted for the workbench (2026-10-02); decision 4 proposed, awaiting the
maintainer. The contract is `docs/spec/SLEY_AGENT_V1.md` section 5.4.

Date: 2026-10-02

## Context

An AF1-X function is a list of blocks. A loop over a vector is therefore written as an entry block, a
header with a bounds test, a body, an index increment, a back edge, an exit, and a trap block for the
impossible failures, with every piece of state threaded through block parameters by hand. The
decisions in such a function are few (the initial state, the update, the result); the rest is
construction that a tool can derive exactly.

Sley also has no integer conversion: a `u64` vector index cannot become an `i64` result, and the
closed SSMC1 epoch-1 opcode set has no conversion opcode.

## Decision

1. **A function may carry `"body"` instead of `"blocks"`.** A body is a list of statements (`let`,
   `var`, `set`, `if`, `for`, `while`, `return`, `ok`, `fail`, `trap`) over expressions built from the
   existing operations plus `min`, `max`, `clamp`, `abs`, `len`, `get`, `field`, `call` and a lazy
   conditional value. `sley-agent` lowers the body to ordinary AF1-X blocks before expansion; the
   expander, compiler, kernel, canonical graph, admission and commit are unchanged, and the lowered
   blocks are inspectable with `view --after`.
2. **Semantics are Sley's, stated explicitly.** Arithmetic stays checked; a failure traps unless the
   operation is spelled `op?` (return the `ArithmeticError`) or `op?Case`. `and`/`or` evaluate both
   operands, as their opcodes do; `["if", c, a, b]` computes only the chosen value. Literals take the
   type of the other operand, the declared variable or the return type, else `i64`; integer types never
   mix implicitly. Unsupported or ambiguous constructs are refused at their authored location
   (`/fns/i/body/...`), and compiler refusals inside lowered blocks are mapped back to the statement
   that produced them.
3. **`["to", T, x]` converts between integer types without a new opcode.** It lowers to a bounded loop
   over the value's binary digits using checked operations of the target type: exact for every value
   that fits, and a value outside `T` fails like any checked operation (`to?`, `to?Case`). It takes at
   most one iteration per bit. A native conversion opcode is a canonical-format change and stays with
   the SSMC epoch process (`docs/spec/EPOCH_MIGRATION_POLICY_V1.md`).
4. **Proposed: a write-only familiar text frontend.** A small statement syntax that parses into exactly
   the `body` tree of decision 1 would be input only (never a canonical form, view or review surface),
   and adds no semantics. It conflicts with the anti-goal "Sley source syntax or parser"
   (`docs/ANTI_GOALS.md`); adopting it requires the maintainer to amend that anti-goal in a follow-up
   ADR. Until then the product accepts JSON bodies only.

## Consequences

- Authors state loop state, updates and results; block names, back edges, indexes and parameter
  threading are constructed and checked by the tool.
- A body function and a block function compile to the same kind of candidate; mixing both in one frame
  is allowed. `patch` still restates blocks; a body function is restated whole through `fns`.
- The lowering is deterministic: the same frame lowers to the same blocks.
- Conversions are exact but linear in the bit width; programs that convert in hot loops pay for it until
  a native opcode exists.
