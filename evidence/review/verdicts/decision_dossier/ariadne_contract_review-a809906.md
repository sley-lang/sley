# Ariadne contract review — section `decision_dossier`, field `ariadne_contract_review`

Role: Ariadne (contract conformance), Sley 2 council. Date: 2026-09-15.
Contract under review: `docs/spec/DECISION_DOSSIER_V1.md` revision 6 (S20-750),
judged against `scripts/build_decision_dossier.py`,
`scripts/build_ga_acceptance_report.py`, `scripts/check_decision_dossier.py`,
and the tracked records `evidence/release/decision-dossier.json`,
`evidence/release/ga-acceptance-report.json`,
`evidence/review/finding-register.json`. Focus per the round brief: the
f7df74f change that made the GA section 26 states and the dossier's
security-review and independent-review items evidence-derived.

## 1. Scope verification

`git rev-parse HEAD` in `/home/gfarch/Work/workspaces/sley2` returned
`a809906f78f1bfdb9cde8da4c108c4d692dad297`, matching SCOPE_SHA. `git status
--porcelain` was empty before and after the review (the only write is this
transcript). Read-only throughout: no builder was run in write mode, nothing
was staged, nothing pushed.

## 2. Inputs read in full

- `/tmp/claude-sley2/review-brief.md`, `/tmp/claude-sley2/round-a809906-sections.md`
- `docs/spec/DECISION_DOSSIER_V1.md` (revision 6, 278 lines)
- `docs/spec/FINDING_REGISTER_V1.md` (revision 4)
- `scripts/build_decision_dossier.py` (784 lines), `scripts/build_ga_acceptance_report.py` (349 lines), `scripts/check_decision_dossier.py` (307 lines)
- `evidence/release/decision-dossier.json` (all 34 entries), `evidence/release/ga-acceptance-report.json` (all 52 criteria), `evidence/review/finding-register.json` (headline fields, all rows for the sections rotated on 2026-09-15, unclaimed/unclassified/superseded lists)
- `machineresearch/sley-2.0/machine-summary.json` sections `decision_dossier`, `threat_coverage`, `finding_register`, `release_candidate_packaging`, `succession`, `ga_acceptance`, `open_findings`, `reproducibility_and_independent_conformance`
- `git show f7df74f` for the three scripts, the contract, and the summary's review fields; `git log --stat 7a94a4a..HEAD`
- prior transcript `evidence/review/verdicts/decision_dossier/ariadne-final-review-d384f0f.md` (harness final, PASS at d384f0f); the Nabu and Vulcan d384f0f finals were listed but not relied on
- `bench/review/tests/test_decision_dossier.py` (test names; the decision-rule and GA-rule tests read in full), `Makefile` targets `quick`, `evidence-refresh`, `release-candidate-smoke`
- `docs/WORK_PACKAGES.md` S20-750 row

## 3. Tool results (exact)

All checkers ran with `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`.

