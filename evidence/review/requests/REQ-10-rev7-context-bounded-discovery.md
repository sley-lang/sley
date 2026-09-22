# Review request REQ-10 rev7 — CONTEXT bounded discovery record completion (rev6 mechanical fixes)

- Supersedes rev6 (sha256
  `2ba40e089e976af6c4772b53761a2b89c6c919a8ac72a9c69bc8df2718559047`,
  225 lines; verdict `REVISE_0_P0_1_P1_4_P2_2_P3_1_P4`, architecture
  still settled, five mechanical fixes specified; transcript
  `evidence/review/verdicts/context_bounded_discovery/nabu_architecture_review-rev6-95d42fda.md`,
  sha256 `4f8de2f02b500b36521e3f8c9aa3bc776f09f1532a89c68325648abe19133928`
  including the writing-lane PACKET_SHA256 record the reviewer
  required). Rev6's intent-level answers stand as reviewed; what
  follows fixes the transcription, ordering, convention, coverage,
  and mechanism defects the rev6 verdict demonstrated, each with the
  exact lines the fix touches.
- NUMBERING: this packet is review revision 7. The trial-runner
  contract revision it proposes is 5. "Rev7" means this packet;
  "revision 5" means the contract.
- Baseline tree: `95d42fda691bc5d8ec468478529b7b174d3a8759` on
  `work/succession-sley20-arm`. Sources identical to `c1e8db56`:
  the delta since is records only (rev5 round, rev5 gate record,
  rev6 packet, rev6 transcript); no Rust sources, fuzz targets, or
  lint inputs changed. Lane: Nabu (architecture). Section
  `context_bounded_discovery`, field `nabu_architecture_review`.
- Settled design carried forward unchanged (move-not-copy,
  encoder scoping, Merlin probe, denial rationale, precedent,
  task-input amendment, acceptance shape with negatives). Nothing
  below redesigns any of it.

## Fix 1 (rev6 P1) — hoisted, converted, anchored revision extraction feeding both gates

Rev6's A2 failed because it copied the precedent's comparison
without the precedent's conversion, at the wrong indent level, with
the existing unanchored regex. The single correct shape, replacing
rev6 A2/A4 in full:

