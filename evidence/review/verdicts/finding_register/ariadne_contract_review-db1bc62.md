# Ariadne contract review — S20-740 finding-register package, revision 4

Subject: S20-740 finding-register package at contract revision 4
Reviewer: Ariadne (contract-review lens — exactness of rules, no silent meaning changes)
Scope SHA: db1bc623d01e838d49c153feb0be05a7502b8794 (HEAD; the rev-4 package
itself is the uncommitted working-tree delta atop it — see transcript note)
Prior verdict read: evidence/review/verdicts/finding_register/vulcan_surface_review-a4b6029.md
(REVISE, 2xP3, both claimed repaired; prior general disposition of this
section is an old FAIL, so this is a full-scope review, not a delta-only check)

## Verification transcript (commands + outputs, read-only)

- `git rev-parse HEAD` = `db1bc623d01e838d49c153feb0be05a7502b8794`. `git status`
  shows the rev-4 package as uncommitted modifications of exactly the 7
  expected files (spec, builder, checker, tests, register, machine-summary,
  decision-dossier) plus 2 unrelated untracked verdict files. HEAD's own
  message records "register 290/63 OPEN"; live reads 291/52 OPEN — the delta
  under review, not drift.
- `python3 scripts/build_finding_register.py --check` -> PASS, obligations 291,
  open_reviews 52, result FINDING_REGISTER_OPEN, exit 0.
- `python3 scripts/check_finding_register.py` -> result PASS, problems [], exit 0.
- `python3 -m unittest discover -s bench/review/tests -t .` -> 48 tests, OK
  (44 at prior scope + 4 new pinning the rev-4 repairs).
- Live artifact shape: contract_revision 4, obligation_count 291,
  states {HISTORICAL_ROUND 53, OTHER 3, PASS 183, PENDING 52},
  severity_mentions P0=57/P1=101/P2=113/P3=131/P4=21, result
  FINDING_REGISTER_OPEN, complete_packages 25, complete_packages_with_open_reviews [].
- Digests recompute exactly (obligations_digest and register_digest); independent
  `collect(summary)` rebuild equals the tracked obligations (291 rows); recomputed
  `unclaimed_carried` == tracked (39 rows) and `mid_string_complete` == tracked
  (10 rows).
- P3-1 repair verified on live data: 39 `unclaimed_carried_findings` block CLEAR
  (builder `clear` conjunction includes `not carried`); every WITH_FOLLOWUPS row
  from the prior verdict's family is present with state still PASS (no
  reclassification): reproducibility nabu P1/P3/P4 + ariadne x2 P2/P3,
  mutation_value_profile vulcan P3/P4, root_backed_query_profile x5 P3/P4
  (family grew: entity-read + contract rows), s20_700_vm fuzz slice P4 — except
  one CLOSED-token exemption, filed below as the single finding.
- P3-2 repair verified on live data: 10 `mid_string_complete_packages` with open
  counts; the verdict's three cases match exactly (s20_360 3 PENDING operation_
  analysis FAILs; s20_390 3 PENDING extended_profile FAILs; mutation_value_profile
  1 OTHER merlin TIMED_OUT row — counted via PENDING/DEFERRED/OTHER, so the OTHER
  is correctly included). Visibility only; never a violation (violations []).
- States read dispositions first-token over the closed head set + alias table:
  spot-checks PASS_WITH_P1_X->PASS, PASSIVE->OTHER, VULCAN_PASS->PASS,
  PASS_P0_OPEN/PASS_OPEN_P1->OTHER, TIMED_OUT/SELF-REVIEW/legacy_artifact_adapter
  ->OTHER all agree; no PASS row carries an OPEN_CLAIM token outside negations
  (0 leaks); all 53 HISTORICAL_ROUND rows carry a resolving same-section
  same-lane PASS superseder (0 bad).
- CLEAR rule: spec section 3 text (no PENDING, no OTHER, unclaimed empty,
  complete_packages_with_open_reviews empty, all top-level counters zero, all
  per-package claims zero; DEFERRED not blocking by itself) matches the builder's
  `clear` conjunction conjunct-for-conjunct; recomputed from the live artifact =
  False, result OPEN. Exact.
- Spec revision-4 text matches implementation: unclaimed definition (PASS +
  severities after negation-strip/zero-absence + no CLOSED token + per-severity
  untracked in section pN_open list/count) == `unclaimed_carried` +
  `section_claimed_severities` (top-section node, non-empty list or positive
  count); mid-string definition (status contains COMPLETE without satisfying the
  suffix test, named with open counts) == `mid_string_complete` +
  `is_complete_status` (excludes INCOMPLETE/NOT_COMPLETE). Checker pins all
  justified: contract_revision 4, carried-shape / mid-complete-shape checks, and
  `unclaimed_carried` / `mid_string_complete` script markers.

## Findings

