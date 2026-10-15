# ADR-0055: optional, noncanonical authoring frontends (familiar text proposals)

Status: accepted 2026-10-02 by operator decision FAMILIAR-FRONTEND-2026-10-02
(`evidence/release/operator-decision-FAMILIAR-FRONTEND-2026-10-02.md`).
Implementation under this ADR is the governance amendment only. The frontend
itself, its contract and its tests are a separate change
(`docs/spec/SLEY_AGENT_V1.md` section 5.5).

Date: 2026-10-02

MPI-0 disposition: `YES_MACHINE_BENEFICIAL` (machine generation efficiency
of authoring proposals, with correctness preserved by the unchanged kernel
path). Not `NO_HUMAN_ONLY`: the justification is measured machine utility,
not human familiarity, readability or manual editing.

## Context

ADR-0054 added structured function bodies (JSON statements lowered to
ordinary AF1-X blocks) and proposed, as its decision 4, a write-only text
syntax that parses into exactly the same body tree. That proposal conflicts
with these active texts:

- `docs/ANTI_GOALS.md`, row "Sley source syntax or parser": "no production
  parser crate/grammar/`.sley`".
- `CONTRIBUTING.md`, "Prohibited additions" (carrier C-01): "Do not add Sley
  source syntax, parser, ... to the 2.0 GA path."
- ADR-0051 decision 3, "Authoring input is JSON only", and decision 4, whose
  disposition of AF1 against the anti-goal row rests on "no parser crate,
  grammar or `.sley` file exists".
- The machine-primacy gate of the BLACKGLASS constitution (MPI-0, carried by
  C-01 and C-02), item "no Sley source parser exists in the GA dependency
  graph", and its list of things that patch does not authorize ("source
  syntax", "parser").

BLACKGLASS also says: "Familiarity may be used only when it independently
improves machine performance or implementation correctness and that benefit
is evidenced." Controlled authoring measurements held by the maintainers
showed that agents produce the same structured bodies with less output when
they write them in a familiar statement syntax. That is the evidenced machine
benefit this ADR relies on. It is not a claim about any release, and no
benchmark result is stated here.

Renaming a parser an "adapter" would not change what it is. This ADR
therefore amends the restriction explicitly instead of reading around it.

## Decision

1. **Approved policy (verbatim).** "Optional, noncanonical authoring
   frontends may accept textual proposals when justified by machine utility.
   They must lower deterministically through the ordinary typed candidate and
   validation path. Their syntax, source files, formatting, and parser must
   not become canonical program authority or a prerequisite for the
   graph-native lifecycle."

2. **What changes, exactly.**
   - C-02, `docs/ANTI_GOALS.md`: the row "Sley source syntax or parser"
     becomes "Sley source syntax or parser outside the optional authoring
     frontend". Its enforcement surface adds the frontend confinement check;
     its acceptance evidence keeps every previous clause (no production parser
     crate, grammar or `.sley` file; inert benchmark fixtures stay out of the
     product graph; source-like protocol input rejected) and adds the
     confinement conditions of decision 3.
   - C-01, `CONTRIBUTING.md`: a paragraph after the SH2 paragraph says that an
     optional, noncanonical authoring frontend adopted under this ADR is
     governed by this ADR instead of the words "Sley source syntax" and
     "parser" in the prohibition. The prohibition paragraph itself is
     unchanged.
   - ADR-0051 decisions 3 and 4 are narrowed, not withdrawn: authoring input
     to the kernel path is still a JSON frame, and AF1 is still data, not
     source syntax. The workbench may additionally accept a textual proposal,
     under an explicit per-command switch, that it turns into such a JSON
     frame before anything else happens.
   - ADR-0054 decision 4 is adopted under the constraints of decision 3.
   - For this repository, the BLACKGLASS gate item "no Sley source parser
     exists in the GA dependency graph" reads: no parser of program text in
     the canonical or kernel dependency graph (codec, store, verifier,
     transaction engine, VM, protocol server, CLI transport), and none on any
     path that querying, editing, validating, executing, exporting or
     reconstructing a committed program needs; the one exception is the
     optional frontend of decision 3. The BLACKGLASS list of things its own
     patch did not authorize is not a standing prohibition and is unaffected.

3. **Confinement conditions** (mechanically checked where possible by
   `scripts/build_anti_goal_conformance.py`):
   - The parser is one module of the `sley-agent` crate behind its optional
     cargo feature `familiar`, which enables no dependency. Building with
     `--no-default-features` removes it; nothing else changes.
   - The module imports only `std` and `serde_json`: it produces a structured
     JSON frame and positions, and cannot reach the kernel, candidate, store
     or commit APIs.
   - No other crate depends on `sley-agent` or names the module. No parser
     crate enters the lock.
   - It is selected explicitly per proposal (`try --familiar`); nothing
     detects or prefers text, and the default authoring paths are unchanged.
   - Its output takes exactly the path of the equivalent JSON frame: the
     shared structured lowering, the AF1-X expander, the frame compiler and
     the kernel's candidate validation. It has no semantics of its own;
     arithmetic, conversion, scoping and failure are those of the structured
     body form.
   - The text is a proposal. It is kept with the draft revision as its input
     record, never as program state. No canonical record, hash or identity
     depends on it; text positions are diagnostics only. Nothing renders a
     program back into the syntax, so it is neither a projection nor a review
     surface.
   - Unsupported or ambiguous text is refused at its line and column, never
     guessed or repaired.

4. **What is unchanged.** SSMC1, SCB1 and SMP1; kernel authority (the
   kernel alone decides validity); capability tokens and policy roots;
   stale-state checks (base head and draft revision checks); admission; and
   atomic commit semantics. Every other anti-goal row, including "canonical
   text or human projection", "formatter, REPL, Tree-sitter, conventional
   LSP", "Sley 1.x compatibility", "source/file/line/comment identity",
   "human readability optimization", "hidden exceptions/null/implicit numeric
   conversion" and "source-based merge". The CLI transport of ADR-0035 is
   unchanged. The syntax is not Sley 1.x syntax and gives no compatibility or
   import path.

5. **What this ADR does not do.** It does not make the frontend a default
   or a required step, does not add a formatter, a pretty-printer, a
   projection from programs to text, an editor integration or a second
   semantic engine, does not change the frozen instruction set or any
   canonical format, and does not authorize publication, release, tagging or
   a GA claim. It authorizes this one boundary adjustment and no other
   policy change.

## Consequences

- The anti-goal checker gains an additive confinement check. The `.sley`
  inventory and the parser-crate denylist still fail unconditionally.
- Removing the frontend (building without the feature, or deleting the
  module) leaves every committed program and every JSON authoring path
  working; the frontend's tests include a build without it.
- A later frontend of the same kind needs its own ADR under this policy.
