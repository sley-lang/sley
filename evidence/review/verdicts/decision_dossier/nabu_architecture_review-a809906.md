# decision_dossier — nabu_architecture_review — Nabu (architecture and layering)

Round: Council round at a809906 (2026-09-15). Section brief: judge the f7df74f
derivation of GA section 26 criterion states and of the dossier's
security-review and independent-review items; ownership of each state; whether
the four scripts re-implement one another's predicates; whether the derivation
order is enforced; whether the lane-rotation convention has exactly one reader;
whether the 2026-09-15 rotations are recorded correctly.

## Scope verification

`git rev-parse HEAD` = `a809906f78f1bfdb9cde8da4c108c4d692dad297` (matches
SCOPE_SHA). Working tree clean at start. Checkout
`/home/dev/Work/workspaces/sley2`. Nothing was edited, staged, or generated
other than this transcript.

## Inputs read in full

- `/tmp/claude-sley2/review-brief.md`, `/tmp/claude-sley2/round-a809906-sections.md`
- `scripts/build_ga_acceptance_report.py` (349 lines), `scripts/build_decision_dossier.py`
  (784), `scripts/build_finding_register.py` (668), `scripts/check_decision_dossier.py`
  (307), `scripts/check_finding_register.py` (277), `scripts/sync_evidence_counters.py` (63)
- `docs/spec/DECISION_DOSSIER_V1.md` revision 6; `docs/spec/FINDING_REGISTER_V1.md` revision 4
- `Makefile` targets `quick`, `evidence-refresh`, `release-candidate-smoke`
- `git show f7df74f -- scripts/build_decision_dossier.py scripts/build_ga_acceptance_report.py
  machineresearch/sley-2.0/machine-summary.json`; `git show a809906 -- machineresearch/...`
- `evidence/release/decision-dossier.json`, `evidence/release/ga-acceptance-report.json`,
  `evidence/review/finding-register.json`, machine-summary sections `decision_dossier`,
  `finding_register`, `threat_coverage`, `release_decision`, `succession`,
  `reproducibility_and_independent_conformance`, `ga_acceptance`, plus every summary
  field carrying `_initial` or `_revision_`
- Prior Nabu transcript `evidence/review/verdicts/decision_dossier/nabu-final-review-d384f0f.md`
  (PASS at d384f0f/3001447; it reviewed derive_decision, not the f7df74f derivation, which
  did not exist yet)
- `bench/review/tests/test_decision_dossier.py` test inventory (names and the GA-related tests)

## Tool results (exact)

All checkers run with `SLEY2_MASTER_GOAL=/home/dev/machineresearch/Sley2.0mastergoal.md`.

| Command | Result |
|---|---|
| `python3 scripts/check_decision_dossier.py` | `FAIL`, problems `["test-inventory:drift"]`, status `S20_750_DOSSIER_IMPLEMENTED_REVIEW_PENDING`, rc 1 |
| `python3 scripts/check_finding_register.py` | `FAIL`, problems `["machine-summary:obligations"]`, rc 1 |
| `python3 scripts/build_decision_dossier.py --check` | `PASS`, 34 entries, 25 evidenced, 9 gated, `BLOCKED`, rc 0 |
| `python3 scripts/build_finding_register.py --check` | `PASS` (mode check), 384 obligations, 1 open, 0 deferred, `FINDING_REGISTER_OPEN`, rc 0 |
| `python3 scripts/build_ga_acceptance_report.py --check` | `FAIL` "the tracked GA acceptance report differs from the derived report", rc 1 |
| `python3 scripts/build_test_inventory.py --check` | `FAIL` "the tracked test inventory differs from the derived inventory", rc 1 |
| `python3 -m unittest discover -s bench/release/tests -t .` | Ran 110 tests, OK |
| `python3 -m unittest discover -s bench/review/tests -t .` | Ran 61 tests, OK |

Diff of tracked vs derived GA report (unified, n=0): line 232 evidence string
`"... across 383 obligations"` → `"... across 384 obligations"`, and
`report_digest` `5342ecac…` → `e3816b11…`. Nothing else differs.

