# Review request REQ-10 rev6 — CONTEXT bounded discovery record completion (rev5 remedies)

- Supersedes rev5 (sha256
  `ace99e9103bc3cc64a2f0889152ff324759d40639c480790aaf22dd4d4a7fbc3`;
  verdict `REVISE_0_P0_1_P1_2_P2_1_P3_1_P4`, architecture settled,
  three remedies specified; transcript
  `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev5-c1e8db56.md`).
  Rev5's verified answers (move-not-copy, encoder scoping, Merlin
  ownership, denial rationale, precedent exactness) stand as reviewed
  and are carried forward without redesign.
- NUMBERING: this packet is review revision 6. The trial-runner
  contract revision it proposes is 5. The two numbers are disjoint
  namespaces; wherever they co-occur below, "rev6" means this packet
  and "revision 5" means the contract.
- Baseline tree: `95d42fda691bc5d8ec468478529b7b174d3a8759` on
  `work/succession-sley20-arm`. Sources identical to `c1e8db56` (rev5
  baseline): the sole delta is the records-only rev5 round (3 added
  files: rev5 packet, rev5 verbatim verdict, rev5 gate record); no
  Rust sources, fuzz targets, or lint inputs changed. Lane: Nabu
  (architecture). Section `context_bounded_discovery`, field
  `nabu_architecture_review`.

## Settled architecture carried forward (no redesign requested)

- `workspace.open` moves from denied to afforded (move, not copy;
  18 → 19 names, tuple order, disjointness and SMP1-coverage
  invariants explicitly tested).
- Field 9 (accepted-head snapshot identity) only on the head-pinned
  `workspace.open` response path via an explicit head-binding
  parameter or a split encoder; `revision_summary`
  (`crates/sley-protocol/src/server.rs:3244-3259`, shared by exactly
  two callers `:2502` and `:2891`) is not given head knowledge, and
  `revision_read` response bytes stay byte-identical.
- Omission-not-refusal disclosure shape; Merlin-owned cached-snapshot
  probe (`docs/WORK_PACKAGES.md:35`; `accept_cached` private at
  `crates/sley-repo/src/index_cache.rs:108`, `read_record` private at
  `:242`, `fresh_snapshot` `pub(crate)` at `:131`,
  `complete_root_snapshot` public at `:203` falling through, verified
  `verify_cached_snapshot` anchor in remedy D); no hidden
  fresh-snapshot rebuild.
- Task-input amendment and bounded-discovery acceptance criteria from
  the settled packet (positive plus incomplete-discovery,
  inconsistent/stale-continuation, budget-breach, and
  missing-evidence negatives).

## Remedy A — revision and review binding (concrete delta)

A1. Same-commit atomic update: spec preamble
(`docs/spec/SLEY2_TRIAL_RUNNER_V1.md:3`, `revision 4` → `revision 5`,
recording this admission alongside the revision-4 entity-reads line);
machine-summary `sley2_trial_runner.contract_revision` 4 → 5;
`status` `S20_620_COMPLETE` → `S20_620_IMPLEMENTED_REVIEW_PENDING`
(the existing `REVIEW_PENDING_STATUS` constant at
`scripts/check_sley2_trial_runner.py:27`); `implementation_complete`
`true` → `false` (the checker's `expected` dict at `:179-189`
derives it from `status == COMPLETE_STATUS`, so the hand-edit must
match or the checker fails — that coupling is the control working).
Rev4's three `PASS_0_P0_0_P1_0_P2_0_P3` field values stay byte-exact
as historical evidence; they are not erased, not re-labeled, and no
new status or review framework is introduced.

A2. Drift assertion (two lines, `:291` region, on the
`check_root_backed_query_profile.py:226-227` pattern verbatim):
after `revision = re.search(r"revision (\d+)", spec)` insert
`summary_revision = section.get("contract_revision")` and
`if summary_revision != revision: problems.append(f"machine-summary:contract_revision:{summary_revision!r}!=spec:{revision!r}")`.
This closes the drift class permanently (any future bump must move
spec, summary, and checker together or `make lint` fails), not for
this bump only. No `CONTRACT_REVISION` constant is added: the
summary-vs-spec comparison is self-consistent without one, and the
s20-310 pinned constant is a different checker's own pin, not this
packet's ask.

