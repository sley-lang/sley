# Vulcan surface review — section `decision_dossier`, field `vulcan_surface_review` (adversarial / security role)

Date: 2026-09-15. Round: a809906 council round (`/tmp/claude-sley2/round-a809906-sections.md`).
Subject: the evidence-derived states introduced by f7df74f in
`scripts/build_ga_acceptance_report.py` and `scripts/build_decision_dossier.py`,
judged against `docs/spec/DECISION_DOSSIER_V1.md` revision 6 and
`docs/spec/FINDING_REGISTER_V1.md` revision 4, plus the records of this section at HEAD.
Prior Vulcan transcripts read: `vulcan-final-review-d384f0f.md` (PASS, harness final, 2026-09-14);
the Ariadne and Nabu d384f0f finals were skimmed for the list of previously closed items only.

## 1. Scope verification

- `git rev-parse HEAD` = `a809906f78f1bfdb9cde8da4c108c4d692dad297` (matches SCOPE_SHA). Branch main,
  checkout `/home/gfarch/Work/workspaces/sley2`.
- Working tree at start: clean. During the review other council lanes wrote their own
  `*-a809906.md` transcripts (root_backed_query_profile, reproducibility, standards); I wrote nothing
  but this file, ran no builder in write mode against the tracked tree, staged nothing.
- `git log --oneline 7a94a4a..HEAD` = a809906, e11103e, 08f577d, 75b5491, 161a2f3, 4168332, 70283ce,
  f7df74f, 6a2eef7. `git show --stat f7df74f` confirms the two builders, the dossier, the GA report,
  the register, and the machine summary all changed in that commit.

## 2. Inputs read in full

`/tmp/claude-sley2/review-brief.md`; `/tmp/claude-sley2/round-a809906-sections.md`;
`scripts/build_ga_acceptance_report.py` (349 lines); `scripts/build_decision_dossier.py` (784 lines);
`scripts/check_decision_dossier.py` (307); `scripts/check_finding_register.py` (277);
`docs/spec/DECISION_DOSSIER_V1.md` (278); `docs/spec/FINDING_REGISTER_V1.md` (299);
`git show f7df74f -- scripts/build_decision_dossier.py scripts/build_ga_acceptance_report.py` (the whole diff);
`scripts/build_finding_register.py` lines 36-300 (exclusion regexes, alias table, `is_obligation`, `collect`);
`scripts/gate_status.py`; the Makefile `quick` and `evidence-refresh` targets;
`evidence/review/verdicts/decision_dossier/vulcan-final-review-d384f0f.md`;
`evidence/release/decision-dossier.json` (header and the decision-input entries);
`evidence/release/ga-acceptance-report.json` (states, gated, awaiting lists, per-criterion diff against a fresh derivation);
`evidence/review/finding-register.json` (header, states, open_reviews, unclaimed_carried_findings, unclassified, the
decision_dossier and threat_coverage rows); the machine-summary sections `decision_dossier`, `finding_register`,
`threat_coverage`, `succession`, `release_decision` (absent), `release_candidate_packaging`, `ga_acceptance`;
`bench/review/tests/test_decision_dossier.py` (test names and the GA/threshold/independent-review references).

## 3. Tool results (exact)

Environment for every checker: `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`.

| command | result |
|---|---|
| `python3 scripts/check_decision_dossier.py` | `FAIL`, problems `["test-inventory:drift"]`, status `S20_750_DOSSIER_IMPLEMENTED_REVIEW_PENDING`, rc 1 |
| `python3 scripts/check_finding_register.py` | `FAIL`, problems `["machine-summary:obligations"]` (summary mirror 383, register 384), rc 1 |
| `python3 scripts/build_ga_acceptance_report.py --check` | `FAIL` "the tracked GA acceptance report differs from the derived report", rc 1 |
| `python3 scripts/build_decision_dossier.py --check` | `PASS`, BLOCKED, 34 entries, 25 evidenced, 9 gated, rc 0 |
| `python3 scripts/build_finding_register.py --check` | `PASS`, 384 obligations, 1 open review, `FINDING_REGISTER_OPEN`, rc 0 |
| `python3 scripts/gate_status.py v2` / `release-check` | `NOT_IMPLEMENTED`, rc 2 (both) |
| `python3 -m unittest discover -s bench/release/tests -t .` | Ran 110 tests, OK |
| `python3 -m unittest discover -s bench/review/tests -t .` | Ran 61 tests, OK |

Derivation diff of the GA report (in-process `build_report()` vs tracked file): states identical
(48 EVIDENCED, 2 AWAITS_REVIEW, 2 GATED); the only differing criterion is "no P0/P1/P2 finding remains open",
whose evidence string reads "across 384 obligations" derived vs "across 383 obligations" tracked.