| Command | Result |
|---|---|
| `python3 scripts/check_decision_dossier.py` | **FAIL**, `problems: ["test-inventory:drift"]`, status `S20_750_DOSSIER_IMPLEMENTED_REVIEW_PENDING`, required_items 34, exit 1 |
| `python3 scripts/build_decision_dossier.py --check` | PASS: entries 34, evidenced 25, gated 9, decision_state BLOCKED |
| `python3 scripts/build_ga_acceptance_report.py --check` | **FAIL**: "the tracked GA acceptance report differs from the derived report", exit 1 |
| `python3 scripts/build_test_inventory.py --check` | **FAIL**: "the tracked test inventory differs from the derived inventory" (tracked `python_tests` 359, derived 363; `bench/release/tests` module row differs; digest differs) |
| `python3 scripts/build_finding_register.py --check` | PASS: obligations 384, open_reviews 1, deferred 0, result FINDING_REGISTER_OPEN |
| `python3 scripts/check_finding_register.py` | FAIL, `problems: ["machine-summary:obligations"]` (summary mirror 383, register 384) — another lane's checker, recorded here because it shares the cause of the drift above |
| `python3 -m unittest discover -s bench/review/tests -t .` | 61 tests OK (0.609 s) |
| `python3 -m unittest discover -s bench/release/tests -t .` | OK (output tail is a test's own printed JSON fixture with `"result": "FAIL"`, not a failure; the suite passes) |
| In-memory GA derivation diff vs tracked report | one criterion differs: "no P0/P1/P2 finding remains open" evidence reads "across 383 obligations" tracked vs "384" derived; `report_digest` 5342ecac… → e3816b11… |
| Obligation-count trace | f7df74f: register 376 / summary 376 / GA text 376; e11103e: 383 / 383 / 376; a809906: 384 / 383 / 383 |
| Gate stubs (via the checker) | `release-check` and `v2` both `NOT_IMPLEMENTED`, exit 2 (stay closed) |

Simulation (read-only, sources mutated in memory by monkeypatching
`build_ga_acceptance_report.load`; script kept in the session scratchpad):

| Mutation | Observed |
|---|---|
| `release_decision = {state: RELEASE_APPROVED, final_commit: 7a94a4a…}` | `UnboundLocalError: cannot access local variable 'attestation' where it is not associated with a value` |
| repro `result = SINGLE_HOST_REPRODUCIBLE`, `working_tree_clean = false` | "second clean build establishes reproducibility" → EVIDENCED, evidence "reproducibility report result SINGLE_HOST_REPRODUCIBLE"; "the source working tree is clean" → EVIDENCED, evidence "working_tree_clean False" |
| repro `attestations = []` | "artifact name …" → EVIDENCED, evidence "reproducibility report attests no attestation"; "working tree is clean" → EVIDENCED with `None` |
| summary `conformance.cross_implementation_agreement = FAIL` | "Rust and independent oracle produce byte-identical output" → EVIDENCED, evidence "…agreement FAIL" |
| conformance `independently_checked = 3` | "Every non-canonical fixture is rejected" → EVIDENCED, evidence "3 of 25 fixture families independently checked" |
| `threat_coverage.independent_security_review = FAIL_1_P1` | three criteria correctly fall to AWAITS_REVIEW (ambient authority, capability tokens, P0/P1 threats) |
| one Ariadne register row → PENDING | "Ariadne approves…" and "No undefined behavior…" correctly fall to AWAITS_REVIEW |
| no Vulcan rows at all | "Vulcan … complete PASS" correctly stays AWAITS_REVIEW (`lane_clear` requires rows) |
| `finding_register.independent_review = PASS` with the register still OPEN | "Vulcan … complete PASS" → EVIDENCED; "all reviewer findings resolved" stays AWAITS_REVIEW |
| `s20_530_crash_recovery.status = IN_PROGRESS` | "Corruption is detected before ref advancement" → AWAITS_REVIEW, but "crash recovery produces only old or complete new state" stays EVIDENCED |

Source audit: 18 of the 52 criteria are literal `EVIDENCED` constants (lines
163, 165, 167, 178, 180, 182, 214, 216, 218, 233, 255, 261, 265, 267, 269,
271, 273, 284 of `scripts/build_ga_acceptance_report.py`).

## 4. Independently re-derived claims

1. **Dossier record is a pure function of its sources at HEAD.** `--check`
   passes; the tracked dossier reads BLOCKED with exactly four sorted
   reasons: "1 review obligations are open" (register `open_reviews` = the
   `root_backed_query_profile.contract_text_review` PENDING row), "4 GA
   acceptance criteria are not evidenced" (GA report states 48/2/2), "no
   succession trial has been executed" (items 17–22 all GATED;
   `succession.trials_executed` 0, `thresholds` null), "the release-check
   and v2 product gates are fail-closed" (both stubs NOT_IMPLEMENTED and
   `release_candidate_packaging.release_check_gate` not OPEN). Each reason
   traces to the entry or source the contract's section 3 mapping names.
2. **Item 15 (security review result), EVIDENCED, value
   `PASS_0_P0_0_P1_0_P2_3_P3_5_P4`.** Source: summary
   `threat_coverage.independent_security_review`; the register carries the
   same string as obligation `threat_coverage.independent_security_review`
   in state PASS (reviewer vulcan); the summary note names transcript
   `evidence/review/verdicts/threat_coverage/independent_security_review-43f2f5b.md`,
   which exists (the prior-round cb841a6 transcript also exists). Evidence
   list `[threat-coverage-report, machine-summary]`; the note reads the
   threat-coverage counts (56 threats, 44 exercised, 0 P0/P1 unlocated),
   all of which match the report. Traceable to a recorded verdict.
3. **Item 32 (independent review result), GATED.** Summary
   `finding_register.independent_review` is `PENDING`; the register result
   is FINDING_REGISTER_OPEN. Correct today. The code path that would evidence
   it checks only `== "PASS"` on the summary field (see finding P3-1).
4. **GA 26.9 review criteria.** Ariadne lane: 78 PASS, 0 PENDING, remaining
   rows HISTORICAL_ROUND; Nabu: 72 PASS, 0 PENDING; Vulcan lane clear; the
   two AWAITS_REVIEW criteria are exactly the S20-740 independent review
   (PENDING) and register clearance (OPEN, 1 pending + 3 OTHER + 43
   unclaimed carried findings). Traceable.