A3. Stale-verdict completion binding (existing gate extended, no new
framework): extend the `:286-289` loop so that when
`status == COMPLETE_STATUS` each of the three review fields must both
start with `PASS` (existing) and carry the current-revision binding
matching the spec revision extracted at `:291` (new problem code
`completion-stale-review:{key}`). While status is off COMPLETE the
loop does not fire, so rev4-bound values rest undisturbed as
history. The follow-on lanes this plan already requires
(`vulcan_surface_review`, `ariadne_contract_review` at revision 5)
write revision-qualified values (e.g. `PASS_r5_0_P0_...`); Nabu's own
implementation acceptance updates `nabu_architecture_review`
likewise; status returns to `COMPLETE` only when all three fields
carry revision-5-bound PASS. Architecture clearance is explicitly
not implementation acceptance: nothing here promotes a field to PASS.

A4. Regressions specified (permanent, no new test files — the repo
has no checker-unit-test precedent; the checker is the control,
exercised on the real tree by every `make lint`): the two new
problem codes above ARE the regressions (both fail closed on future
drift). Implementation demonstrates each fires by running the
checker against scratch copies outside the repo (`/tmp`, never the
worktree): (a) summary `contract_revision` 4 against spec revision 5
must emit `machine-summary:contract_revision:4!=spec:5`; (b) status
`COMPLETE` with rev4-bound fields against a revision-5 spec must
emit `completion-stale-review:{key}` for all three keys. Both
outputs are pasted into the implementation gate record.

## Remedy B — harness/agent split (explicit, both jobs named)

B1. The harness retains `_read_head`
(`bench/live/sley2_tool.py:256-268`), which calls `_raw_request`
directly (`:261`), bypassing the `:322` dispatcher guard by original
construction. It populates `self._head` (tx, root, policy,
workspace, epoch; `:213`, `:268`) for exactly these six
harness-internal consumers, none deleted, none re-routed: the `head`
property (`:381-382`); epoch object decode (`:424`); epoch inventory
(`:482`); the rejected-append head-movement invariant (`:751`,
`:754-755`); the entity request-body root (`:876`); and the mediated
workspace id (`bench/live/mediated_sley.py:137`). No additional
protected state is exposed under the label "removing the relay".

B2. The agent gains `workspace.open` as its own afforded route:
`TOOL_METHODS` (`:88-107`) grows to 19 names in the same tuple order
as `ARM_AFFORDANCES` after the move, so the existing `:322` guard
and the `raw` command path (`:723`, `rest[0] in TOOL_METHODS`) admit
it, and every agent call is captured by `_record` like any other
agent-visible context. An agent-visible `open` command is added
alongside `revision` (same `_view` shape, routed through `_request`
so guard plus transcript apply).

B3. The `revision` command path (`:717-718`) no longer relies on a
harness-pre-supplied transaction identity: it takes the tx returned
by the agent's own prior `open` call (agent-held discovery result,
recorded in the transcript) and fails closed when no
agent-performed open precedes it. `session.head["tx"]` at `:718`
therefore denotes agent-discovered context, while the six B1
consumers keep reading the harness's own `_head` — harness
bookkeeping stays distinguishable from agent-visible context in the
record, and the omission/truncation accounting (`:315-320`) is
unchanged.

B4. Pin claim disposition (reviewer confirms which): either author
the pinning test (`TOOL_METHODS == ARM_AFFORDANCES` in tuple order)
or strike the claim at `sley2_tool.py:10,84` (`pinned equal to the
smoke runner's list by test`; `grep TOOL_METHODS --
bench/live/tests/` returns nothing — documentation only, verified).

## Remedy C — honest change inventory (a floor, with verification)