Register delta f7df74f..HEAD: eight obligations added (seven `zjx_transport_readiness` lane fields, one
`standards_sbom_and_provenance.vulcan_surface_review_revision_5`), none removed. The register file was rebuilt
(384, `--check` PASS) but the summary's `finding_register.obligations` counter still reads 383.

Mutation experiments. All run in a scratch copy under
`/tmp/claude-1000/-home-gfarch/481411f0-c394-4b42-ae75-ae86d05d4818/scratchpad/sley2-scratch/` containing only the
files the two builders read (scripts, machine summary, the eleven evidence inputs, `gate_status.py`); the copy reproduced
HEAD's three `--check` results before any mutation, and the tracked tree was verified untouched afterwards.

| # | mutation (machine summary unless stated) | GA report | register (rebuilt in scratch) | dossier |
|---|---|---|---|---|
| E1 | `release_decision = {state: RELEASE_APPROVED, final_commit: 7a94a4a…}` | builder crashes: `UnboundLocalError: cannot access local variable 'attestation'` at line 156 | — | — |
| E2 | `threat_coverage.independent_security_review = PASS_PENDING_CONFIRMATION_2_P0_OPEN` | "No ambient authority exists", "capability tokens cannot be forged…", "all P0/P1 threats have passing tests" all EVIDENCED | row state `OTHER`, listed in `unclassified`, result OPEN | item 15 EVIDENCED, value `PASS_PENDING_CONFIRMATION_2_P0_OPEN`; state BLOCKED |
| E2b | same field = `PASSED_TO_NEXT_ROUND`, `PASS_2_P1`, `PASS_WITH_OPEN_P1` | each: "all P0/P1 threats…" EVIDENCED | — | each: item 15 EVIDENCED with that value |
| E3 | `finding_register.independent_review = PASS`, register left OPEN | "Vulcan or current independent reviewer issues a complete PASS" EVIDENCED; "all reviewer findings resolved…" stays AWAITS_REVIEW | untouched (OPEN, 1 PENDING) | item 32 EVIDENCED value `PASS` citing the OPEN register; BLOCKED with "1 review obligations are open", "3 GA acceptance criteria are not evidenced" |
| E4 | `succession.thresholds = {strict_correctness: PASS}`, `trials_executed = 1` | "every section 22 threshold passes" EVIDENCED, evidence "succession trials_executed 1; threshold rows strict_correctness=PASS" | — | still "no succession trial has been executed" (arm entries GATED); BLOCKED |
| E5 | tracked GA report hand-edited: `states = {EVIDENCED: 52}`, lists emptied | — | — | builds (rc 0); reason "4 GA acceptance criteria are not evidenced" gone; BLOCKED on the remaining three reasons |
| E6 | `decision_dossier.vulcan_surface_review_initial` renamed to `…_initial_note` | — | the FAIL row vanishes (only `vulcan_final_review`, `vulcan_surface_review` remain) | — |
| E7 | `decision_dossier.nabu_architecture_review = PASS_WITH_2_P1_FOLLOWUPS` | "Nabu approves architectural…" and "policy is protected…" EVIDENCED | row state PASS, listed in `unclaimed_carried_findings` with `['P1']`, result OPEN | — |

The dossier's decision state never left BLOCKED in any experiment.

## 4. Independently re-derived claims

1. BLOCKED derives fail-closed from the entries, as revision 6 section 3 requires: the open-obligation reason comes
   from the "findings by severity and disposition" entry (open_reviews 1 at HEAD: `root_backed_query_profile.contract_text_review`
   PENDING); the no-trial reason from the six arm entries all GATED; the gate reason from the live stub (rc 2,
   NOT_IMPLEMENTED) dual-sourced with `release_candidate_packaging.release_check_gate`; a gated decision-input entry
   blocks with its fact named (`test_a_gated_findings_entry_fails_closed`). E2-E5 each removed at most one reason and
   the state stayed BLOCKED. `enforce_pass_guard` is present and tested. The dossier cannot reach PASS from a
   non-CLEAR register at HEAD because the register entry carries `open_reviews` and the GA report carries the two
   AWAITS_REVIEW criteria that themselves depend on `register_clear()`; the weak points are in the individual item and
   criterion states, not in the terminal decision.