Machine summary counters at HEAD: `finding_register.obligations` = 383;
register `obligation_count` = 384. `git show a809906` adds one obligation
(`standards_sbom_and_provenance.vulcan_surface_review_revision_5`), the
register was rebuilt (384) but `sync_evidence_counters.py` was not run and
neither the GA report nor the test inventory was rebuilt (a809906 also added
four tests to `bench/release/tests/test_standards_sbom.py`).

Probe (module loaded by path, summary patched in memory only, nothing
written): with `release_decision = {"state": "RELEASE_APPROVED",
"final_commit": "7a94a4a…"}` the GA builder raises
`UnboundLocalError: cannot access local variable 'attestation' where it is not
associated with a value`. Live `release_decision` is `null`, so the branch is
short-circuited today.

Live register facts used below: states `{PASS: 276, HISTORICAL_ROUND: 104,
PENDING: 1, OTHER: 3}`; `unclaimed_carried_findings` = 43 rows (vulcan 23,
ariadne 10, nabu 7, no lane 3); `mid_string_complete_packages` = 10 sections;
the three `OTHER` rows carry reviewer `merlin`, `None`, `None`; declared open
findings all zero; `release_candidate_packaging.release_check_gate` =
`FAIL_CLOSED_NOT_IMPLEMENTED`; `approved_conditional_items` absent.

## Independently re-derived claims

### 1. Which authority owns each state

| Fact | Owner (single writer) | Readers | Verdict |
|---|---|---|---|
| Review obligation state (PASS/PENDING/HISTORICAL_ROUND/OTHER/DEFERRED), lane, severities, clearance | `build_finding_register.py` from the machine summary (contract FR §1-3) | GA builder (rows + `result` + `declared_open_findings`), dossier item 29, `check_finding_register` (re-derives via the builder module, so not a second implementation) | Single owner. |
| Section 26 criterion state | `build_ga_acceptance_report.py` | dossier (`states` counts for rule 5 and the item-33 note) | Single owner, but no contract, no checker, no unit test governs it (grep of `docs/spec`, `docs/adr`, `bench` finds only the dossier contract and dossier tests naming it). |
| Dossier items 1-34 and decision state | `build_decision_dossier.py` | `check_decision_dossier`, `sync_evidence_counters` (mirror only) | Single owner; digests cover entries, mirrors declared non-inputs (contract §2). Held from d384f0f. |
| Machine summary mirrors (`finding_register.obligations`, `decision_dossier.evidenced_items`, …) | `sync_evidence_counters.py` | both checkers cross-check | Single writer, but only by convention (see §3). |
| Reproducibility facts (commit, digest, size, clean tree, result) | `build_reproducibility_report.py` (S20-730) | dossier items 3, 4, 24-27; GA 26.8 | Single owner. |
| Security-review verdict | machine summary `threat_coverage.independent_security_review` (hand-recorded lane field) | register row (classified by contract), GA (`startswith("PASS")`), dossier item 15 (`startswith("PASS")`) | Three classifiers of one string; two of them bypass the register (finding P3-4). |
| Independent-review verdict | summary `finding_register.independent_review`, asserted per status by `check_finding_register` | GA 26.9.3 (`== "PASS"` and `lane_clear("vulcan")`), dossier item 32 (`== "PASS"`) | Same string, two definitions of "complete PASS" (P4). |

### 2. Do the four scripts re-implement one another's predicates?

Yes, in three places, each time more loosely than the contract owner:

