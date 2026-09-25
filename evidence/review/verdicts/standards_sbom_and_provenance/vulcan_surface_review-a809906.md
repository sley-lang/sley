# Vulcan surface review — standards_sbom_and_provenance delta round (rev 5, SCOPE_SHA a809906)

Field: `vulcan_surface_review`. Role: Vulcan (Greyforge QA/security authority, SURFACE lane).
Round: delta review of the a809906 repair of this lane's own P3 at 70283ce (the
ineligible-closure fold assumed every builder `--check` failure is the closure refusal),
plus everything the repair commit touched.

## Scope verification

`git rev-parse HEAD` = `a809906f78f1bfdb9cde8da4c108c4d692dad297`, matching the assigned
SCOPE_SHA exactly. `git status --short | wc -l` = `0` before and after the review; the only
file written is this transcript. No builder was run in write mode, nothing was staged, no
remote was touched. The scratch simulation described below lives in the session scratchpad,
outside the tree.

## Inputs read in full

- `/tmp/claude-sley2/review-brief.md` and `/tmp/claude-sley2/round-a809906-sections.md`.
- `evidence/review/verdicts/standards_sbom_and_provenance/vulcan_surface_review-70283ce.md`
  (own prior REVISE, 1 P3; the finding being repaired).
- `git show a809906` in full: `scripts/check_standards_sbom_and_provenance.py` (+51/-5),
  `bench/release/tests/test_standards_sbom.py` (+48), `evidence/release/decision-dossier.json`,
  `evidence/release/ga-acceptance-report.json`, `evidence/review/finding-register.json`,
  `machineresearch/sley-2.0/machine-summary.json`, and the new Nabu transcript
  `nabu_architecture_review-70283ce.md` (stat only; not this lane's field).
- `scripts/check_standards_sbom_and_provenance.py` at HEAD, full file (447 lines):
  `builder_refusal_label` (130-152), `fold_closure_refusals` (155-165), the builder loop
  (388-395), the closure block (396-424).
- `scripts/build_standards_sbom.py` 440-530 (`git_head`, `require_attested_candidate`,
  `build_documents`) and 560-699 (`validate_tracked` tail, `candidate_evidence_mismatch`,
  `main` with every `--check` print site); `scripts/build_release_provenance.py` 195-240
  (`build_statement` closure gate) plus a grep of every print/return/raise site in `main`.
- `scripts/records_closure.py` 55-95 (`is_closure`, `reason`: the five `records-closure-*`
  markers).
- `bench/release/tests/test_standards_sbom.py` 770-825 (`CheckerFoldTests`, four tests).
- `machineresearch/sley-2.0/machine-summary.json` section `standards_sbom_and_provenance`
  (every `vulcan_*` field, `status`, `contract_revision`);
  `evidence/release/ga-acceptance-report.json` Vulcan criterion (lines 301-307).
- `git diff --stat 70283ce..a809906 -- scripts bench crates docs` (code scope of the delta).

## Tool results (exact)

1. `SLEY2_MASTER_GOAL=... python3 scripts/check_standards_sbom_and_provenance.py` — exit 1.
   `"problems": ["closure:ineligible"]` exactly; `"result": "FAIL"`;
   `"status": "S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING"`;
   `records_closure.advanced = true`, `attested_source_commit = 7a94a4a31272a6dc7588aff902dcde81f7d10a4e`,
   `records_closure_head = a809906f78f1bfdb9cde8da4c108c4d692dad297`,
   `reason = "records-closure-ineligible: RESUME.md, bench/release/tests/test_standards_sbom.py,
   crates/sley-query/src/root_query.rs, crates/sley-repo/tests/zjx_readiness_witness.rs,
   docs/WORK_PACKAGES.md, docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md,
   docs/audits/S20_ZJX_TRANSPORT_READINESS_AMENDMENT.md, docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md,
   docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md, docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md,
   docs/status/HANDOFF-2026-09-15-QUALIFICATION.md, scripts/build_decision_dossier.py,
   scripts/build_ga_acceptance_report.py, scripts/check_standards_sbom_and_provenance.py"`.
   This is the expected pre-re-mint posture named in the round brief.
2. `python3 scripts/build_standards_sbom.py --check` — exit 1, stdout one JSON record:
   `code 74001`, `name INVENTORY_INVALID`, `detail "candidate commit 7a94a4a… is not HEAD;
   records-closure-ineligible: <same 14 paths>; rebuild the candidate on this tree before
   deriving SBOMs"`.
   `python3 scripts/build_release_provenance.py --check` — exit 1, `code 74005`,
   `name EVIDENCE_INVALID`, same `records-closure-ineligible:` detail with the provenance
   suffix. Both builders therefore fail through the closure-gated branch at this HEAD, which is
   exactly the case the fold is meant to absorb.
3. `python3 -m unittest bench.release.tests.test_standards_sbom -v` — exit 0,
   `Ran 56 tests … OK` (52 at 70283ce + the 4 new `CheckerFoldTests`).
   `python3 -m unittest discover -s bench/release/tests -t .` (the invocation the checker
   itself makes) — exit 0, `Ran 110 tests … OK`.
4. Read-only simulation (scratchpad script, imports the real checker module, monkeypatches
   only `chk.run` to synthesize the two builders' stdout/exit while every other subprocess —
   `git rev-parse`, `records_closure.closure_status`, `gate_status.py` — runs for real on the
   live ineligible HEAD; the in-checker unittest call is stubbed to 0 to keep each scenario
   short). Results, `problems` list per scenario:
   - A both builders emit the live closure refusal, exit 1 → `['closure:ineligible']`
   - B **the 70283ce P3 scenario**: sbom `state=MISMATCH_TRACKED_INVALID`, provenance closure
     refusal → `['closure:ineligible', 'sbom:tracked-invalid']`
   - C both `MISMATCH_TRACKED_INVALID` → `['closure:ineligible', 'sbom:tracked-invalid', 'provenance:tracked-invalid']`
   - D sbom `DOCUMENT_DRIFT` record under the ineligible closure → `['closure:ineligible', 'sbom:drift']`
   - E empty stdout, exit 1 (builder crash) → `['closure:ineligible', 'sbom:drift']`
   - F traceback text on stdout, exit 1 → `['closure:ineligible', 'sbom:drift']`
   - G JSON list instead of dict, exit 1 → `['closure:ineligible', 'sbom:drift']`
   - H closure-refusal text but exit 0 → `['closure:ineligible']` (a zero exit is never a problem;
     the fold has nothing to act on and `closure:ineligible` still stands)
   - I `records-closure-ineligible` placed in `state` rather than `detail`, exit 1 →
     `['closure:ineligible', 'sbom:drift']` (only the `detail` key is honored; anything else
     surfaces)
   Every scenario exits 1. No scenario produced a PASS or removed a non-closure failure.

## Independently re-derived claims

**Claim 1 — the fold can only remove a label the builder itself attributed to the closure.**
`fold_closure_refusals` (155-165) drops exactly the problems ending in `:closure-refusal`, and
`builder_refusal_label` (130-152) assigns that suffix only when `"records-closure-" in
str(record.get("detail", ""))`. In both builders the substring enters a `detail` at exactly one
site each — `build_standards_sbom.py:483` and `build_release_provenance.py:219`, both
`f"{status.reason}"` inside the `if not status.is_closure:` branch of the HEAD-advanced gate
(`grep -n 'records-closure\|\.reason'` over both builders: no other detail carries it; the
remaining mentions are docstring/comment text). `records_closure.reason` yields
`records-closure-eligible` only when `is_closure` is true, and that branch never raises, so an
admitted derivation cannot produce a `:closure-refusal` label. The other 36 `raise` sites
carry fixed strings, an OSError text for a fixed path, or the candidate commit — none can
contain `records-closure-`. The classification is therefore sound against the builders as
written, not merely against the test fixtures.

**Claim 2 — no false-PASS path exists through the repair.** The fold executes only when
`closure_ineligible` is true (161-165), and the same branch that sets `closure_ineligible`
appends `closure:ineligible` to `problems` (419-421) before the fold runs (424). `problems`
is therefore non-empty whenever anything is folded; `result` is `FAIL` and the exit is 1.
When `closure_ineligible` is false (HEAD equals the candidate, eligible closure, or
`closure:unverifiable` after an OSError/JSONDecodeError on the evidence file), nothing is
removed: `:closure-refusal` is renamed to `:drift` and every other label passes through
(`test_eligible_closure_folds_nothing`). The eligible-closure path thus still surfaces every
builder failure, and on that path the builders proceed past the closure gate to the document
comparison, so a real `DOCUMENT_DRIFT` reaches the checker as `sbom:drift`/`provenance:drift`.

**Claim 3 — malformed or empty stdout cannot be misclassified in the hiding direction.**
`builder_refusal_label` degrades to `{}` on empty stdout, on a `JSONDecodeError`, and on a
non-dict JSON value, and `{}` yields `:drift`, which is never folded. A nonzero exit with no
parseable record therefore always surfaces (scenarios E, F, G; pinned by the two `""` and
`"not json"` assertions in `test_tracked_invalid_and_drift_keep_their_own_labels`). Only the
`detail` key is consulted for the fold; a closure marker placed anywhere else surfaces
(scenario I). A zero exit is never appended regardless of stdout (scenario H), matching the
pre-repair contract that the builders' exit code is the failure signal.

**Claim 4 — the tests pin the behavior that matters.** The four `CheckerFoldTests` cover:
detail-keyed closure refusal → `:closure-refusal`; `MISMATCH_TRACKED_INVALID` → `:tracked-invalid`;
`DOCUMENT_DRIFT`, empty, and non-JSON → `:drift`; the ineligible fold retaining
`tracked-invalid` and `drift` while dropping both closure refusals; the eligible path renaming
and retaining everything. The wiring between those functions and `main()` is two statements
(395 and 424) and is exercised end to end by the live checker run (scenario A shape) and by
my simulation of the P3 scenario (B). The pure-function tests are the right level for the
decision logic; an integration test of the wiring would add little beyond what the live run
already demonstrates on every checker invocation.

**Claim 5 — records touched by the commit are honest.** The finding register adds a
`vulcan_surface_review_revision_5` row (`REVISE_0_P0_0_P1_0_P2_1_P3`, `HISTORICAL_ROUND`,
`superseded_by: vulcan_surface_review`), the machine summary records the same form with a
note naming the transcript, the defect, the repair, and that "the delta verdict replaces"
the held base field; the base field `vulcan_surface_review` stays at the revision-4 form
`PASS_0_P0_0_P1_0_P2_0_P3_1_P4` with a note disclosing the held state. The GA report's
Vulcan criterion remains `AWAITS_REVIEW` and its Ariadne/Nabu counts changed only by the
Nabu 70283ce PASS row; the dossier's register counters (obligations 383→384, HISTORICAL_ROUND
103→104, P3 mentions 203→204) match the one added row. Package status stays
`REVIEW_PENDING`, so the checker's `machine-summary:<review>` PASS requirement is not in
force and no derived artifact treats the held base field as a fresh acceptance. The
convention of holding a base field at the last final verdict while a repaired round sits as
`HISTORICAL_ROUND` pending its delta lane is disclosed in-line; this transcript now supplies
that delta verdict.

