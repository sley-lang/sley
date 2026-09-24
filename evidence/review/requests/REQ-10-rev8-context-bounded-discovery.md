# Review request REQ-10 rev8 — CONTEXT bounded discovery record completion (rev7 transcription fixes)

- Supersedes rev7 (sha256
  `24d2b5056f14afe619737306a592c47071384ce11540439edddf47574f0bfe08`;
  verdict `REVISE_0_P0_1_P1_2_P2_1_P3_0_P4`, architecture settled,
  Fixes 3 and 5 correct and complete, Fix 2 convention right,
  four mechanical transcription faults with inline remedies;
  transcript
  `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev7-95d42fda.md`,
  sha256 `590df37ed6495dc7c4e3dfa9933f5ac0d509987e18ca1bc626a1c8cef062e243`
  including the writing-lane packet-hash record).
- NUMBERING: this packet is review revision 8; the proposed
  trial-runner contract revision is 5.
- Baseline tree: `95d42fda691bc5d8ec468478529b7b174d3a8759` on
  `work/succession-sley20-arm` (records-only delta over `c1e8db56`;
  no Rust sources, fuzz targets, or lint inputs changed). Lane: Nabu
  (architecture). Section `context_bounded_discovery`, field
  `nabu_architecture_review`.
- Settled design unchanged. Fixes 3 (inventory floor + token set +
  status_note) and 5 (namespace statement) are confirmed correct and
  complete and are carried forward verbatim, not restated. What
  follows adopts the four inline remedies exactly, each at the edit
  site the rev7 verdict names.

## Fix 1 (rev7 P1) — hoist target corrected to after `:178`

Rev7's "~`:155`" target is withdrawn, with the sentence that
claimed "no gate references a variable defined below it" (falsified
by the P1: the drift assertion read `section`, first bound at
`:174`/`:177`). Adopted remedy, exact: the single extraction,
int conversion, and drift assertion land after
`status = section.get("status")` (`:178`) and before the `if status
in IMPLEMENTATION_STATUSES:` block (`:201`), indent 4, outside the
block — satisfying every property argued (single extraction,
indent 4, above the block, feeding `:286-289` and the `:291`
payload) without the ordering fault:

`spec_revision_match = re.search(r"^Status: S20-620 contract draft, revision (\d+)", spec, flags=re.M)`
`spec_revision = int(spec_revision_match.group(1)) if spec_revision_match else None`
`summary_revision = section.get("contract_revision")`
`if summary_revision != spec_revision: problems.append(f"machine-summary:contract_revision:{summary_revision!r}!=spec:{spec_revision!r}")`

`re` is already imported (`:8`); no new import. The precedent's
ordering (section binding, then extraction, then comparison — cf.
`check_root_backed_query_profile.py:209/214/218`) is now followed,
not just its comparison. The `:291` payload reads the hoisted
`spec_revision`; the A4 demonstration asserts both directions
(fires 4-vs-5, silent 5-vs-5) from scratch copies outside the repo,
outputs pasted into the implementation gate record.

## Fix 2 (rev7 P2-1) — `TOOL_METHODS` edit plus folded docstring correction

Edit 4.0 (omitted from rev7 Fix 4, added here): `workspace.open`
joins `TOOL_METHODS` (`bench/live/sley2_tool.py:88`) in the same
tuple order as `ARM_AFFORDANCES` after the move (appended after
`session.capabilities`, 19 names). Without it the `:322` guard —
`if method not in TOOL_METHODS: _fail("method")`, correctly
identified by rev7 as applying to the new `open` command via
`_view` (`:390`) → `session.call` (`:395`) → `_request`
(`:321-324`) — rejects every agent `open` with `_fail("method")`,
shipping a dead command. The harness path (`:261` via
`_raw_request`, bypassing the guard) never needed the entry; the
agent path does. The packet's acceptance clause already scopes
`workspace.open` as allowed on the 19-name surface, so this is the
enumerated completion of stated intent, not new scope.

Folded docstring correction (rev7 D1, carried P4): `:10` and `:84`
read "nineteen-method" in the same commit, since edit 4.0 is what
makes them stale.

B4 disposition, decided (reviewer judges the choice): STRIKE, not a
new pin test. The false clause "pinned equal to the smoke runner's
list by test" is replaced with a true statement (nineteen-method
surface; order matches the smoke runner's allowlist, verified at
each admission review — no test asserts it). Reason: the runner
side stays pinned by the checker count/spec-order/digest gates; no
live test imports `bench.sley2.runner` today (verified: live tests
import `bench.live.sley2_tool` only), so a pin test would invent a
new cross-package test dependency for a 19-name literal the checker
does not read. The claim stops being false; nothing new is built to
replace it.

## Fix 3 (rev7 P2-2) — binding starts at revision 5; no invented rev4 transcript

Ground truth adopted: no `evidence/review/verdicts/sley2_trial_runner/`
directory exists (verified absent this round), and the three lanes'
rev4 `PASS_0_P0_0_P1_0_P2_0_P3` values have no transcript behind
them — those verdicts predate transcript capture. No
`<base>_revision_4` / `_revision_4_note` fields are written; an
implementer is explicitly not asked to satisfy a template naming a
nonexistent path. Rev4 approvals survive as valid historical
evidence in the byte-exact base values, in git history, and in this
REQ chain — not re-labeled, not erased.

Binding starts at revision 5, where transcripts exist: the
follow-on lanes write `<base>_revision_5` values plus
`<base>_revision_5_note` (shape per Fix 4); the `:286-289` gate
requires `section.get(f"{key}_revision_{spec_revision}")` to start
with `PASS` (new code `completion-unbound-review:{key}`) alongside
the existing base-field check; base fields update to the revision-5
values only in the commit that returns status to `COMPLETE`.

## Fix 4 (rev7 P3) — note template in the form the register parses

The `_note` template uses `on <40-hex>`, not the quoted `at <sha>`:
`NOTE_SCOPE = re.compile(r"\bon ([0-9a-f]{40})\b")`
(`scripts/build_finding_register.py:540`; scope derivation at
`:604-607`). The rbqp-quoted form (`at` + short or 40-hex sha)
yields `scope = None`; adopting "the tree's existing convention"
means the form the consuming code parses. Template:
`Historical <lane> verdict on <40-hex commit>; transcript <path>.`
This affects register scope attribution only — no gate PASS/FAIL,
GA predicate unaffected (stated, not elided).

## Acceptance (unchanged shape, revision-bound per Fixes 1-3)

`workspace.open` (allowed, 19-name surface, agent route via
`TOOL_METHODS` + `open` command per Fix 2) → accepted-head snapshot
binding (field 9, head-pinned opener path only) → class-14 discovery
with actual continuation where needed → agent mutation → production
admission → real oracle → append → verify; rejection for incomplete
discovery, inconsistent continuation, exceeded budgets, missing
evidence; no private IDs as witness arguments, no hidden manifest,
no inventory dump. Status returns to `COMPLETE` only with three
revision-5-bound PASS fields. Follow-on lanes remain
`vulcan_surface_review` and `ariadne_contract_review`, deferred to
the implementation they must evaluate.

- Constraints: read-only review. No file writes, no test execution in
  the reviewer session. A returned REVISE remains REVISE whether the
  issue is governance or code; no redesign of settled architecture is
  requested, and a genuine new finding must not be suppressed.
  Verdict in reply text only, in the REQ verdict format, against
  section `context_bounded_discovery`, field
  `nabu_architecture_review`.
