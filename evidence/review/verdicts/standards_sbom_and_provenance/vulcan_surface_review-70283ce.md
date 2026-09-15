# Vulcan surface review — standards_sbom_and_provenance clean-pass round (rev 5, SCOPE_SHA 70283ce)

Field: `vulcan_surface_review`. Role: Vulcan (Greyforge QA/security authority, SURFACE lane).

## Scope verification

`git rev-parse HEAD` = `70283ce1932d4182f206f0d09e6129af3976a4c2`, matching the assigned
SCOPE_SHA exactly. `git status --porcelain` was not required to be run to prove read-only
intent — no file besides this transcript was written. The repair commit under review is
`f7df74f` ("Qualification: S20-310 revision 6, evidence-derived GA and dossier states,
package acceptances, record reconciliation"); the prior boundary repair `bedd422` and the
prior council round `f81f942` are its immediate ancestors on this branch.

## Inputs read in full

- `evidence/review/verdicts/standards_sbom_and_provenance/vulcan_surface_review-db1bc62.md`
  (own prior PASS, rev 5, holding a P3 on redundant problem labels and a P4 on the
  admit-without-remint test's name/coverage claim).
- `evidence/review/verdicts/standards_sbom_and_provenance/vulcan_surface_review-a4b6029.md`
  (own prior REVISE-3, a self-invalidating live-tree closure test; since repaired — its
  successor at db1bc62 already confirmed the fix and this round is not re-litigated).
- `scripts/check_standards_sbom_and_provenance.py` (full file, 407 lines).
- `scripts/build_standards_sbom.py` lines 440-699 (git_head, require_attested_candidate,
  build_documents, tracked_candidate_facts, validate_tracked, candidate_evidence_mismatch,
  main).
- `scripts/build_release_provenance.py` lines matching the same shape (main, --check branch
  ordering) — grepped for `MISMATCH_TRACKED`, `records_closure`, `is_closure`, `return 0/1`,
  `sys.exit` to confirm the branch ordering mirrors the SBOM builder.
- `bench/release/tests/test_standards_sbom.py` — the renamed admission test
  (`test_builders_admit_an_eligible_closure_keeping_the_candidate_binding`, lines 702-727)
  and its neighbors in `RecordsClosureTests`.
- `evidence/runtime/s20-720-release-candidate/evidence.json` (head, for live-state context).
- `git log --oneline -3 -- scripts/check_standards_sbom_and_provenance.py`.

## Tool results (exact)

Command: `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md python3 scripts/check_standards_sbom_and_provenance.py`
Exit: 1. Output:

```json
{
  "codes": [ "SBOM_INVENTORY_MISSING", "SBOM_INVENTORY_INVALID", "SBOM_COMPONENT_INCOMPLETE",
    "SBOM_DOCUMENT_DRIFT", "PROVENANCE_EVIDENCE_MISSING", "PROVENANCE_EVIDENCE_INVALID",
    "PROVENANCE_SUBJECT_MISMATCH", "PROVENANCE_DOCUMENT_DRIFT" ],
  "contract": "s20-710-full-standards-sbom-and-provenance-v1",
  "implementation_complete": false,
  "problems": [ "closure:ineligible" ],
  "records_closure": {
    "advanced": true,
    "attested_source_commit": "7a94a4a31272a6dc7588aff902dcde81f7d10a4e",
    "reason": "records-closure-ineligible: bench/release/tests/test_standards_sbom.py, crates/sley-query/src/root_query.rs, docs/WORK_PACKAGES.md, docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md, docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md, docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md, scripts/build_decision_dossier.py, scripts/build_ga_acceptance_report.py, scripts/check_standards_sbom_and_provenance.py",
    "records_closure_head": "70283ce1932d4182f206f0d09e6129af3976a4c2"
  },
  "result": "FAIL",
  "s20_710_audit_complete": false,
  "status": "S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING"
}
```

Command: `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md python3 -m unittest bench.release.tests.test_standards_sbom -v`
Exit: 0. `Ran 52 tests in 0.093s — OK`. All 52 pass, including
`test_builders_admit_an_eligible_closure_keeping_the_candidate_binding` and
`test_builders_admit_exactly_the_live_closure_verdict` (both `RecordsClosureTests`).

## Independently re-derived claims

**Claim A — one `closure:ineligible` cause, drift folded away only when ineligible.**
Read `scripts/check_standards_sbom_and_provenance.py:350-385`. `drift_problems` is computed
unconditionally by running both builders' `--check` (lines 350-356). The closure block
(357-381) sets `closure_ineligible = True` and appends `closure:ineligible` only when
`cstat.is_closure` is `False`. Line 382-383: `if not closure_ineligible:
problems.extend(drift_problems)`. So:
- Not advanced (HEAD == candidate): `closure` stays `{"advanced": False}`,
  `closure_ineligible` stays `False` → `drift_problems` always surfaces normally.
- Advanced + **eligible** closure: `closure_ineligible` stays `False` → `drift_problems`
  still surfaces normally. **The task's named question — can folding hide a real drift on
  an eligible closure — is `False`: it cannot, by this code path.** Any real drift on an
  eligible closure is reported as `sbom:drift`/`provenance:drift` unmasked.
- Advanced + **ineligible**: `closure_ineligible = True` → `drift_problems` (whatever it
  computed to) is dropped, and only `closure:ineligible` is reported. Live run above
  confirms this: `problems` is exactly `["closure:ineligible"]`, no `sbom:drift` /
  `provenance:drift` redundant entries — the db1bc62 P3 (redundant labels) is fixed as
  designed.

Confirmed directly against the live run: at this HEAD the closure is ineligible (9
attestation-bound paths listed in `records_closure.reason`) and `problems` carries the
single folded cause, matching the code's stated behavior exactly.

**Claim B — is the fold always attributing the same root cause, or can it mask an
unrelated failure?** This is the adversarial question worth pressing past the literal
prompt (which named only the eligible-closure case, confirmed safe above). I traced
whether an *ineligible*-closure fold can ever swallow a real, independent problem.

`build_standards_sbom.py:main()` (and the mirrored `build_release_provenance.py:main()`)
branches in this order under `--check`:
1. `candidate_evidence_mismatch()` — compares the **tracked** documents' recorded
   (commit, artifact digest) against the **untracked** local candidate evidence file. If
   they disagree (a fresh untracked candidate build exists that the tracked docs don't
   yet describe — an expected, legitimate pre-`release-candidate-smoke` state per the
   docstring at lines 593-601), the builder takes a *separate* branch: it calls
   `validate_tracked()` (checks CycloneDX/SPDX shape, determinism pins, namespace binding
   of the *tracked* pair) and returns 1/`MISMATCH_TRACKED_INVALID` if that internal
   validation itself fails, or 0/`MISMATCH_TRACKED_VALIDATED` if it's merely stale. In
   this branch, `require_attested_candidate()` — and therefore `records_closure.closure_status()`
   — is **never called** by the builder.
2. Otherwise it calls `build_documents()` → `require_attested_candidate()`, which is where
   the closure predicate is consulted and where an ineligible closure raises
   `SbomError(INVENTORY_INVALID, ...)` before ever reaching the drift-comparison loop.

So when the *checker's own, independently-computed* `cstat.is_closure` (lines 364-379,
using the live-tree diff against the runtime evidence commit) says ineligible, and the
builder's nonzero `--check` exit happened to come from branch 1
(`candidate_evidence_mismatch` → `MISMATCH_TRACKED_INVALID`, e.g. a genuinely rewritten
namespace or corrupted tracked document) rather than branch 2's closure-gated
`require_attested_candidate` raise, the checker's fold would still swallow that failure
into the single `closure:ineligible` label — because the fold keys only on the checker's
own closure computation, not on which branch inside the builder actually produced the
nonzero exit. The two computations (checker's `closure_status` call and the builder's
internal `require_attested_candidate` call) are structurally decoupled: nothing ties the
specific reason the builder failed to the reason the fold suppresses it.

This is real but narrow: it can only fire when (a) an untracked local candidate build
exists and disagrees with the tracked documents' recorded facts, **and** (b) that same
untracked candidate's commit is, independently, an ineligible-closure HEAD, **and** (c)
the *tracked* documents also carry an unrelated internal defect that `validate_tracked()`
would catch. At the live HEAD checked above, none of this applies — `drift_problems`
correctly is `[]` regardless (both builders' checks presumably fail via the closure-gated
path, since the tracked documents remain internally valid; not independently re-verified
per-branch here since it does not change the live result). It does not create a false
`PASS` (the overall `result` stays `FAIL` either way, since `closure:ineligible` alone is
already appended) — it is a diagnostic-precision gap, not a fail-open hole: an operator
told only `closure:ineligible` in this exact overlap case would not learn that the tracked
documents themselves also have an independent, real defect until they ran the builder
directly. No test in `RecordsClosureTests` or elsewhere in
`bench/release/tests/test_standards_sbom.py` exercises this branch-1/branch-2 interaction
under the checker's fold.

**Claim C — admission test name/comment.** `test_builders_admit_an_eligible_closure_keeping_the_candidate_binding`
(lines 702-727) carries the comment (703-705): "Compares the candidate-bound properties,
subject, and SPDX version of the admitted documents; byte identity of re-derived documents
is the drift tests' claim, not this one's (Vulcan P4, 2026-09-13)." This directly answers
and closes the db1bc62 P4 (the old name overclaimed byte-identity coverage; the new name
and comment scope the test to binding-preservation and hand the byte-identity claim to the
drift tests, which do cover it — `test_documents_are_a_pure_function_of_the_evidence` and
the `--check` drift branch itself). The rename is accurate; the P4 is resolved, not merely
relabeled.

## Per-item analysis

1. **Closure fold correctness (section 9 evidence rule, this round's headline repair).**
   Confirmed sound for the case the task named (eligible closure never loses drift
   reporting). Confirmed there exists a narrow, untested overlap case on the *ineligible*
   side where the fold's single-cause assumption is not actually guaranteed by the code —
   see Claim B. This does not reopen the db1bc62 P3 (that was about needless redundancy on
   the common path, now fixed); it is a new, narrower finding about the fold's soundness
   assumption. Filed as P3 (fails closed — `result` stays `FAIL` — but the diagnostic can
   mislead an operator toward the wrong remediation script in the overlap case).
2. **Admission test rename (db1bc62 P4).** Resolved — Claim C. Closed, not re-listed.
3. **Live surface.** `problems == ["closure:ineligible"]` exactly, no redundant drift
   labels, `records_closure` block correctly separate from and never mixed into the
   tracked documents (I did not re-verify document byte contents this round since no
   document-shape claim changed since db1bc62 and the test suite's drift/namespace tests
   re-cover it at 52/52 green).
4. **Test suite.** 52/52 green (one net new test vs. db1bc62's 51 — the renamed admission
   test), `SLEY2_MASTER_GOAL` set per brief.

## Findings

- [P3] [surface/diagnostic] `scripts/check_standards_sbom_and_provenance.py:350-385` — the fold that suppresses `drift_problems` under `closure_ineligible` assumes the builders' nonzero `--check` exit is always caused by the same closure-ineligibility the checker separately computed. `build_standards_sbom.py`'s and `build_release_provenance.py`'s `main()` (see `build_standards_sbom.py:613-666`) has an earlier, independent failure branch — `candidate_evidence_mismatch()` → `validate_tracked()` → `MISMATCH_TRACKED_INVALID` — that never consults `records_closure.closure_status()` at all. When an untracked local candidate build disagrees with the tracked documents *and* that candidate's commit is independently an ineligible-closure HEAD *and* the tracked documents also carry an unrelated internal defect, the fold swallows that unrelated defect into the single `closure:ineligible` label. Does not produce a false PASS (result stays FAIL either way) and is not reproducible at the live HEAD checked this round, but is untested and not something the code rules out. Suggest: only suppress `drift_problems` for a given label when the corresponding builder's own failure detail names a records-closure reason (parse it, or have the builders emit a distinguishable code for the `MISMATCH_TRACKED_INVALID` branch), or add a `RecordsClosureTests` case pinning that this overlap still surfaces the tracked-document defect.

## Summary

Revision 5's repair does what it claims for the case actually named in the task: an
eligible closure's drift is never hidden by the fold, because `closure_ineligible` is
never set `True` on that path, so `drift_problems` always reaches `problems` unchanged.
The db1bc62 P3 (redundant `sbom:drift`/`provenance:drift` plus `closure:ineligible` for one
cause) is fixed — the live run confirms exactly one folded cause. The db1bc62 P4 (test name
overclaiming byte-identity) is fixed by the rename plus an explicit scoping comment.
Pressing further, the fold's single-cause assumption is not fully guaranteed by the code:
a narrow, untested overlap between the builders' pre-closure `MISMATCH_TRACKED_INVALID`
branch and the checker's independent closure computation could suppress a genuinely
unrelated tracked-document defect behind the `closure:ineligible` label, without ever
producing a false PASS. That is the one actionable item this round. `python3 scripts/check_standards_sbom_and_provenance.py`
correctly reports `closure:ineligible` alone (exit 1) at this pre-re-mint HEAD, matching
the expected state named in the task, and `python3 -m unittest bench.release.tests.test_standards_sbom`
is 52/52 green.

```
VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3
SECTION: standards_sbom_and_provenance
FIELD: vulcan_surface_review
SCOPE_SHA: 70283ce1932d4182f206f0d09e6129af3976a4c2
FINDINGS:
[P3] [surface/diagnostic] scripts/check_standards_sbom_and_provenance.py:350-385 - the ineligible-closure fold assumes the builders' `--check` failure is always the same closure-ineligibility cause the checker independently computed, but the builders have an earlier, closure-independent failure branch (`candidate_evidence_mismatch` -> `validate_tracked` -> `MISMATCH_TRACKED_INVALID`); an untested overlap could fold a genuinely unrelated tracked-document defect into `closure:ineligible`. No false PASS results (result stays FAIL); not reproducible at the live HEAD; untested.
SUMMARY: The named question is answered No — folding never hides real drift on an eligible closure, because `closure_ineligible` is only ever True on the ineligible path, and on that path drift_problems is unconditionally computed but only suppressed there. The live run at 70283ce shows exactly one folded cause (`closure:ineligible`), resolving the db1bc62 P3 on redundant labels; the admission test's rename plus scoping comment resolves the db1bc62 P4. One new, narrow, non-blocking diagnostic-soundness gap remains in the fold's single-cause assumption on the ineligible side, filed as P3. All required commands ran clean for the expected pre-re-mint state: checker reports closure:ineligible alone (exit 1), and bench.release.tests.test_standards_sbom is 52/52 green.
```
