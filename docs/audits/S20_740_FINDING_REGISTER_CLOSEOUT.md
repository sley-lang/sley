# S20-740 Finding Register Closeout

Status: **implemented under the draft Finding Register v1 contract (revision 2); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-05

Validation tier: **Tier 1 plus register-focused Tier 2 handoff**

## Claim under review

The finding register is the one machine-readable answer to which review
obligations exist, which are open, and which severity tokens their
dispositions name. Revision 2 closes all eight P0s the three Council lanes
raised against the revision-1 draft, each reproduced before fixing:

- a `COMPLETE` package whose only recorded disposition was a failure read
  `FINDING_REGISTER_CLEAR` (Ariadne P0-1, Vulcan P0-2). Reproduced with a
  synthetic `S20_999_COMPLETE` section carrying a single `FAIL_3_P0`
  verdict: clear, no violation. The builder now classifies a `FAIL`/`REVISE`
  round as historical only with a same-reviewer superseding `PASS` in the
  same section, directional from the early-or-qualified round toward the
  late-or-general review; anything else stays `PENDING`. The two live
  counterexamples the reviewers named (both restricted-query Nabu `REVISE`
  records in `COMPLETE` sections with no Nabu `PASS`) failed the build as
  `COMPLETION_VIOLATION` the moment the rule landed, and were reconciled by
  two Nabu re-reviews that both returned `PASS` with no findings (below).
- classification by `startswith` plus a hardcoded `VULCAN_PASS` alias
  admitted `PASS_PENDING_CONFIRMATION_2_P0_OPEN` as `PASS` (Ariadne P0-2).
  Classification is now first-token over the closed head set
  `PASS/PENDING/DEFERRED/FAIL/REVISE` with `VULCAN_PASS` as a declared alias
  entry, and a `PASS` head that still claims an open finding outside a
  negation group contradicts itself into `OTHER`.
- the Boundary promised open-obligation severity while no `PENDING`
  obligation carried any (Nabu P0-1). The Boundary now promises
  per-obligation severity tokens named in dispositions, `open_reviews`
  entries carry disposition and severities, and the per-finding record the
  dossier item ultimately needs is named as future work in contract section
  7 rather than claimed.
- `S20_740_COMPLETE` was unreachable: the checker demanded
  `independent_review` be `PENDING` at every status including `COMPLETE`
  while also demanding `PASS` there, with the field itself collected as a
  blocking obligation (Nabu P0-2). The register's own verdict is excluded
  from collection as review output and asserted per status instead.
- clearance read only the top-level counters and ignored every per-package
  open claim (Vulcan P0-1). `CLEAR` now requires the validated `p0`..`p4`
  counters and every per-package open list and count to read zero, recorded
  as `package_open_claims`; a missing or non-numeric counter is
  `REGISTER_SUMMARY_INVALID` rather than a vacuous clear.
- the frozen shape omitted the obligation payload behind
  `obligations_digest` (Vulcan P0-3). `obligations`, `superseded_rounds`,
  and `package_open_claims` are now part of the frozen shape, and the
  checker validates the payload count and digest against a re-derivation.
- severity tokens counted negations (Vulcan P0-4). `NO_OPEN`/`NO_NEW`
  groups are stripped before a deduplicating scan, and the contract states
  the list is distinct tokens per obligation, not a finding count.

The contract is `docs/spec/FINDING_REGISTER_V1.md` revision 2 with ADR-0042.
It is a draft: the three revision-1 reviews predate it, so the reviews that
freeze revision 2 and complete the package are pending and must pass before
the status above changes.

## Evidence

- Contract draft revision 2 and ADR-0042 (first-token classification, the
  supersession rule with its direction, negation stripping, the extended
  clearance rule, the per-status verdict assertion, the frozen obligation
  payload); the revision-2 answers are itemized in contract section 7.
- Mechanics: `scripts/build_finding_register.py` (token states, lane-aware
  directional supersession, validated counters, enriched register),
  `scripts/check_finding_register.py` (revision pin, payload digest
  re-derivation, per-status verdict, revision-agnostic markers), and
  `bench/review/tests/test_finding_register.py` (38 offline tests: exact
  heads, the contradicting `PASS`, negation stripping, lane tokens,
  supersession direction including the slice-`PASS` case, the scoped verdict
  exclusion, the `INCOMPLETE` guard, and every new clearance rule).
- Register: `evidence/review/finding-register.json` (204 obligations: 87
  `PASS`, 26 `HISTORICAL_ROUND` each naming its superseder, 79 `PENDING`,
  11 `DEFERRED`, 1 `OTHER`; severity tokens `P0` 71, `P1` 83, `P2` 86,
  `P3` 75; result `FINDING_REGISTER_OPEN`).
- Reconciliation re-reviews (read-only, pinned worktree at `9c7d4ba`):
  Nabu re-reviewed the restricted query boundary to `PASS` with no findings
  (`machineresearch/sley-2.0/reviews/s20-310-nabu-rereview-restricted-2026-09-05.log`),
  and the restricted capsule boundary to `PASS` with no findings
  (`machineresearch/sley-2.0/reviews/s20-320-nabu-rereview-restricted-2026-09-05.log`);
  both verdicts are retained in `reviews/verdicts.json` (71 verdicts: 68
  `FAIL`, 3 `PASS`). The `REVISE` dispositions stand as history and now read
  `HISTORICAL_ROUND` with `superseded_by` naming the new `PASS` fields.
- Machine summary: the `finding_register` section at contract revision 2
  with `p0_open_count` 0 and eight `p0_closed` entries; both restricted
  sections carry their new `nabu_final_review` `PASS` verdicts.

## Findings closed in revision 2

All eight S20-740 P0s (2 Ariadne, 2 Nabu, 4 Vulcan), listed under the claim
above. No P0 was closed by editing a disposition: the two reconciliations
are new recorded verdicts from the lane that issued the originals.

## Explicitly open and deferred

- The three Council reviews of revision 2 itself are pending; the section
  verdicts stay `FAIL`/`FAIL`/`PENDING` until they land.
- The independent review is pending; `S20_740_COMPLETE` stays unreachable by
  design until it records `PASS` against a clear register.
- The per-finding record (id, title, severity, disposition, owning package,
  closing commit) that an independent review starts from is future S20-740
  work, named in contract section 7.
- Completion is tested by status suffix; mid-string boundary statuses are
  not treated as complete for the violation check, though their
  unsuperseded reviews still block clearance as `PENDING` (contract section
  7). No silent pass exists either way.
- Reviewer P1/P2 items not promoted by this revision (excluded-candidate
  reporting, the obligations-digest tamper argument, per-status summary
  expectations, campaign-record staleness, substring `declares_*` flags)
  remain open in the retained logs and are not claimed here.

## Validation record

Tier 1 `make quick` and `make lint` pass at this commit. Tier 2 re-ran for
contract revision 2 on 2026-09-05: `make core`, `make conformance`,
`make adversarial`, and `make fuzz-smoke` all exited 0; the change touches
only the review-register scripts, its tests, and derived evidence, so no
package oracle changes behavior.
The full `make v1` gate was skipped because this is a subsystem handoff,
not a release boundary; `make v2` and `make release-check` remain
intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.