2. The accepted PASS forms are NOT bare-or-zero-finding. `build_ga_acceptance_report.py:101-102` defines
   `field_pass` as `startswith("PASS")`, used at line 107 for the independent security review; `build_decision_dossier.py:292-295`
   uses the same `startswith("PASS")` for item 15. Only `independent_review_pass` (GA line 108, dossier line 424) binds the
   bare token. The register contract (section 2) classifies by first token over a closed head set and turns a PASS head
   with an open claim into OTHER; the two builders introduced by f7df74f do not reuse that classification, so the same
   string reads OTHER in the register and EVIDENCED in the GA report and dossier (E2). No checker binds the form of
   `threat_coverage.independent_security_review` (`grep` finds only the builder's `"PENDING"` initialiser in
   `build_threat_coverage_report.py:369`).
3. `lane_clear` (GA lines 87-89) reads register row `state` only. The register's clearance rule (contract section 3)
   also blocks on `unclaimed_carried_findings`, which revision 4 added precisely so a `PASS` that still names findings
   cannot clear. At HEAD the register lists 43 unclaimed rows, including nabu `vm_extended_opcode_profile.nabu_architecture_review
   = PASS_0_P0_7_P1_6_P2_5_P3` (P1, P2, P3 unclaimed), nabu `reproducibility_and_independent_conformance.nabu_architecture_review
   = PASS_WITH_P1_P3_P4_FOLLOWUPS_NO_P0_P2` (P1), ariadne `repository_exchange.ariadne_implementation_review
   = PASS_IMPLEMENTATION_AFTER_P1_REMEDIATION` (P1), ariadne `reproducibility…ariadne_contract_review = PASS_WITH_P2_P3_FOLLOWUPS_NO_P0_P1`
   (P2). `lane_clear("nabu")` and `lane_clear("ariadne")` are nevertheless True, so at HEAD the criteria "Nabu approves
   architectural and cross-product boundaries", "policy is protected from the judged candidate", "Ariadne approves SSMC1 and
   semantic correctness", and "No undefined behavior exists in the program model" read EVIDENCED while the register's own
   rule says those lanes' rows block clearance. E7 shows the generic form `PASS_WITH_2_P1_FOLLOWUPS` keeps a lane clear.
   The GA builder comment (lines 78-84) claims "the register digest binds the dispositions"; the report records no register
   digest.
4. The GA report is a decision input (dossier rule 1 count, rule 5 gate) that nothing verifies: `build_dossier` loads it
   with the contract tag only (line 677), never checks `report_digest`, and `make quick` runs no
   `build_ga_acceptance_report.py --check` (the only Makefile reference is the write-mode call in `evidence-refresh`,
   line 211; the tests only `load` it). The live tree demonstrates the gap: the tracked report is stale (383 vs 384) yet
   `build_decision_dossier.py --check` PASSes, and E5 shows a hand-edited states table silently removes a BLOCKED reason.
5. `final_commit_fixed` (GA lines 153-157) references `attestation`, assigned at line 159. Today
   `release.get("state") == "RELEASE_APPROVED"` is False so the `and` short-circuits; the moment an operator records
   RELEASE_APPROVED the builder raises (E1). The direction is fail-closed (no report, no wrong state) but the criterion
   "artifact is built from the final candidate commit" can never reach EVIDENCED through this code, and `packaging`
   (line 158) and `accounting_path` (lines 135, 151) are dead.
6. Section 22 thresholds stay GATED at HEAD (no `succession.thresholds` table, `trials_executed` 0). But the GA builder reads a
   hand summary table (`succession.thresholds`, a dict of strings) while the dossier reads a different hand key
   (`succession.thresholds_pass`, a bool, line 655); neither is bound to the S20-630 accounting report, whose path the GA
   builder computes and deletes. E4 makes 26.7 EVIDENCED from a one-key hand edit while the dossier still reports no trial.
7. Item 32 (dossier lines 424-428) is evidenced from `finding_register.independent_review == "PASS"` alone; the note says
   "over a CLEAR register" but the builder never reads `register["result"]`. E3 evidences it over an OPEN register citing
   that register as evidence. Exposure is bounded because `check_finding_register.py:174-177` pins that field to PENDING unless
   S20-740 is COMPLETE, and COMPLETE requires `register_result == FINDING_REGISTER_CLEAR`; the builder still states a guard it
   does not implement.
8. Missing transcript: the item 15 note names "transcripts under evidence/review/verdicts/threat_coverage/" but its
   `evidence` list is `[SUMMARY, THREAT_COVERAGE]`; the cited-evidence existence check never covers the transcript
   (`independent_security_review-43f2f5b.md` exists today; its deletion would change nothing).
9. Renamed lane field: E6 shows a `_note` suffix removes a FAIL round from the register (contract section 1 exclusion). At HEAD
   the decision_dossier `_initial` FAILs are already HISTORICAL_ROUND, so no state changes here; this is the register lanes'
   accepted design and inherited, not a f7df74f defect. Observation only.