1. Immediately after `spec = read(SPEC)` in
   `scripts/check_sley2_trial_runner.py` (~`:155`, indent 4, above
   and outside the `if status in IMPLEMENTATION_STATUSES:` block
   that opens at `:201`), insert:
   `spec_revision_match = re.search(r"^Status: S20-620 contract draft, revision (\d+)", spec, flags=re.M)`
   `spec_revision = int(spec_revision_match.group(1)) if spec_revision_match else None`
   The anchor matches the precedent's form (`^Status: ... revision
   (\d+)` with `re.M`, cf.
   `check_root_backed_query_profile.py:214`) and binds the Status
   line (`SLEY2_TRIAL_RUNNER_V1.md:3`) instead of the first
   lowercase "revision" anywhere in a preamble that already carries
   "revision 1", "Revision 3", "Revision 4" in prose (this also
   closes rev6 P3-1).
2. The drift assertion (same indent-4 region, directly after the
   extraction): `summary_revision =
   section.get("contract_revision")` and `if summary_revision !=
   spec_revision: problems.append(f"machine-summary:contract_revision:{summary_revision!r}!=spec:{spec_revision!r}")`.
   `int`-to-`int`: fires at 4-vs-5, silent at 5-vs-5 (this closes
   the P1 transcription defect and the A4(a) wrong-reason pass).
3. The `:291` result payload reads the hoisted `spec_revision`
   (`"revision": spec_revision`) instead of running its own bare
   search; no second extraction exists anywhere.
4. Regressions (permanent, checker-as-control, exercised by every
   `make lint`; no new test files — the repo has no checker-unit
   precedent): the drift problem code plus the binding code in Fix
   2. Implementation demonstrates both directions from scratch
   copies outside the repo: drift case must emit
   `machine-summary:contract_revision:4!=spec:5`; aligned case must
   emit neither code. Both outputs pasted into the implementation
   gate record (this replaces rev6 A4: direction-asserted, not
   fire-only).

## Fix 2 (rev6 P2-1, P2-2) — ordering plus the tree's own field-name binding convention

Ordering (P2-1): Fix 1's hoist is the whole fix. The `:286-289`
completion loop (indent 8, inside the `IMPLEMENTATION_STATUSES`
block) and the result payload both read the one hoisted
`spec_revision`; no gate references a variable defined below it.
Stated explicitly so the two inserts compose: extraction once at
~`:155`, consumers at `:286-289` and `:291`.

Binding convention (P2-2): rev6's value-encoded `PASS_r5_...` is
withdrawn. Adopted instead is the tree's existing field-name
convention, verified this round: 129 `<lane>..._review_revision_N`
fields in `machine-summary.json` (e.g. `root_backed_query_profile`
carries `vulcan_surface_review_revision_5/6`,
`ariadne_contract_review_revision_5`,
`nabu_architecture_review_revision_5`, each with a `_note` naming
transcript and commit); `build_finding_register.py`
`is_lane_field_name` / `verdict_field_names` derive that shape from
the summary itself; the GA predicate
(`build_ga_acceptance_report.py:81`,
`^PASS_0_P0_0_P1_0_P2_0_P3(?:_0_P4)?$`) keeps matching only the bare
and zero-count forms, so base-field values stay in grammar and the
GA/dossier rows the rev6 verdict traced are undisturbed
(predicate verified at `:81`; downstream row ranges carried from
the rev6 verdict, not re-read this round). Concrete delta:

1. At the implementation commit (status `REVIEW_PENDING`, gate
   loop dormant): base fields
   (`sley2_trial_runner.ariadne_contract_review`,
   `.nabu_architecture_review`, `.vulcan_surface_review`) keep
   their rev4 `PASS_0_P0_0_P1_0_P2_0_P3` values byte-exact; add
   `<base>_revision_4` with the same values plus `<base>_revision_4_note`
   naming the rev4 transcript and commit, on the rbqp `_note` shape
   (`Historical ... verdict at <sha>; ... Transcript: <path>`).
2. Extend the `:286-289` loop: when `status == COMPLETE_STATUS`,
   each base key must both start with `PASS` (existing) and have
   `section.get(f"{key}_revision_{spec_revision}")` starting with
   `PASS` (new problem code `completion-unbound-review:{key}`).
3. The follow-on lanes this plan already requires write
   `..._revision_5` values plus notes; the final commit updates the
   three base fields to the revision-5 values and returns status to
   `COMPLETE`. No new status, no new framework, no value-encoded
   token the register classifier has no rule for.

## Fix 3 (rev6 P2-3) — widened floor: WORK_PACKAGES row, token set, status_note clause

1. `docs/WORK_PACKAGES.md` S20-620 row joins the active list: it is
   a present-tense attestation (`... (revision 4, 2026-09-09,
   ADR-0036, ... Council reviews PASS; status `S20_620_COMPLETE`
   (2026-09-15)) ...`) going stale on exactly the three axes A1
   moves. It is live text, not frozen history, and the checker reads
   the file every run (`:148`, `:166-169`; `WORK_PACKAGE_MARKERS`
   at `:73` does not pin the stale text, which is why nothing
   catches it — noted, not changed: the row is prose, not a gate).
2. C4's verification token set is widened to `` `revision 4`,
   `S20_620_COMPLETE`, `reviews PASS`, `Council reviews PASS`,
   `implementation_complete`, `eighteen` `` (plus
   `eighteen-method`); every hit dispositioned as moved /
   frozen-history / updated in a disposition table in the
   implementation gate record. The `eighteen`-only grep could not
   see the row; the widened set can.
3. `status_note` correction: the current note asserts "the
   WORK_PACKAGES row and closeout still read 'reviews pending'",
   already false for the row (it reads "Council reviews PASS").
   Moving the note with the status means correcting that clause to
   name what each actually reads, not just its tense.
4. Frozen history restated unchanged: closeout lines under the
   "Revision 4 attestation" heading, its retained revision-2
   record, `PHASE3_V2_OFFER_DESIGN.md:163`, summary
   `p0_closed`/`p0_open` history, rev4 field values (Fix 2.1).

## Fix 4 (rev6 P2-4) — named mechanism for `revision`: argument form (a)

Adopted: option (a). `revision` takes the tx as an argument; the
agent obtains it from its own prior `open` invocation; the
fail-closed condition is a missing/invalid argument. Exact edits in
`bench/live/sley2_tool.py`:

1. `dispatch` (`:703`): replace `:717-718` with an `open` command
   (`if command == "open" and not rest: return _view(session,
   "workspace.open", "")`) followed by `if command == "revision"
   and len(rest) == 1: return _view(session, "revision.read",
   _hex(rest[0], 64).hex())`. Both route through `_request`/`call`,
   so the `:322` dispatcher guard and `_record` transcript capture
   apply to agent discovery exactly as to every other agent call.
   `session.head` is no longer read at `:718` at all — the
   separation stated plainly.
2. `_command_evidence` (`:968-1028`): add the `open` case
   (`args = {"opened": True}`) and the `revision` case
   (`args = {"tx": rest[0]}`), mirroring the existing
   `side`/`read` cases, so the agent-supplied tx is bound in the
   chained evidence.
3. The six B1 consumers (`:381-382`, `:424`, `:482`, `:751`,
   `:754-755`, `:876`, `mediated_sley.py:137`) keep reading the
   harness's own `_head`, populated by the retained `_read_head`
   (`:256-268`) at construction (`:229`) — undisturbed by
   construction, since the agent path never writes `self._head`.

Why (a) over (b): one endpoint call per command (the module runs
exactly one `dispatch` per invocation; `:8` no cross-invocation
state); `revision` stays a pure read of an explicitly supplied
discovery result, keeping agent discovery countable (`open`
invocations vs `revision` invocations with explicit tx args); and
the refusal idiom already exists (`_fail("command")` on arity,
`_hex` length check on shape). No hidden second request inside a
command, no harness-state aliasing to reason about.

## Fix 5 (rev6 P3-2) — namespace statement

`context_bounded_discovery` is not a section of
`machineresearch/sley-2.0/machine-summary.json` (verified absent
this round); it carries no register obligation and no checker reads
it. Fix 2 operates on `sley2_trial_runner.*` lane fields only. This
REQ's SECTION/FIELD is a review-request record, not a summary key —
an implementer finding no such section invents nothing.

## D carry-forward (verified, no action)

D1 `:10`/`:84` (rev6 P4 confirmed); D2 forward-record correction
stands (authoritative rev5 verdict
`REVISE_0_P0_1_P1_2_P2_1_P3_1_P4`; `95d42fda` subject-line `0P1`
uncorrected in history, corrected in the open record); D3 anchors
(`verify_cached_snapshot` `:278`; `:108`/`:131`/`:203`/`:242`
exact); B4 premise (`TOOL_METHODS` absent from
`bench/live/tests/`, pin claim documentation-only until the Fix 4
test or the strike lands).

## Acceptance (unchanged shape, revision-bound by Fix 2)

`workspace.open` (allowed, 19-name surface) → accepted-head snapshot
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