5. **BLOCKED derives fail-closed for the four asked conditions.** Open
   obligation: read from the findings entry's `open_reviews`; gated findings
   entry → "openness is unknown"; missing entry → "no … entry to derive
   from" (tests at lines 138, 186, 192). No trial: the six arm entries all
   GATED → blocked regardless of what `succession` claims (test line 171).
   Product gates: `gate_results` keys off the live stub runs dual-sourced
   with the summary hand field; anything but OPEN blocks; FAILED maps to
   FAIL only after no BLOCKED reason remains (test line 298). GA: any
   AWAITS_REVIEW/GATED count blocks (test line 285). `enforce_pass_guard`
   is unreachable by construction and still present. All confirmed by the
   61 passing tests and by reading the code.
6. **Lane rotations of 2026-09-15.** `decision_dossier`: rotated at 6a2eef7
   (the 2026-09-04 `FAIL_2_P0_5_P1_7_P2_3_P3` retained as
   `ariadne_contract_review_initial`, base field carries the 09-13/14 final
   PASS, `_note` records the rotation); Nabu and Vulcan likewise. Register
   classifies each `_initial` row HISTORICAL_ROUND with `superseded_by` a
   same-lane PASS (`ariadne_contract_review`, `nabu_architecture_review`,
   `vulcan_final_review`), which FINDING_REGISTER_V1 section 2 permits
   (initial folds into the final or unmarked review of the same lane).
   `reproducibility_and_independent_conformance`: rotated at f7df74f
   (`vulcan_surface_review` FAIL → `PASS_0_P0_0_P1_0_P2_0_P3`, the FAIL
   retained as `vulcan_surface_review_initial`, note added, the missing
   0bcc9c6 Nabu revision_3 round recorded); register: `_initial` and
   revisions 1–3 HISTORICAL_ROUND superseded by `vulcan_final_review`,
   revisions 4–5 PASS. Both rotations are recorded per the register
   contract and re-derive cleanly (`build_finding_register.py --check`
   PASS). The decision_dossier rows do not appear in
   `unclaimed_carried_findings` (the `P0_P1_P2_P3_CLOSED` form is accepted
   as a closure scope).