P3 [contract] docs/spec/FINDING_REGISTER_V1.md:148-154 (unclaimed "no CLOSED
token" exemption); scripts/build_finding_register.py:321-350 (`if
item["declares_closed_findings"]: continue`) — the CLOSED exemption is boolean
per row, not per severity, so one live PASS escapes the P3-1 repair it was meant
to close: s20_700_scb1_persistent_fuzz_slice.vulcan_review
`PASS_PRIOR_P2_CLOSED_NO_OPEN_P0_P1_P2_WITH_P3_P4_FOLLOWUPS` carries P2/P3/P4 yet
is exempt because P2 closed, leaving the explicit P3/P4 followups (the prior
verdict's WITH_FOLLOWUPS family member) able to survive into a CLEAR read while
every sibling WITH_FOLLOWUPS row without a CLOSED token (e.g. the three
complete_entity_impact_profile `PASS_WITH_P4_NOTES_*` rows) correctly blocks.
States unchanged, result stays OPEN, so precision — not a silent pass. Fix
direction: exempt per severity (only severities covered by the CLOSED/NO_OPEN
claim), or require the CLOSED declaration to name the carried severities.

VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3 / SECTION: finding_register / FIELD: ariadne_contract_review / SCOPE_SHA: db1bc623d01e838d49c153feb0be05a7502b8794 / FINDINGS: 1

---
## RE-REVIEW 2026-09-13 (carried-scope delta only)

Scope: ONLY the per-severity repair of my P3 above
(`closed_severities()` / `negated_severities()` / `strip_negations()` +
`unclaimed_carried()` per-severity rule, spec section 3 text, 3 new pinning
tests). Machine-summary disposition updates in the same tree (genuine Council
reviews) are out of scope and noted as such where they move counts.

Verification transcript (read-only commands):
- `python3 -m unittest discover -s bench/review/tests -t .` -> 51 tests, OK
  (48 prior + 3 new: partial_closure, stacked_negation, closed_substring).
- `python3 -m unittest bench.review.tests.test_finding_register.InvariantTests.test_a_closed_carried_finding_does_not_block_clearance`
  -> OK (closed-scope still honors PRIOR_P1_CLOSED).
- `python3 scripts/build_finding_register.py --check` -> PASS, obligations
  291, open_reviews 51, result FINDING_REGISTER_OPEN, exit 0.
- Live scb1 row: `s20_700_scb1_persistent_fuzz_slice.vulcan_review`
  `PASS_PRIOR_P2_CLOSED_NO_OPEN_P0_P1_P2_WITH_P3_P4_FOLLOWUPS` now reads
  unclaimed [P3, P4] (`closed_severities` = [P2] only). My P3 is closed
  exactly: unclaimed 39 -> 40, the +1 being precisely this row.
- PRIOR-closed family still clean (no over-listing): the 3 clean_room
  `PASS_PRIOR_P0_P1_P2_P3_CLOSED_*` rows (closed = all four) and the
  s20_360 `nabu_review` PRIOR-closed row are absent from
  `unclaimed_carried_findings`; the `PASS_P3_CORPUS_BREADTH_CLOSED_*` rows
  (s20_360 vulcan_review, s20_700-audit candidate_result) stay claimed via
  their valid `NO_OPEN_..._P3_P4` negation, not via the CLOSED substring.
- Unit probes: DISCLOSED/UNCLOSED -> closed [] (no smuggling);
  `NO_NO_OPEN_P1` -> negated [] / severities [P1] (stacked void, stays
  visible); `PRIOR_P1_CLOSED` -> [P1] (prior exemption preserved).
- No rule-caused reclassification: independent `collect()` rebuild equals
  the tracked obligations (digests match, states recompute exactly);
  recomputed `unclaimed_carried` == tracked (40) and `mid_string_complete`
  == tracked (10). The PASS 183->184 / PENDING 52->51 shift is NOT the
  rule (`classify_token` untouched): it is the genuine
  root_backed_query_profile `vulcan_surface_review` REVISE->PASS disposition
  change in machine-summary. Severity-mention shifts (P0 57->55, P1
  101->99, P2 113->111, P3 131->130) likewise trace to genuine disposition
  changes, not to the rule (no live stacked-negation instance exists for
  the strip change to move).
- No new silence: result stays FINDING_REGISTER_OPEN; every
  WITH_FOLLOWUPS sibling still blocks; substring and stacked evasions are
  pinned by tests with no live instances.

Out-of-scope observation (not a delta finding, unseverity-tagged):
`python3 scripts/check_finding_register.py` currently reports FAIL /
`machine-summary:open_reviews` because the finding_register section tally
in machine-summary still says open_reviews 52 while the live register has
51 after the genuine root_backed_query_profile PASS above. Stale tally from
out-of-scope review updates, not a contract-rule defect; the delta's own
checker markers (carried-shape, mid-complete-shape, contract_revision 4)
are present and pass.

New findings: none.

RE-REVIEW: PASS / SCOPE: carried-scope-delta