a. Completion. The register contract (§3, §7) defines completion by suffix
   `COMPLETE` excluding `INCOMPLETE`/`NOT_COMPLETE`, and names every
   mid-string `COMPLETE` status as "boundary language … not a completion
   claim" (`mid_string_complete_packages`). The GA builder defines
   `complete()` = `endswith("COMPLETE")` (no exclusion; harmless today, no
   live status ends INCOMPLETE/NOT_COMPLETE) and `completeish()` =
   `"COMPLETE" in status` (lines 66-67, 104-105, and inline `"COMPLETE" in
   status(...)` at 188, 223, 225). Seven sections the register names as
   mid-string complete are consumed by `completeish`/inline as terminal:
   `candidate_construction_profile`, `capability_token_profile`,
   `mutation_schema`, `protected_policy_root`, `s20_360_candidate_validation`,
   `s20_390_atomic_commit`, `s20_500_native_refs_branches`. Nine criteria
   states depend on them ("No undefined behavior", "No ambient authority",
   "agents can create…", "typed affordances…", "refs update atomically",
   "branches preserve ancestry", "policy is protected…", "capability tokens
   cannot be forged…", plus "Corruption is detected…" via the inline form).
   f7df74f introduced `completeish` specifically to move these to EVIDENCED
   (its diff changes `complete("protected_policy_root")` to `completeish(...)`).
   The GA builder should consume `register["complete_packages"]` and
   `register["mid_string_complete_packages"]` instead of re-deciding
   completion; if a restricted-boundary completion is meant to evidence a
   criterion, that mapping needs a contract sentence and a test, not a looser
   substring.

b. Lane clearance. `lane_clear(reviewer)` (GA lines 87-89) = every row of
   the lane is PASS or HISTORICAL_ROUND. The register's clearance also
   requires no `unclaimed_carried_findings`. At HEAD the Nabu lane has seven
   PASS rows carrying unclaimed severities (including
   `vm_extended_opcode_profile.nabu_architecture_review` =
   `PASS_0_P0_7_P1_6_P2_5_P3`, unclaimed P1/P2/P3), which the register says
   block clearance, yet GA 26.9.2 "Nabu approves architectural and
   cross-product boundaries" reads EVIDENCED. Same for Ariadne (10 rows) and
   26.9.1. `register_clear()` (lines 91-94) then re-adds `no PENDING` on top
   of `result == CLEAR`, which the register already guarantees: redundant
   duplication in one direction, missing condition in the other.

c. Disposition classification. The register classifies by first token over
   a closed head set with alias table and turns a self-contradicting PASS
   (`PASS_…_P0_OPEN`) into `OTHER` (FR §2). GA `field_pass` (line 101-102)
   and dossier item 15 (lines 292-295) classify the same summary string with
   `str(...).startswith("PASS")`, and neither reads the register row
   `threat_coverage.independent_security_review` that already carries the
   contract classification (state PASS, severities P3/P4). A future
   `PASS_PENDING_…_P0_OPEN` would be `OTHER` in the register, EVIDENCED in
   the dossier and in GA 26.3 "No ambient authority", 26.6 "capability
   tokens…" and "all P0/P1 threats…". The blast radius is bounded (an OTHER
   row keeps the register OPEN, so GA 26.9.4 stays AWAITS_REVIEW and the
   dossier stays BLOCKED), but item 15 would carry a value it should not.

### 3. Is the derivation order enforced or conventional?

Conventional, and the convention is wrong for the GA report:

- `evidence-refresh` (Makefile 210-216) runs `build_ga_acceptance_report`
  **before** `build_finding_register`, although the GA builder reads the
  register (obligations, result, declared counters). After the
  `sync_evidence_counters` pass the register and dossier are rebuilt; the GA
  report is not. So whenever a refresh changes the obligation set, the GA
  report lags the register by one build and the dossier consumes that
  lagging report as a source.
- `release-candidate-smoke` rebuilds the reproducibility report, register,
  and dossier, but not the GA report (which reads the repro attestation for
  26.8) nor the test inventory. The queued re-mint will therefore not clear
  the two drifts observed at HEAD by itself.
- No checker runs `build_ga_acceptance_report.py --check`.
  `check_decision_dossier` already runs `build_test_inventory.py --check`
  (lines 264-265) for the same reason (a dossier source must not drift); the
  GA report is the only dossier source with a builder and no drift gate.