## Per-item analysis

1. **The 70283ce P3 (fold single-cause assumption).** Closed. Scenario B reproduces the exact
   overlap described in the finding — an ineligible HEAD plus a `MISMATCH_TRACKED_INVALID`
   builder exit — and the checker now reports `closure:ineligible` **and**
   `sbom:tracked-invalid`. The keying is on the builder's own `records-closure-` detail, which
   is precisely the coupling the finding asked for.
2. **Can any builder failure be hidden?** No. Only `:closure-refusal` is ever dropped, only
   under an already-reported `closure:ineligible`, and the label is reachable only from the
   builders' closure-gated refusal (Claim 1).
3. **Malformed / empty stdout direction.** Always toward surfacing (Claim 3).
4. **Eligible-closure drift.** Unchanged and intact (Claim 2).
5. **Tests.** 56/56 and the checker's own 110/110 discovery are green; the four new tests pin
   the classification and the fold in both closure states (Claim 4).
6. **Records.** Consistent and disclosed (Claim 5).

Non-actionable observations (recorded for the owner, no finding): (a) the `:drift` default
label also names non-drift builder codes that are not closure refusals or tracked-invalid
states (for example `INVENTORY_MISSING`, a non-PASS candidate record, or a missing
attestation); that is the pre-repair vocabulary, never folded, and the builder's own JSON
record carries the precise code, so it is a naming choice rather than a defect. (b) The
`builder_refusal_label` docstring's "three failure branches" under-counts the generic
`SbomError`/`ProvenanceError` catch-all as one branch; the behavior is correct for all of
them. (c) On the non-ineligible path a builder closure refusal is renamed `:drift`; that case
is reachable only if the builder's and the checker's `closure_status` disagree (a HEAD moving
between the two subprocesses), it still fails, and a re-run resolves it. (d) The
`MISMATCH_TRACKED_VALIDATED` zero-exit posture (tracked documents describing a previous
candidate while the untracked evidence describes a newer one) is contract section 5
behavior untouched by this commit and was reviewed in prior rounds; not re-litigated here.