7. **Contract text versus the f7df74f derivation.** The contract was not
   touched by f7df74f; its generic section 1 rule ("resolved from tracked
   evidence or explicitly marked gated; the resolver reads the machine
   summary, the tracked reports…") covers deriving items 15 and 32 from
   summary verdict fields. Section 3's per-rule mapping and the code's
   `derive_decision` docstring agree on the four source-read inputs. One
   text gap and one un-recorded change are listed as P4 below.

## 5. Per-item analysis of the f7df74f derivation

**Can a criterion read EVIDENCED without a recorded verdict or checker
result?** In the dossier, no: items 15 and 32 require a recorded PASS
string, and the 23 other EVIDENCED items carry tracked facts. In the GA
report, yes: 18 criteria are constants, and five of those interpolate a
fact into their evidence string without testing it. The simulation shows
"second clean build establishes reproducibility" EVIDENCED against
`SINGLE_HOST_REPRODUCIBLE`, "the source working tree is clean" EVIDENCED
against `false`/`None`, "Rust and independent oracle produce byte-identical
output" EVIDENCED against `FAIL`, "Every non-canonical fixture is rejected"
EVIDENCED at 3 of 25 families, and the artifact-name criterion EVIDENCED
with no attestation. The commit message's "derives every section 26
criterion from recorded evidence; nothing is hard-coded" is therefore
inaccurate for 18 of 52. Because the dossier's rule-1 GA input is only the
pending count, a constant can only under-count pending — the fail-open
direction. No live wrong state exists today (every interpolated value is
the good one), which keeps this at P2 rather than P0/P1.

**Does the GA builder survive the release decision?** No. `final_commit_fixed`
(lines 153–157) reads `attestation.get("commit")` two lines before
`attestation` is assigned (line 159). It is masked today only by
short-circuit evaluation on `release.get("state") == "RELEASE_APPROVED"`.
The first time the operator records `release_decision.state:
RELEASE_APPROVED`, `build_ga_acceptance_report.py` raises
`UnboundLocalError`; the criterion "artifact is built from the final
candidate commit" can never read EVIDENCED. It fails loud, not wrong, and no
unit test covers the GA builder at all.

**Is the GA report kept fresh?** No checker runs
`build_ga_acceptance_report.py --check`; only `make evidence-refresh` writes
it. At HEAD it has drifted (383 → 384 obligations after a809906 added the
Nabu standards PASS row). The dossier's `--check` still passes because it
reads only the state counts, which did not change — but the derivation
chain the round brief asks about (register → GA report → dossier BLOCKED
rule) has an unguarded link, and staleness is in the fail-open direction
(a criterion regressing from EVIDENCED to AWAITS_REVIEW in a fresh
derivation is invisible until someone refreshes).

**Items 15 and 32.** Item 15 evidences on `startswith("PASS")` of the
summary string, while the register (the contract's named finding source)
classifies a self-contradicting `PASS_…_P1_OPEN` as OTHER; the two rules
disagree at the edge. Item 32 evidences on `independent_review == "PASS"`
alone yet (a) lists `REGISTER` in its evidence without reading it — the
exact substitution the contract's revision-5 clarification names ("an
entry that cites a source it never reads") — and (b) its note promises
"over a CLEAR register", which the code does not check (the register
checker enforces PASS-only-at-COMPLETE, so the guarantee is transitive, not
the entry's own). The GA "Vulcan … complete PASS" criterion has the same
shape (simulation: EVIDENCED with `independent_review = PASS` over an OPEN
register).

**"Any open review obligation" (contract section 3 rule 1).** The code reads
`open_reviews` (PENDING rows only). The three live OTHER rows
(`mutation_value_profile.merlin_review`,
`rw075_correction.native_review_r12_2026_09_07`,
`s20_600_frozen_legacy_adapter.review_lane`) and the register `result` are
not read by `derive_decision`, and `unclassified` is not in the findings
entry's value at all. Today the dossier blocks on the one PENDING row and
transitively on the GA "all reviewer findings resolved" criterion; once the
PENDING row closes, only the transitive path (through an unguarded GA
report) keeps OTHER rows blocking. The contract's own principle ("each
rule reads the entry that carries its fact") argues for carrying the
register result and unclassified count in the entry and reading them.

**Section checker at HEAD.** `check_decision_dossier.py` FAILs on
`test-inventory:drift`: a809906 added four unit tests to
`bench/release/tests/test_standards_sbom.py` without refreshing
`evidence/validation/test-inventory.json` (359 → 363 Python tests). Dossier
item 11 cites that inventory, so its `python_tests` fact is stale by four.
This is not the candidate-bound staleness the round brief exempts; it is a
tree-derived record left unrefreshed by a commit in this round. The
finding-register checker fails for the sibling cause (summary mirror 383
vs register 384) — another lane's field, noted for the owner.

**Stale summary mirrors and package row.** The summary's `ga_acceptance`
section still reads states 34 EVIDENCED / 16 AWAITS_REVIEW / 2 GATED (the
pre-f7df74f numbers) against the GA report's 48/2/2 and the dossier reason
"4 GA acceptance criteria are not evidenced". `decision_dossier.license_disposition_blocked`
reads 19 against the dossier's 0 (and the unit test asserting zero).
`docs/WORK_PACKAGES.md` line 68 still describes revision 5 with "19
BLOCKED". None is read by a builder (section 2 declares the mirrors
non-inputs), so the dossier is unaffected; they are record contradictions
an independent reviewer would trip over.

**Contract text.** Section 3 rule 1 enumerates six BLOCKED causes and omits
the single-attesting-host cause that the same section's mapping ("single
attesting host: the reproducibility result entry") and the code (lines
621–626, reason "only one host has attested the candidate") implement.
Section 8's revision-6 paragraph predates f7df74f and does not record that
items 15 and 32 now derive from recorded verdicts. Both editorial.

**`open_p2_rows`.** A HISTORICAL_ROUND row naming P2 (e.g. every rotated
`_initial` FAIL) is neither PASS nor `declares_no_open_p0_p1_p2`, so with
`declared_open_findings.p2 > 0` it is reported as an uncovered P2 row and
forces FAIL where the contract's rule 4 would allow CONDITIONAL_PASS on an
approval naming the live row. Fail-closed direction; precision only.

Non-actionable observations (prose only): the `threat_coverage.independent_security_review`
PASS row carries unclaimed P3/P4 tokens in the register, which will keep
the register OPEN until the section claims or closes them — that is the
finding-register/threat-coverage lanes' matter, and item 15 correctly
records the PASS as the result. The prior Ariadne final's "partial-null
stays EVIDENCED" deviation (item 12's `campaign_duration: null`) is
unchanged and still bounded by the note; not re-raised.

## 6. Findings

Listed in the footer. Four P2, three P3, two P4; no P0 or P1. The dossier
itself reads the right state for the right reasons at HEAD; the actionable
items are the GA builder's derivation and its latent crash, the section
checker being red at the scope SHA, the unguarded GA report, and the
entry-level precision of items 15/32 and the open-obligation rule.

```
VERDICT: REVISE_0_P0_0_P1_4_P2_3_P3_2_P4
SECTION: decision_dossier
FIELD: ariadne_contract_review
SCOPE_SHA: a809906f78f1bfdb9cde8da4c108c4d692dad297
FINDINGS:
[P2] [records] scripts/check_decision_dossier.py:264 - the section checker FAILs at HEAD with `test-inventory:drift`: a809906 added four Python tests without refreshing evidence/validation/test-inventory.json (python_tests 359 tracked vs 363 derived), so dossier item 11 cites a stale source; not the candidate-bound staleness the round exempts
[P2] [derivation] scripts/build_ga_acceptance_report.py:178 - 18 of 52 criteria are literal EVIDENCED constants; five (lines 178, 180, 261, 271, 273) interpolate a fact they never test and read EVIDENCED against SINGLE_HOST_REPRODUCIBLE, working_tree_clean false/None, cross_implementation_agreement FAIL, 3 of 25 families, and "no attestation" (simulated); the f7df74f claim that every criterion is derived does not hold, and constants can only under-count the dossier's rule-1 pending input
[P2] [defect] scripts/build_ga_acceptance_report.py:156 - `final_commit_fixed` reads `attestation.get("commit")` before `attestation` is assigned at line 159; masked by short-circuit today, raises UnboundLocalError the moment release_decision.state reads RELEASE_APPROVED, so the criterion can never reach EVIDENCED and the GA report cannot be built at the decision; no unit test covers the GA builder
[P2] [records] evidence/release/ga-acceptance-report.json:1 - tracked report has drifted at HEAD (evidence text "across 383 obligations" vs 384 derived; report_digest differs) and no checker in `make quick` runs `build_ga_acceptance_report.py --check`, so the dossier's rule-1 GA input can go stale in the fail-open direction unnoticed
[P3] [contract] scripts/build_decision_dossier.py:425 - item 32 lists REGISTER as evidence without reading it (the substitution the contract's revision-5 clarification names) and its note promises "over a CLEAR register" that the code does not check; item 15 (line 293) evidences on `startswith("PASS")` where the register classifies a self-contradicting PASS as OTHER; GA line 280 has the same shape (EVIDENCED with independent_review PASS over an OPEN register)
[P3] [derivation] scripts/build_decision_dossier.py:584 - rule 1 "any open review obligation" reads only PENDING rows; the register `result` and the three live OTHER rows are not carried in the findings entry nor read, so once the PENDING row closes, OTHER rows block only transitively through the unguarded GA report
[P3] [records] machineresearch/sley-2.0/machine-summary.json:1 - summary `ga_acceptance.states` still reads 34/16/2 against the GA report's 48/2/2 and the dossier reason "4 GA acceptance criteria are not evidenced"; `decision_dossier.license_disposition_blocked` 19 against the dossier's 0; docs/WORK_PACKAGES.md:68 still describes revision 5 with "19 BLOCKED"
[P4] [contract] docs/spec/DECISION_DOSSIER_V1.md:146 - rule 1's enumerated BLOCKED causes omit the single-attesting-host cause that the section-3 mapping (line 185) and the code (lines 621-626) implement; section 8's revision-6 paragraph does not record the 2026-09-15 change deriving items 15 and 32 from recorded verdicts
[P4] [derivation] scripts/build_decision_dossier.py:527 - `open_p2_rows` treats HISTORICAL_ROUND rows naming P2 (every rotated `_initial` FAIL) as uncovered open P2 rows, forcing FAIL where rule 4 would admit CONDITIONAL_PASS; fail-closed direction, precision only
SUMMARY: At a809906 the tracked dossier is a pure function of its sources and reads BLOCKED for four exact, entry-traceable reasons (one open obligation, four unevidenced GA criteria, no succession trial, fail-closed gates); items 15 and 32 trace to the recorded Vulcan security PASS and the PENDING S20-740 review; the 2026-09-15 rotations of decision_dossier (6a2eef7) and reproducibility (f7df74f) are recorded per FINDING_REGISTER_V1 with `_initial` FAIL rows HISTORICAL_ROUND under same-lane PASS superseders; 61 dossier tests and the release tests pass. Revision is required because the section checker is red at HEAD (unrefreshed test inventory), the GA builder still hard-codes 18 criteria (five of which read EVIDENCED against contrary evidence in simulation) and crashes on the RELEASE_APPROVED path, the GA report has drifted with no checker guarding it, and items 15/32 and the open-obligation rule stop short of the register facts the contract says an entry must carry.
```