- Evidence that the order is not enforced: at a809906 the register was
  rebuilt but the counter sync, the GA report, and the test inventory were
  not, and the section's two `make quick` checkers fail at HEAD
  (`machine-summary:obligations`, `test-inventory:drift`) while the dossier's
  own `--check` passes because item 29 reads the fresh register directly and
  the GA `states` counts happen to be unchanged. The fail-closed detection
  worked; the pipeline did not.

### 4. Is the lane-rotation convention read by exactly one reader?

Yes. Only `build_finding_register.py` interprets `_initial`, `_revision_N`,
`first`, `final` (`ROUND_EARLY`, `field_core`, `field_early`, `field_late`,
`supersedes`). `check_finding_register` loads that module by path and calls
`collect` (re-derivation, not re-implementation). The GA builder reads
register rows by `reviewer`/`state`, never field names. The dossier and
`check_decision_dossier` read only base lane fields. The grep across
`scripts/` and `bench/` finds no other interpreter (other `revision` hits
are contract-revision pins).

Rotations recorded at HEAD, re-derived from the register:
`decision_dossier.{ariadne,nabu,vulcan}_*_initial` FAIL rows are
`HISTORICAL_ROUND` superseded by the base field (ariadne, nabu) or
`vulcan_final_review`; base fields carry the 2026-09-13/14 final PASS form.
`reproducibility_and_independent_conformance.vulcan_surface_review_initial`
(`FAIL_2_P0_4_P1_6_P2_4_P3`) is `HISTORICAL_ROUND` superseded by
`vulcan_final_review`; base `vulcan_surface_review` = `PASS_0_P0_0_P1_0_P2_0_P3`;
`nabu_architecture_review_revision_3` (REVISE, the previously missing
0bcc9c6 round) is `HISTORICAL_ROUND` superseded by `nabu_architecture_review`.
`supersedes()` reasoning checked by hand: equal cores (`{surface}`) require
the PASS not to be early, so `…_revision_5` PASS correctly does not fold
`…_initial`, while `vulcan_final_review` (empty core, late token) does. The
rotations are recorded correctly.

### 5. Dossier derivation at HEAD (fail-closed check)

BLOCKED with exactly four reasons, each traced: "1 review obligations are
open" (item 29 `open_reviews` = 1: `root_backed_query_profile.contract_text_review`);
"no succession trial has been executed" (items 17-22 all GATED);
"the release-check and v2 product gates are fail-closed" (live stub
`NOT_IMPLEMENTED` for both, summary hand field `FAIL_CLOSED_NOT_IMPLEMENTED`,
dual-sourced); "4 GA acceptance criteria are not evidenced" (GA states 2
AWAITS_REVIEW + 2 GATED). All four are entry- or contract-named-source
reads; none reaches behind the entries. `enforce_pass_guard` unreachable,
retained. Item 15 EVIDENCED from the summary's recorded
`PASS_0_P0_0_P1_0_P2_3_P3_5_P4` (transcript
`evidence/review/verdicts/threat_coverage/independent_security_review-43f2f5b.md`
exists, 25959 bytes, but is cited only in prose, not in the entry's evidence
list, so `entry()`'s existence check does not bind it). Item 32 GATED
(`independent_review` = PENDING). Item 33 GATED by construction. Correct.

### 6. Can a GA state be EVIDENCED without evidence?

Yes. Eighteen of the fifty-two criteria are the literal `EVIDENCED`
regardless of any input (lines 163, 165, 167, 178, 180, 182, 214, 216, 218,
233, 255, 261, 265, 267, 269, 271, 273, 284). Five of those interpolate into
their evidence string the very fact their state ignores: "second clean build
establishes reproducibility" (271, ignores `repro.result`), "the source
working tree is clean" (273, ignores `working_tree_clean`), "publication
remains unauthorized" (284, ignores `publication_authorized`), "Rust and
independent oracle produce byte-identical output" (178, ignores
`cross_implementation_agreement`), "invalid candidates cannot commit" (214,
ignores the S20-360 status). Each would still read EVIDENCED if its fact
read `PACKAGE_NOT_REPRODUCIBLE`, `false`, `true`, `FAIL`, or an absent
section. Today every one of those facts is favorable, so no wrong state is
emitted at HEAD; but the f7df74f claim that "every section 26 criterion is
derived from recorded evidence" holds for 34 criteria, not 52. These are
the states the dossier's rule-5 `PASS` gate counts.