## Findings

None actionable. The P3 filed at 70283ce is resolved by a809906 without opening a
false-PASS path.

## Summary

The repair keys the ineligible-closure fold on the builders' own `records-closure-*` detail,
which enters a builder `detail` at exactly one closure-gated site per builder; every other
failure — `MISMATCH_TRACKED_INVALID`, `DOCUMENT_DRIFT`, empty, malformed, or non-dict
stdout, and any other error code — keeps a label the fold never removes, and the fold runs
only after `closure:ineligible` has already been appended, so no folded run can reach PASS.
The eligible-closure path removes nothing. The live checker at this HEAD reports exactly
`closure:ineligible` (exit 1), both builders' `--check` fail through the closure gate with a
`records-closure-ineligible` detail, the module's tests are 56/56 and the checker's own
discovery 110/110, and a read-only simulation of the original P3 overlap on this HEAD
surfaces `sbom:tracked-invalid` beside `closure:ineligible`. Records touched by the commit
disclose the held base field and the pending delta accurately.

```
VERDICT: PASS
SECTION: standards_sbom_and_provenance
FIELD: vulcan_surface_review
SCOPE_SHA: a809906f78f1bfdb9cde8da4c108c4d692dad297
FINDINGS:
SUMMARY: The a809906 repair closes the 70283ce P3: the fold now drops only a builder failure whose own `detail` names a `records-closure-*` reason, that substring enters a builder detail at exactly one closure-gated refusal site per builder, `MISMATCH_TRACKED_INVALID` and `DOCUMENT_DRIFT` keep their own labels, empty/malformed/non-dict stdout defaults to the never-folded `:drift`, and the fold runs only after `closure:ineligible` is already recorded so no folded run can PASS; the eligible-closure path removes nothing. Live checker at this HEAD reports exactly `closure:ineligible` (exit 1) as expected pre-re-mint, both builders' `--check` fail via the closure gate, tests are 56/56 (module) and 110/110 (checker discovery), a read-only simulation of the original overlap surfaces `sbom:tracked-invalid` beside `closure:ineligible`, and the touched records disclose the held base field honestly. Nothing remains actionable.
```
