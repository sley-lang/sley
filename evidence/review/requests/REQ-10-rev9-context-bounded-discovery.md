# Review request REQ-10 rev9 — CONTEXT bounded discovery (rev8 inline remedies adopted)

- Supersedes rev8 (sha256
  `011535124cc30fd7d865eef718d6eda5fa5e15f96c928108d2d5b4c8cf8dcaa6`;
  verdict `REVISE_0_P0_0_P1_2_P2_2_P3_1_P4`, architecture settled,
  Fix 1 and the note template verified correct and complete,
  four items with inline remedies; transcript
  `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev8-95d42fda.md`,
  sha256 `e617e0c1db180d38b71df46d8b2965dfe50e373335c0e9`
  including the writing-lane packet-hash record).
- NUMBERING: this packet is review revision 9; the proposed
  trial-runner contract revision is 5.
- Baseline tree: `95d42fda691bc5d8ec468478529b7b174d3a8759` on
  `work/succession-sley20-arm` (records-only delta over `c1e8db56`;
  no Rust sources, fuzz targets, or lint inputs changed). Lane: Nabu
  (architecture). Section `context_bounded_discovery`, field
  `nabu_architecture_review`.
- Settled design unchanged. Adopted verbatim from the rev8 verdict:
  Fix 1 (hoist after `:178`, converted, anchored) and the `on
  <40-hex>` note template need nothing further; Fix 3's revision-5
  binding trade is ratified as stated. What follows closes the four
  remaining items exactly as the verdict remedies them.

## Item 1 (rev8 P2-1) — pin test required; false premise withdrawn

Withdrawn in the open: rev8's "a pin test would invent a new
cross-package test dependency" is false. `bench/live/sley2_tool.py:51`
carries a module-level `from bench.sley2.runner import
(PROFILE_ARGS, Endpoint, endpoint_offer, probe_handshake,
request_frame)` (verified this round), and every live test touching
the tool pulls it transitively at import time
(`test_sley2_tool.py:17`, `test_agent_access.py`, `test_acceptance_repairs`
top-level `from bench.live import ... sley2_tool`). The dependency
is load-bearing and unavoidable; the test's true cost is one import
line plus one assertion, not a new dependency. The verified half of
the rationale stands (no live test names `bench.sley2`; the checker
never reads `bench/live` — re-verified: zero occurrences of
`sley2_tool`/`bench/live` in `check_sley2_trial_runner.py` — so the
runner-side count/spec-order/digest gates pin one side only, which
is the problem, not the defence).

Required edit (same implementation commit as edit 4.0): in
`bench/live/tests/test_sley2_tool.py` (already imports
`sley2_tool`), add `from bench.sley2 import runner` and
`test_tool_methods_equal_runner_allowlist_in_order` asserting
`tuple(sley2_tool.TOOL_METHODS) == tuple(runner.ARM_AFFORDANCES)`.
After edit 4.0 both tuples read nineteen names; this assertion is
what makes them the same nineteen in the same order, converting two
of the four coupled enumerations
(`ARM_AFFORDANCES`/`TOOL_METHODS`) from editorial to mechanical.
The honest prose (strike of the false "pinned ... by test" clause
at `sley2_tool.py:10,84`) is kept alongside — prose made true plus
the assertion that makes it true.

## Item 2 (rev8 P2-2) — edit 4.1: `open` joins the mediated surface

Edit 4.1 (same commit): add `"open"` to `ALLOWED_COMMANDS`
(`bench/live/mediated_sley.py:48-51`). Verified sufficient with no
new forward case: `GATEWAY_COMMANDS = ALLOWED_COMMANDS |
{"resolve"}` (`:62`) inherits it; `_run_command` forwards every
allowed command to `sley2_tool.dispatch` (verified); the `:47`
comment ("same commands as sley2_tool.dispatch") stays true. The
mediated lane — the captured, measured route — therefore affords
the dedicated discovery command rather than leaving it reachable
only via the `raw` escape hatch (`:723`/`:220` gate on the widened
`TOOL_METHODS`). No silence, no deliberate-exclusion alternative:
the dedicated command is the surface Fix 2 exists to unify.

## Item 3 (rev8 P3-2) — capture-vocabulary sentence folded into Fix 2

Folded into the Fix 2 edit-site list, stated plainly: admitting
`workspace.open` to `TOOL_METHODS` flips its capture label from
`raw:denied` to `raw:workspace.open` under
`bench/live/mediated_sley.py:220-223` (comment `:217-219`),
correct by design (labelling an allowed method denied would be the
bug), the second vocabulary change absorbed under that comment, in
the evidence stream the judge audits bounded-query discipline from.
No gate moves; no PASS/FAIL affected; named so the record, not just
the code, knows the vocabulary moved.

## Item 4 (rev8 assumption 4) — load-bearing checker/spec lines confirmed in-packet

Confirmed here, not assumed: the implementation delta reaches the
three mechanical lines the `:286-289` extension and the drift gate
depend on — checker `:135` (`_spec_allowlist` anchor literal
`holds eighteen names` → nineteen), checker `:257`
(`len(allowlist) != 18` → `!= 19`), and the spec §9 frozen-order
text plus order list (`workspace.open` last, eighteen→nineteen) the
anchor parses. These were in the rev7 C1 active list all along;
this item makes the dependency explicit so it cannot surface as a
P0 at implementation. If any of the three is missed, the checker
fails closed (`spec-allowlist:missing` / `allowlist-count:19`
against an eighteen anchor) — the control working, not a defect.

## Item 5 (rev8 P3-1) — record-state statement

At dispatch of this packet, the rev6/rev7 packets and transcripts
are untracked worktree files, not yet in any reachable commit —
stated here explicitly (the second option the P3-1 remedy allows),
not cited as though resolvable: intended paths
`evidence/review/requests/REQ-10-rev{6,7}-context-bounded-discovery.md`
and
`evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev{6,7}-95d42fda.md`
(packet shas `2ba40e08…`, `24d2b505…`; transcript shas with
writing-lane hash records `4f8de2f0…`, `590df37e…`). They land in
the round commit carrying this round, in or before it — so the next
packet's SUPERSEDES names committed paths. This packet's own
SUPERSEDES above names the rev8 transcript path likewise
not-yet-committed at dispatch, committed-here at landing; the
writing lane records this packet's sha256 in the transcript file on
landing (PACKET_SHA256 convention), never fabricated in-session.

## Acceptance (unchanged shape, revision-bound)

`workspace.open` (allowed, 19-name surface; agent route via
`TOOL_METHODS` + `open` command + pin test; mediated route via edit
4.1) → accepted-head snapshot binding (field 9, head-pinned opener
path only) → class-14 discovery with actual continuation where
needed → agent mutation → production admission → real oracle →
append → verify; rejection for incomplete discovery, inconsistent
continuation, exceeded budgets, missing evidence; no private IDs as
witness arguments, no hidden manifest, no inventory dump. Status
returns to `COMPLETE` only with three revision-5-bound PASS fields.
Follow-on lanes remain `vulcan_surface_review` and
`ariadne_contract_review`, deferred to the implementation they must
evaluate.

- Constraints: read-only review. No file writes, no test execution in
  the reviewer session. A returned REVISE remains REVISE whether the
  issue is governance or code; no redesign of settled architecture is
  requested, and a genuine new finding must not be suppressed.
  Verdict in reply text only, in the REQ verdict format, against
  section `context_bounded_discovery`, field
  `nabu_architecture_review`.
