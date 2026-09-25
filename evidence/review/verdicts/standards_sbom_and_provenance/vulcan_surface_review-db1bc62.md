# Vulcan surface review — standards_sbom_and_provenance records-closure repair (rev 5, SCOPE_SHA db1bc62)

Read-only review; HEAD verified equal to SCOPE_SHA at dispatch (`git rev-parse HEAD` =
`db1bc623d01e838d49c153feb0be05a7502b8794`, `git status --porcelain` empty).
No files edited except this verdict.
Scope: the repaired rev-5 records-closure surface — spec Status rev 5 + section 5,
`scripts/records_closure.py` (`d255516a…`), builder/checker behavior, the 7
closure tests (repaired coupling test), and live runs of
`build_standards_sbom.py --check`, `build_release_provenance.py --check`,
`check_standards_sbom_and_provenance.py`, and the full 105-test suite.

**Live surface (executed 2026-09-13, all outputs captured).**
- Candidate: `36f0e1f99bb7b09c407e870141ca18f109dfaf8a`. HEAD: `db1bc62` (advanced).
- `closure_status(36f0e1f)`: `is_closure=False`, 43 changed tracked paths, 21
ineligible (Makefile, bench/…, conformance/…, crates/sley-query, docs/spec+WORK_PACKAGES,
scripts/… incl. the closure module itself), `bound_changed=[]` (T52 inventory
untouched). (Delta vs a4b6029's 27/18 is the db1bc62 boundary-verdicts commit
itself; still attestation-bound, still refuses — correct.)
- SBOM `--check` → exit 1, `74001 SBOM_INVENTORY_INVALID`, detail names candidate,
`records-closure-ineligible` list, and the rebuild command. Provenance `--check` →
exit 1, `74005 PROVENANCE_EVIDENCE_INVALID`, same shape. Refusal codes reused
correctly with records-closure reasons; reconciling command named in both.
- Checker → exit 1, `result FAIL`, `status
S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING`,
`problems=[sbom:drift, provenance:drift, closure:ineligible]` (note:
`release-tests:fail` is GONE — the P1 repair effect),
`records_closure={advanced:true, attested_source_commit:36f0e1f…,
records_closure_head:db1bc62…, reason:records-closure-ineligible:…}`.
Closure HEAD recorded in checker output only: 0 occurrences of `db1bc62` in
cyclonedx-1.6.json, spdx-2.3.json, provenance.json; all three stay
candidate-bound (cyclonedx `sley2:commit=36f0e1f`, spdx namespace binds
inventory digest + attested artifact digest `54b50a6e…`, provenance
`externalParameters.commit=36f0e1f` + subject digest `54b50a6e…`).
- Docs untouched by the checker: sha256 of all three documents identical
before/after the checker run (`sha256sum -c` OK).
- Suite: `Ran 105 tests ... OK` via `python3 -m unittest discover -s
bench/release/tests -t .` (the checker gate's own command; the single
`test_standards_sbom` module contributes 51, incl. all 7 closure tests).
The repaired `test_builders_admit_exactly_the_live_closure_verdict` passes: it
reads the real `closure_status(candidate)` (here: non-closure) and asserts both
builders refuse with a `records-closure-` reason — state-independent, no rot.

**Bypass attempts (all executed, none admitted).**
- B1, live-tree (executed, no FS change): out-of-prefix tracked diff
(`crates/`, `scripts/`, `docs/spec/`, `Makefile`, …) → `is_closure=False`,
`records-closure-ineligible`; both builders refuse (74001/74005). An
attestation-bound path change is NOT admissible.
- B2, in-process predicate (executed): T52 inventory perturbation →
`is_closure=False`, `records-closure-bound-changed:
evidence/security/T52/pre-release-inventory.json`; both builders refuse
(pinned by `test_builders_refuse_a_bound_artifact_change`, passing).
- B3, in-process predicate (executed): combined attestation-bound + bound
change → still `is_closure=False`, `bound-changed` reason takes precedence;
single refusal, no ambiguity.
- Prefix classifier spot-check (executed against module constants):
`evidence/…`/`machineresearch/…` eligible, `scripts/`, `docs/spec/`,
`Cargo.lock` ineligible; only the T52 inventory is bound.
- At/between candidate boundaries: reasoned from code paths, NOT executed
(checkout would dirty the tree): at candidate (`head == attested`) builders
take the normal derivation path (`records-closure-not-advanced`, no closure
involved); any advanced non-closure HEAD takes the refusal path above. The
admission path itself is executed at unit level
(`test_builders_admit_an_eligible_closure_without_remint`, passing).
- Nabu P3 (T52 repro report in bound inventory): NOT re-raised — no new
evidence; byte-identical re-derivation + 4-tuple gate still bind it.

**Refusal pinning: yes, at all three levels.** Predicate reasons (unit),
builder codes + reason substrings (stubbed eligible/ineligible/bound-changed),
live-tree refusals (real outputs above). Hand-mint resistance unchanged
(rewritten-namespace / rewritten-subject tests pass).

**Narrowness from the surface.** Unchanged since a4b6029: no new commands,
flags, codes, or document fields; refusal shapes unchanged apart from the
appended reason; no timestamps, hostnames, or paths in documents (checker pins);
vocabulary stable
(`records-closure-eligible/ineligible/bound-changed/unverifiable/not-advanced`).

**Honest notes.** The current redness (builders refuse, checker FAIL) is the
designed posture on a non-closure HEAD, not a regression. Machine summary
correctly keeps the standards rows unclosed; closing any row on a closure HEAD
still needs the fresh Council review of this model per the Status line.

**FINDINGS:**

- [P3, held — unchanged since a4b6029, non-blocking] `scripts/check_standards_sbom_and_provenance.py:350-373` — on a non-closure HEAD the checker reports both `sbom:drift`/`provenance:drift` (builder nonzero relabeled) and `closure:ineligible` for one underlying cause; accurate but redundant, operator must read `records_closure.reason` for the true cause. Cosmetic.
- [P4, informational — unchanged, non-blocking] `bench/release/tests/test_standards_sbom.py:678-700` — the admit-without-remint test compares properties/subject/SPDX version rather than full byte-identity; byte-identity covered separately by drift tests, coverage complete in aggregate, test name overclaims slightly.

**SUMMARY:** The P1 is repaired and verified: the suite is 105/105 green at a
non-closure HEAD by asserting state rather than assuming it, and the
`release-tests:fail` problem is gone from the checker output while every gate
stays red for the right underlying reason. The closure model refuses all
executed bypasses with the designed codes and reasons, keeps the closure HEAD
out of the documents, and never rewrites them. No new evidence on the declined
Nabu P3. Surface is green-by-design on closures and red-by-design otherwise.

VERDICT: PASS / SECTION: standards_sbom_and_provenance / FIELD: vulcan_surface_review / SCOPE_SHA: db1bc623d01e838d49c153feb0be05a7502b8794 / FINDINGS: 2