10. Lane rotations: the three `decision_dossier` `_initial` fields carry the 2026-09-04 FAIL values with a dated note; the base
    fields carry the d384f0f finals; the register classifies all three `_initial` rows HISTORICAL_ROUND (superseded by
    `ariadne_contract_review`, `nabu_architecture_review`, `vulcan_final_review`). The three final transcripts exist under
    `evidence/review/verdicts/decision_dossier/`; no in-repo transcript records the 2026-09-04 FAIL rounds (the finals recite them
    secondhand). The base values are the closure form `PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4`, which the
    checker will not accept for `S20_750_COMPLETE` (bare `PASS` required, check line 276); f7df74f normalised other sections
    but not this one. Not actionable now: this round's three verdicts replace those values.
11. Records at HEAD: `check_decision_dossier` is red on `test-inventory:drift` (a809906 added four tests to
    `bench/release/tests/test_standards_sbom.py`; the inventory was not rebuilt), `check_finding_register` is red on the stale
    383 mirror, and the GA report's evidence string is stale. None of these is candidate-binding, so the round brief's
    pre-re-mint exemption does not cover them; `make quick` fails at HEAD on this section's own checker. The machine
    summary's `ga_acceptance` mirror reads `{AWAITS_REVIEW: 16, EVIDENCED: 34, GATED: 2}` at f7df74f and at HEAD while the
    report f7df74f wrote reads 48/2/2; nothing reads or checks the mirror.

## 5. Per-item analysis against the round brief

- "Is every derived state traceable to a recorded verdict or checker result?" Package-status criteria: yes (summary statuses
  the per-package checkers gate). Review criteria: traceable to register rows, but to their `state` only, not to the register's
  clearance verdict (claim 3). Security criteria and item 15: traceable to a summary string prefix, not to a classified verdict
  or a transcript (claims 2, 8). Item 32: traceable to a summary string that another checker pins (claim 7).
- "Can a state be EVIDENCED without evidence?" Yes, at the criterion/item level: E2, E2b, E3, E4, E7 each flip a state with one
  summary string or one hand key; E5 flips a dossier reason with a hand-edited report. The terminal decision stayed BLOCKED
  every time.
- "Does BLOCKED derive fail-closed?" Yes (claim 1).
- "Are the 2026-09-15 lane rotations recorded correctly?" Yes for fields, notes, and register supersession; the initial rounds
  lack an in-repo transcript (claim 10).
- "Section 22 thresholds stay GATED?" At HEAD yes; the guard is a hand key with no accounting-report binding and a key that
  differs from the dossier's (claim 6).

## 6. Findings

Actionable items are listed in the footer. Non-actionable observations: the `_note`-suffix exclusion (claim 9) belongs to the
register lanes; the closure-form base values in this section (claim 10) are superseded by this round; the live
`PASS_0_P0_0_P1_0_P2_3_P3_5_P4` security verdict is arguably an honest pass for the "all P0/P1 threats" criterion (its findings
are P3/P4 hygiene), so the P2 below is about the accepted form set, not about that instance.

