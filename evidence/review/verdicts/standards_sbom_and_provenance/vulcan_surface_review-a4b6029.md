# Vulcan surface review — standards_sbom_and_provenance records-closure amendment (rev 5, SCOPE_SHA a4b6029)

Read-only review; HEAD verified equal to SCOPE_SHA at dispatch (`git rev-parse HEAD` =
`a4b60294e4dd75133fa08f92bf16022fb05bb807`). No files edited except this verdict.
Scope: the observable surface of the amendment — spec Status rev 5 + sections 4/5,
`scripts/records_closure.py`, builder/checker diffs, the 7 closure tests, and live runs
of `build_standards_sbom.py --check`, `build_release_provenance.py --check`,
`check_standards_sbom_and_provenance.py`, and the full 105-test suite.

**Live surface (executed 2026-09-13, all outputs captured).**
- Candidate: `36f0e1f99bb7b09c407e870141ca18f109dfaf8a`, result PASS, exactly 1 clean
REPRODUCIBLE attestation in the tracked report. HEAD: `a4b6029` (advanced).
- `closure_status(36f0e1f)`: `is_closure=False`,
`reason=records-closure-ineligible: Makefile, bench/…(4), conformance/…(3),
crates/sley-query/src/root_query.rs, docs/spec/…(2), scripts/…(7)` — 18 ineligible of
27 changed tracked paths; 9 eligible records paths (8 under `evidence/`:
lint-report, decision-dossier, provenance, reproducibility-report, both SBOMs,
finding-register, secret-scan; plus `machineresearch/sley-2.0/machine-summary.json`);
`bound_changed=[]` (T52 inventory untouched).
- SBOM `--check` → exit 1, `74001 SBOM_INVENTORY_INVALID`, detail names candidate,
`records-closure-ineligible` list, and the rebuild command. Provenance `--check` →
exit 1, `74005 PROVENANCE_EVIDENCE_INVALID`, same shape. Refusal codes reused
correctly with records-closure reasons; reconciling command named in both.
- Checker → exit 1, `result FAIL`, `status
S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING`,
`problems=[sbom:drift, provenance:drift, closure:ineligible, release-tests:fail]`,
`records_closure={advanced:true, attested_source_commit:36f0e1f…,
records_closure_head:a4b6029…, reason:records-closure-ineligible:…}`.
Closure HEAD recorded in checker output only: 0 occurrences of `a4b6029` in
cyclonedx-1.6.json, spdx-2.3.json, provenance.json; all three bind `36f0e1f`.
- Suite: `Ran 105 tests ... FAILED (failures=1)`. The single failure is the new
`test_live_tree_is_a_records_closure_of_the_attested_candidate`, asserting
`is_closure` on the live tree with the live reason as message; it fails because the
amendment commit itself carries the 18 attestation-bound paths above. The other 6
closure tests pass: attestation-bound ineligible, bound-input changed, unverifiable
git, admit-eligible-without-remint (properties/subject equality, SPDX version pin),
refuse-ineligible (both builders, exact codes 74001/74005 + reason substring),
refuse-bound-change (both builders).

**Refusal pinning: yes, with one self-invalidating test.** The 74001/74005 refusals are
pinned at unit level (closure predicate reasons), builder level (stubbed eligible /
ineligible / bound-changed verdicts, exact codes, reason substrings), and live level
(real refusal outputs above). Hand-mint resistance stays pinned by the pre-existing
rewritten-namespace / rewritten-subject tests plus drift codes. What is not pinned
correctly is the live-tree integration test: it encodes "HEAD is a closure" as an
invariant of the tree, but the contract promises no such invariant at any commit, and
the very commit shipping the test falsifies it. Result: the suite is red at HEAD, the
checker carries `release-tests:fail`, and the red baseline masks genuine regressions —
the suite cannot serve as a council signal until fixed. This fails closed (nothing
passes open; every gate is red for the right underlying reason too), so it is a
surface-hygiene defect, not a soundness hole — but the surface is what Council reads,
and a permanently red suite at the shipping commit is exactly what this lane must
reject.

**Narrowness from the surface.** No new commands, flags, codes, or document fields;
refusal shapes unchanged apart from the appended reason; no timestamps, hostnames, or
paths leak into documents (checker pins); verdict vocabulary is stable
(`records-closure-eligible/ineligible/bound-changed/unverifiable/not-advanced`).

**Honest notes.** The current redness (builders refuse, checker FAIL, 104/105) is the
designed posture on a non-closure HEAD, not a regression — except the one test, which
is red by construction flaw rather than by design. Machine summary correctly keeps
`s20_710_pre_release_audit.standards_sbom=false`, `release_provenance=false`, and the
9 standards rows open; this review informs but does not close the Council decision,
and closing any row on a closure HEAD still needs the fresh Council review of this
model per the Status line.

**FINDINGS:**

- [P1] `bench/release/tests/test_standards_sbom.py:624-632` - `test_live_tree_is_a_records_closure_of_the_attested_candidate` fails at its own shipping commit: it asserts live-tree closure, but `a4b6029` is a non-closure (18 attestation-bound paths from the amendment itself). Suite 104/105 red at HEAD; `release-tests:fail` in checker problems. Fix before council use: stub `git_head`/diff fixture or invert to assert the expected non-closure refusal (codes + reason) on this tree, keeping a records-only fixture for the admission path (already covered by `test_builders_admit_an_eligible_closure_without_remint`). Fails closed, no open gate, hence P1 for signal corruption not P0.
- [P3] `scripts/check_standards_sbom_and_provenance.py:353-373` - on a non-closure HEAD the checker reports both `sbom:drift`/`provenance:drift` (builder nonzero relabeled) and `closure:ineligible` for one underlying cause; the problem list is accurate but redundant, and an operator must read `records_closure.reason` to find the true cause. Consider surfacing the closure reason as the primary problem. Cosmetic.
- [P4] `bench/release/tests/test_standards_sbom.py:661-683` - the admit-without-remint test compares properties/subject/SPDX version rather than full byte-identity of both documents against tracked bytes; byte-identity is covered separately by drift tests, so coverage is complete in aggregate, but the test name overclaims slightly. Informational.

**SUMMARY:** The amendment's observable behavior is exactly the designed fail-closed
posture: precise refusals with reused codes and records-closure reasons, closure HEAD
kept out of the documents and present in checker output, bound inputs double-covered,
and 6 of 7 new tests pinning the right verdicts. It cannot pass surface while the
suite ships red at HEAD by a test-construction flaw: fix the live-tree test to assert
state rather than assume it, re-run to 105/105 on a records-only HEAD (or the
inverted assertion here), then the surface is green-by-design on closures and
red-by-design otherwise.

VERDICT: REVISE-3 / SECTION: standards_sbom_and_provenance / FIELD: vulcan_surface_review / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS: 3