C1. Active references the 18→19 move touches (must change in the
implementation commit): `bench/sley2/runner.py` — the denied tuple
(`workspace.open` last entry), the affordances tuple (append in
tuple order after `session.capabilities`), and the five
eighteen-name phrasings (`:148` profile comment, `:446` offer
docstring, `:931` snapshot binding, `:936` legacy stamp; plus
`:115-117` denial-rationale rewrite and the `ArmAffordanceTests`
docstring at `bench/sley2/tests/test_runner.py:541-546`);
`bench/live/sley2_tool.py` — `:10` and `:84` (nineteen + B4
disposition), `:88-107` (`TOOL_METHODS`), `:256-268` plus the new
`open` command and the `:717-718` path change; spec `:3` (preamble),
`:35` (§1), `:291-297` (§9 list, `workspace.open` last,
eighteen→nineteen); `scripts/check_sley2_trial_runner.py` — `:134`
docstring, `:236` delta-pin comment (revision 4→5 wording),
`:257` count, `:135` anchor, `:286-289` gate extension, `:291`
drift assertion; machine-summary `sley2_trial_runner`
(`contract_revision`, `status`, `implementation_complete`,
`status_note` — the note describes current status, so it moves with
it); closeout `:5` (present-tense revision-4 attestation goes stale
in the same way the summary field does); `test_runner.py:335`
(legacy-stamp docstring).

C2. Frozen history (left byte-exact, no blanket replacement):
closeout lines under the "Revision 4 attestation" heading
(`:124/:140/:156` per the rev5 verdict) and its retained revision-2
record; `docs/audits/PHASE3_V2_OFFER_DESIGN.md:163` (revision-4
design record); machine-summary `p0_closed`/`p0_open` history; the
rev4 review field values themselves (A1).

C3. Omissions rev5 missed (now included above): `test_runner.py:335`,
checker `:134`/`:236`, closeout `:5` and `status_note`.

C4. Floor statement: this inventory is a minimum set, not an
exhaustive one. Implementation must `grep -rn eighteen` (and
`eighteen-method`) across the repo, disposition every hit as
moved / frozen-history / updated, and record the disposition table
in the implementation gate record; repository-wide verification, not
packet exhaustiveness, is what closes C.

## Remedy D — citation and record correction

D1. Citation fix: the eighteen-method claim is at
`bench/live/sley2_tool.py:10`, not `:8` (`:8` is "No state survives
across invocations; the harness records every result."). `:84`
stands as cited. Rev4 cited `:10,84` correctly; rev5's `:8` was a
new off-by-two, corrected here.

D2. Authoritative rev5 verdict (preserved exactly):
`REVISE_0_P0_1_P1_2_P2_1_P3_1_P4` — one P1 (governance, not design),
two P2, one P3, one P4. The `95d42fda` commit subject abbreviates
this as `0P1/2P2/1P3`: a subject-line error in the forward record.
Git history and the verbatim transcript are untouched and
authoritative; this packet corrects the forward record here, in the
open, rather than anywhere silent.

D3. Anchor note (substance unchanged): the rev5 verdict cites
`verify_cached_snapshot` at `index_cache.rs:288`; the
implementation-verified anchor is `:278` (`pub fn
verify_cached_snapshot`, fall-through body following; `accept_cached`
`:108`, `fresh_snapshot` `:131`, `complete_root_snapshot` `:203`,
`read_record` `:242` all as cited). Implementation uses the verified
anchors; the Merlin-ownership decision stands exactly as reviewed.

## Acceptance (unchanged shape, revision-bound)

`workspace.open` (allowed, 19-name surface) → accepted-head snapshot
binding (field 9, head-pinned opener path only) → class-14 discovery
with actual continuation where needed → agent mutation → production
admission → real oracle → append → verify; rejection for incomplete
discovery, inconsistent continuation, exceeded budgets, missing
evidence; no private IDs as witness arguments, no hidden manifest,
no inventory dump. Completion returns to `COMPLETE` only with three
revision-5-bound PASS fields (A3). Follow-on lanes remain
`vulcan_surface_review` and `ariadne_contract_review`, deferred to
the implementation they must evaluate.

- Constraints: read-only review. No file writes, no test execution in
  the reviewer session. A returned REVISE remains REVISE whether the
  issue is governance or code; no redesign of settled architecture is
  requested, and a genuine new finding must not be suppressed.
  Verdict in reply text only, in the REQ verdict format, against
  section `context_bounded_discovery`, field
  `nabu_architecture_review`.