```
VERDICT: REVISE_0_P0_0_P1_4_P2_4_P3_2_P4
SECTION: decision_dossier
FIELD: vulcan_surface_review
SCOPE_SHA: a809906f78f1bfdb9cde8da4c108c4d692dad297
FINDINGS:
[P2] [derivation] scripts/build_ga_acceptance_report.py:101 - `field_pass` accepts any `PASS`-prefixed string: `PASS_PENDING_CONFIRMATION_2_P0_OPEN`, `PASSED_TO_NEXT_ROUND`, `PASS_2_P1`, `PASS_WITH_OPEN_P1` in `threat_coverage.independent_security_review` each make three section 26 criteria EVIDENCED while the register classifies the same string OTHER; no checker binds that field's form. Accept only the bare token or an enumerated zero-finding form (reuse the register's `classify_token`/row state and its unclaimed rule), and record the register digest the comment at lines 78-84 claims binds the dispositions.
[P2] [derivation] scripts/build_decision_dossier.py:292 - item 15 "security review result" uses the same `startswith("PASS")`, publishes the raw string as the value, and cites `[SUMMARY, THREAT_COVERAGE]` but not the transcript its note names, so a self-contradicting verdict evidences the item and a missing transcript never fails the build. Bind the form as above and cite the transcript path in `evidence`.
[P2] [derivation] scripts/build_ga_acceptance_report.py:87 - `lane_clear` reads row `state` only and ignores `unclaimed_carried_findings` (and OTHER rows without a lane); `PASS_WITH_2_P1_FOLLOWUPS` keeps a lane clear (E7). Live at HEAD: nabu rows `vm_extended_opcode_profile` (`PASS_0_P0_7_P1_6_P2_5_P3`) and reproducibility (`PASS_WITH_P1_P3_P4_FOLLOWUPS`) and ariadne `repository_exchange` (`…AFTER_P1_REMEDIATION`) are unclaimed P1 rows blocking register clearance while "Nabu approves…", "policy is protected…", "Ariadne approves SSMC1…", and "No undefined behavior…" read EVIDENCED. A lane is clear only when none of its rows appears in `unclaimed_carried_findings` or `unclassified`.
[P2] [binding] scripts/build_decision_dossier.py:677 - the GA report is a decision input (rule 1 count, rule 5 gate) loaded by contract tag only; `report_digest` is never verified and no `build_ga_acceptance_report.py --check` runs under `make quick` (Makefile:211 is write mode). A hand-edited states table removes the "GA acceptance criteria are not evidenced" reason (E5), and the live tree already carries a stale report (383 vs 384) under a passing dossier `--check`. Verify the digest in the builder and add the `--check` to `quick`.
[P3] [defect] scripts/build_ga_acceptance_report.py:156 - `attestation` is used before its assignment at line 159; the first `release_decision.state == RELEASE_APPROVED` crashes the builder with UnboundLocalError (E1), so "artifact is built from the final candidate commit" can never reach EVIDENCED by this path. Move the assignment above line 153; drop the dead `packaging` (158) and `accounting_path` (135/151).
[P3] [derivation] scripts/build_decision_dossier.py:424 - item 32 is evidenced from `finding_register.independent_review == "PASS"` alone; the note claims "over a CLEAR register" but `register["result"]` is never read, and E3 evidences the item citing an OPEN register. Exposure is bounded by `check_finding_register.py:174` pinning the field; make the builder require `FINDING_REGISTER_CLEAR` as its note states.
[P3] [binding] scripts/build_ga_acceptance_report.py:138 - the section 22 criterion reads a hand summary dict `succession.thresholds` plus hand `trials_executed`, while the dossier (line 655) reads a different hand key `succession.thresholds_pass`; neither is bound to the S20-630 accounting report. A one-key edit evidences 26.7 (E4) while the dossier still reports no trial. Read one key, derived from the tracked accounting report.
[P3] [records] evidence/validation/test-inventory.json:1 - HEAD records drift outside the re-mint exemption: `check_decision_dossier` FAIL `test-inventory:drift` (a809906 added four tests), `check_finding_register` FAIL `machine-summary:obligations` (summary 383, register 384 after the ZJX and standards rev-5 obligations), GA report evidence string stale; `make quick` is red at HEAD on this section's checker. Records-only refresh and the summary sync write-back before the queued re-mint.
[P4] [records] machineresearch/sley-2.0/machine-summary.json:1 - the `ga_acceptance` mirror reads `{AWAITS_REVIEW: 16, EVIDENCED: 34, GATED: 2}` at f7df74f and HEAD while the report f7df74f wrote reads 48/2/2; nothing reads or checks the mirror. Sync it or have `check_decision_dossier` cross-check it like the dossier counters.
[P4] [records] machineresearch/sley-2.0/machine-summary.json:1 - the rotated `decision_dossier.*_initial` FAIL values (2026-09-04) cite no transcript; only the d384f0f finals exist under `evidence/review/verdicts/decision_dossier/`, and they recite the initial rounds secondhand. Name the initial-round record (or state that none is tracked) in the `_note` fields.
SUMMARY: The dossier's terminal state is sound: BLOCKED derives from the findings entry, the six gated arm entries, the live fail-closed gate stub dual-sourced with the summary hand field, and the GA pending count, and it stayed BLOCKED under every mutation I ran; the checker chain (`check_finding_register`, `check_decision_dossier`, 61 review tests, 110 release tests) runs green apart from honest record drift at HEAD. The f7df74f derivations beneath it are not fail-closed at the criterion and item level: `startswith("PASS")` in both builders admits verdict strings the register itself classifies OTHER, `lane_clear` ignores the register's unclaimed-carried rule so lanes with unclaimed P1 rows read approved at HEAD, the GA report is consumed as a decision input with no digest check and no `--check` in `quick` (and is stale right now), the release-approval path crashes on an unbound name, item 32 claims a CLEAR-register guard it does not implement, and the section 22 threshold reads an unbound hand key that differs from the dossier's. Four P2, four P3, two P4; nothing reaches a P0/P1 because the terminal decision and the product gates hold.
```
