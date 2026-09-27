# ADR-0053: decision-only authoring in the workbench (AF1-X, drafts, delta repair, test tables)

Status: accepted for the workbench (2026-09-27). The contract is `docs/spec/SLEY_AGENT_V1.md`
revision 2.

Date: 2026-09-27

## Context

Writing a correct change through the 2.0.2 workbench costs more author
output than the change's decisions need. Three kinds of work dominate:

- mechanical control flow: a `switch` and two blocks for every checked
  operation, block parameters threaded by hand, one named `const` per
  literal;
- whole-frame rewrites: a refused frame is corrected by resubmitting all
  of it, tests included;
- repeated reading: the default views reprint unchanged bodies and every
  passing test.

These are interface costs, not language properties. The constitution
(ADR-0001, `docs/ANTI_GOALS.md`) still forbids a source syntax, a parser,
canonical text and textual review gates, and ADR-0051 keeps the kernel the
only judge.

## Decision

1. **AF1-X is an opt-in JSON authoring dialect.** A frame with `"afx": 1`
   may use nested operations, literal operands, checked propagation
   (`op?Case`), conditional exits (`["!Case", "if", c]`), `ok`/`fail`
   terminators, name-based qualification and derived trailing edge
   arguments, and `test_tables`. The workbench expands it, client-side and
   deterministically, into an ordinary AF1 frame that the unchanged AF1
   compiler compiles. Frames without `"afx"` compile exactly as before.
2. **Still data, not source syntax.** A nested operation is a JSON array
   over the closed opcode table. There is no text grammar, no tokenizer,
   no parser crate and no `.sley` file, and nothing is evaluated during
   expansion. The disposition of ADR-0051 item 4 against the anti-goal row
   "Sley source syntax or parser" extends to AF1-X unchanged.
3. **Expansion preserves meaning.** Nested operations evaluate left to
   right, depth first, at the position written. A failure exits at its
   written position. Unselected branches never execute: nothing is hoisted
   into a conditional path. Checked arithmetic stays checked. There is no
   reassociation, speculation, retry or duplicated effect. A failure route
   is used only when it is determined: a name that is both a block and an
   error case is refused, a payload moves only to a target of its exact
   type, and no payload or value is invented. Bounds on depth and size
   refuse rather than truncate.
4. **Source maps.** Every generated entity maps to the authored JSON
   pointer it came from. Refusals of the expanded frame are reported at the
   authored pointer, and function-wide kernel refusals say that they are
   function-wide instead of naming an invented location.
5. **Drafts are advisory local state.** Every `try` keeps a draft revision
   `(handle, revision, base head)`, including input that fails to parse,
   expand or validate. `fill` applies closed, atomic pointer replacements
   to a named revision and refuses a stale revision or a changed head
   unless a rebase is requested. A draft is never admission evidence. Only
   a complete, Valid candidate of the named revision can be submitted, and
   an older revision's candidate is never used for a newer one. Candidate
   handles keep their meaning.
6. **Tests.** Test tables lower into ordinary TestCases under the existing
   limits and grant ceilings. Tests are counted by provenance (provided,
   imported, authored). An imported case never counts as authored, and no
   expected value is taken from executing the candidate.
7. **Namespaces.** `"namespace": null` keeps newly created top-level
   entities out of every namespace. Omitting the key keeps the 2.0.2
   behaviour.
8. **Views stay output only.** `view --focus` and the AF1-X-shaped
   `view --x` are derived, non-canonical, and never parsed back.
9. **Local attribution counters.** Each command appends one line of counts
   (sizes, feature use, refusal symbols) to `.sley/events.jsonl`. The line
   holds no program content and no clock.
10. **Two transformations and a bounded search are enabled; the rest stay
    off.** `ripple` enables `arity` (a signature change carried to the
    calls and tests of the function by parameter name) and `guard` (a
    checker routed through the uses of one parameter, in a preserving or
    an entry mode). Each derives ordinary edits that the unchanged compiler
    and kernel judge; a decision a derivation cannot make exactly, an
    exported boundary, or a bound is an explicit obligation, never a guess.
    The intents `effect`, `member`, `retype`, `move` and `prune` are refused
    as not enabled. `search` proposes neighbours of one function from six
    typed generators, validates each through the kernel, evaluates only the
    public cases the author passes, and never submits; it is bounded per
    command and per accepted head. Neither is admission evidence
    (`SLEY_AGENT_V1.md` sections 5.3 and 13).

## Consequences

- An author states decisions: the operations, the error policy, the tests.
  The workbench derives the switches, blocks, parameters and constants.
- A refused frame is repaired by replacing the pointed-at subtrees. Tests
  written earlier survive layered edits.
- The kernel, candidate record, validation profile and VM are unchanged.
  The same expanded frame yields the same candidate bytes for the same
  head, nonce and allocation inputs.