## Findings

Actionable (listed in the footer):

- P2 [architecture] The GA acceptance report is a dossier decision input
  with no drift gate and the wrong place in the pipeline: built before the
  register it reads in `evidence-refresh`, never rebuilt after the sync
  pass, absent from `release-candidate-smoke`, and no checker runs its
  `--check`. Verified stale at HEAD. Fix: order it after the final register
  build and before the dossier in both targets; add
  `run(["scripts/build_ga_acceptance_report.py", "--check"])` to
  `check_decision_dossier.py` beside the test-inventory drift check.
- P2 [derivation] Eighteen hard-coded `EVIDENCED` states, five ignoring a
  fact they cite. Fix: derive each from the fact named (repro result,
  `working_tree_clean`, `publication_authorized`, agreement, S20-360 status,
  legacy-freeze digests, conformance rejection counts, …) or downgrade to
  `AWAITS_REVIEW` with the reason; give the report a short contract section
  (it has none) and a unit test per derived predicate.
- P3 [layering] `complete`/`completeish` re-implement the register's
  completion predicate divergently; seven live mid-string-COMPLETE sections
  the register contract calls "not a completion claim" feed nine criteria as
  terminal. Fix: read `complete_packages`/`mid_string_complete_packages`
  from the register; document any boundary-completion mapping.
- P3 [layering] `lane_clear` ignores `unclaimed_carried_findings`; Nabu and
  Ariadne lanes read "clear" for 26.9.1/26.9.2 while the register carries 7
  and 10 unclaimed PASS rows for them. Fix: require the lane's rows to be
  absent from `unclaimed_carried_findings` (or read a per-lane clearance the
  register exports).
- P3 [layering] GA `field_pass` and dossier item 15 classify the
  security-review disposition with `startswith("PASS")`, bypassing the
  register's contract classifier and the existing register row; item 15
  cites neither the register nor the transcript its note names. Fix: read
  the register row's `state`, cite `REGISTER` (and the transcript path) in
  the evidence list so `entry()` binds their existence.
- P3 [correctness] `build_ga_acceptance_report.py:153-159` reads
  `attestation` before it is assigned; reproduced `UnboundLocalError` on the
  `RELEASE_APPROVED` path, i.e. the builder crashes at exactly the moment
  26.8.2 could become EVIDENCED. Fix: move line 159 above line 153.
- P3 [records] At HEAD `finding_register.obligations` (383) lags the
  register (384) and the test inventory drifts, so both section checkers in
  `make quick` fail; the queued re-mint does not run the sync-and-inventory
  path that clears them. Fix: run the builders/sync (or `evidence-refresh`)
  with the re-mint and commit the mirrors together with the register.
- P4 [editorial] Dossier item 32 note claims evidence "over a CLEAR register"
  the builder does not evaluate (the register checker enforces it
  externally); GA 26.9.3 and dossier item 32 define "complete PASS"
  differently (GA adds `lane_clear("vulcan")`); dead `accounting_path` /
  `del` at lines 135/151.

Non-actionable observations (prose only): `derive_decision` reads
`open_reviews`/`deferred_reviews` but not `result`, `unclassified`, or
`unclaimed_carried_findings`; those reach the decision only through GA
26.9.4 → "criteria not evidenced". That is one hop indirect but every hop is
a tracked source and today it blocks correctly; a future contract revision
may want rule 1 to name the register `result` directly. `lane_clear` is
global across sections (a Nabu PENDING anywhere flips 26.6.1), which is
conservative and acceptable. The prior Nabu PASS at d384f0f on
`derive_decision` stands: nothing in f7df74f touched the decision rules, and
the four BLOCKED reasons re-derive exactly.

