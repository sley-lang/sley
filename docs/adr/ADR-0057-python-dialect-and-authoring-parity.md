# ADR-0057: the Python dialect of the familiar frontend, and authoring cost parity

Status: accepted 2026-10-13 under ADR-0055's policy, by the operator's direction to work toward
cost parity with other languages in Sley 2.1.1. Evidence is recorded in
`docs/release/SLEY-2.1.1.md`.

Date: 2026-10-13

MPI-0 disposition: `YES_MACHINE_BENEFICIAL`. The justification is measured machine utility: the
cost, in model tokens and money, of an agent authoring a correct Sley function. It is not human
familiarity or readability.

## Context

ADR-0055 admits optional, noncanonical authoring frontends "when justified by machine utility". They
must lower deterministically through the ordinary typed candidate and validation path, and their
syntax must never become canonical program authority. The familiar frontend implemented that policy
with a small statement syntax.

Sley 2.1.1 set out to reach cost parity with other languages when an agent writes a function. A
cross-language benchmark over several hosted models measured the cost per correct task for Sley,
Python, Go, Rust, C and C++. Sley's correctness already matched. Its cost did not, for two reasons:

- **Input.** An agent must be told how to write Sley; a mainstream language needs no explanation.
- **Output.** Models write more, and deliberate longer, in an unfamiliar syntax.

Making the familiar syntax shorter and more forgiving helped only partly. Letting models write the
typed Python they are most fluent in, lowered to the same structured bodies, closed most of the gap
(the measurements are in the release record).

## Decision

1. **The Python dialect.** The familiar frontend accepts a Python dialect, selected by a leading
   `def`, `import` or `from`. It lives in the same feature-gated module, behind the same
   `try --familiar` switch, and produces the same structured-body frame as the familiar syntax. Its
   contract is `docs/spec/SLEY_AGENT_V1.md` section 5.5.1.
2. **Python's meaning, or a refusal.** Where Sley can express Python's meaning exactly, the dialect
   keeps it:
   - floor division and modulo;
   - short-circuit `and` and `or`;
   - truthiness;
   - chained comparisons and membership;
   - slices and `zip` as read-only views.

   Where it cannot (floats, mutable lists, dicts, tuples, `break`), the text is refused at its line
   and column. Nothing is approximated silently.
3. **Integer types stay explicit.** Python's single integer type is `i64`. Lengths and indexes are
   converted with explicit `to` operations in the lowered frame. The anti-goal against implicit
   numeric conversion is unchanged: the conversion is written into the program, and a value outside
   the target type fails like a checked operation.
4. **New structured forms, no new semantics.** The structured body language gains `fold`,
   `floordiv`, `floormod`, `truthy`, `pow`, `gcd` and an untyped `var`. Each one lowers to existing
   statements and operations: loops, conditionals and checked arithmetic. The kernel, SSMC1, the
   opcode set, admission and commit are unchanged.
5. **Familiar-syntax ergonomics.** The familiar syntax also gains inferred and assignable
   variables, compound assignment, generator folds, tail values, `if` expressions and the C-family
   logical spellings.
6. **Short references.** `help card` (familiar syntax) and `help python` (the dialect) are compact
   references for an author's system prompt. The first line of `help python` is a one-line note.

## Consequences

- A committed program is a Sley graph whichever syntax wrote it. No text is stored as program state,
  and the dialect can be removed by building without the `familiar` feature.
- The confinement checked by `scripts/build_anti_goal_conformance.py` holds unchanged. The frontend
  is one module that imports only `std` and `serde_json`, behind a feature that enables nothing, and
  no other crate names it.
- Programs keep Sley's fixed-width checked integers: an overflow traps, which Python's unbounded
  integers would not.
- The dialect is a frontend, not Python compatibility: unsupported Python is refused, never run.