```
VERDICT: REVISE_0_P0_0_P1_2_P2_5_P3_1_P4
SECTION: decision_dossier
FIELD: nabu_architecture_review
SCOPE_SHA: a809906f78f1bfdb9cde8da4c108c4d692dad297
FINDINGS:
[P2] [architecture] Makefile:210-216 - evidence-refresh builds the GA acceptance report before the finding register it reads and never after the sync pass; release-candidate-smoke omits it; no checker runs build_ga_acceptance_report.py --check (check_decision_dossier.py:262-265 gates test-inventory drift the same way); tracked report drifts at HEAD (383 vs 384 obligations, digest differs)
[P2] [derivation] scripts/build_ga_acceptance_report.py:163-284 - eighteen criteria are hard-coded EVIDENCED; five ignore the fact they interpolate (271 repro result, 273 working_tree_clean, 284 publication_authorized, 178 cross_implementation_agreement, 214 S20-360 status); the report has no contract section and no unit test; the dossier's PASS gate counts these states
[P3] [layering] scripts/build_ga_acceptance_report.py:66-67,104-105,188,223,225 - complete()/completeish() re-implement the register's is_complete_status more loosely; seven live sections the register names in mid_string_complete_packages ("not a completion claim", FINDING_REGISTER_V1 sec 7) feed nine criteria as terminal; consume register complete_packages instead
[P3] [layering] scripts/build_ga_acceptance_report.py:87-94 - lane_clear ignores unclaimed_carried_findings (nabu 7, ariadne 10 rows at HEAD) so 26.9.1/26.9.2 read EVIDENCED against the register's clearance rule; register_clear re-adds a condition CLEAR already implies
[P3] [layering] scripts/build_ga_acceptance_report.py:101-102 and scripts/build_decision_dossier.py:292-295 - security-review disposition classified by startswith("PASS") in two builders, bypassing the register's first-token classifier and the existing register row threat_coverage.independent_security_review; item 15 evidence list cites neither the register nor the transcript its note names
[P3] [correctness] scripts/build_ga_acceptance_report.py:153-159 - attestation used before assignment; UnboundLocalError reproduced when release_decision.state == RELEASE_APPROVED (builder crashes on the path that would evidence 26.8.2)
[P3] [records] machineresearch/sley-2.0/machine-summary.json:finding_register.obligations - 383 vs register 384 (check_finding_register FAIL machine-summary:obligations) and evidence/validation/test-inventory.json drifts (check_decision_dossier FAIL test-inventory:drift) after a809906; release-candidate-smoke does not rebuild the inventory or GA report, so the queued re-mint alone will not clear them
[P4] [editorial] scripts/build_decision_dossier.py:426-428 - item 32 note asserts "over a CLEAR register" the builder does not evaluate; GA 26.9.3 (build_ga_acceptance_report.py:280) and item 32 define "complete PASS" differently; dead accounting_path at build_ga_acceptance_report.py:135,151
SUMMARY: Ownership is clean where a contract exists: the register is the single classifier of review state and the only reader of the _initial/_revision_N rotation (check_finding_register re-derives through the builder module), the dossier is the single owner of the 34 items and the decision, and the 2026-09-15 rotations for decision_dossier and the reproducibility Vulcan/Nabu lanes re-derive correctly as HISTORICAL_ROUND under the right superseders. The dossier is BLOCKED fail-closed with four traced reasons and its derive_decision is unchanged since the d384f0f PASS. The f7df74f GA derivation is where the layering fails: the report is a decision input with no contract, no test, no drift gate, and the wrong pipeline position (built before the register it reads, verified stale at HEAD); eighteen criteria stay hard-coded EVIDENCED, five ignoring the fact they cite; and three predicates the register owns (completion, lane clearance, disposition classification) are re-implemented more loosely in the GA builder and, for one of them, in the dossier. At HEAD the section's two make-quick checkers fail on records the a809906 commit did not re-derive, and the queued re-mint does not run that path. Nothing wrong reaches a caller today; the verdict is REVISE until the GA report is gated, ordered, and derived from the register's predicates.
```
