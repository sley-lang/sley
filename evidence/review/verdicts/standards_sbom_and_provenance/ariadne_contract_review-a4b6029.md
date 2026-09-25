# Ariadne contract review — standards_sbom_and_provenance records-closure amendment (rev 5, SCOPE_SHA a4b6029)

Read-only review; HEAD verified equal to SCOPE_SHA at dispatch (`git rev-parse HEAD` =
`a4b60294e4dd75133fa08f92bf16022fb05bb807`). No files edited except this verdict.
Scope: `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md` Status rev 5 + section 5
records-closure language vs `scripts/records_closure.py`, the builder/checker diffs in
`a4b6029`, and live execution of the three checks plus the 105-test suite.

**Contract read.** Status (spec `:3-6`) declares revision 5 and explicitly conditions
closing any standards row on a closure HEAD on fresh Council review of this model.
Section 4 (`:181-187`) admits a non-HEAD candidate only as a "provable records-closure
HEAD ... whose attestation-bound inputs are unchanged so the derived statement is
byte-identical". Section 5 (`:216-222`) keeps the write-mode refusal for anything else,
and the records-closure model block (`:241-266`) states the full rule: attested source
commit vs closure HEAD distinguished; admission only when every changed tracked path is
under `evidence/` or `machineresearch/` and bound inputs are unchanged; documents stay
candidate-bound; closure HEAD recorded separately, never in documents; refusals reuse
`SBOM_INVENTORY_INVALID` / `PROVENANCE_EVIDENCE_INVALID` with `records-closure-…`
reasons; write mode still names `make release-candidate-smoke`.

**Implementation match (verified by execution, not by reading alone).**
`scripts/records_closure.py:35-48` encodes exactly the spec's eligible prefixes and the
single bound path (`evidence/security/T52/pre-release-inventory.json`); `:62-89` makes
`is_closure` conjunctive (no git error, advanced, no ineligible, no bound-changed) and
`reason` a stable machine-readable marker; `:106-139` fails closed on any git error and
on unknown commits. Both builders bypass only the HEAD-equality refusal on
`is_closure` (`scripts/build_standards_sbom.py:475-484`,
`scripts/build_release_provenance.py:214-222`) while the attestation 4-tuple gate
(commit, artifact_sha256, manifest_digest, artifact_size_bytes + REPRODUCIBLE + clean)
runs unconditionally afterwards (`build_standards_sbom.py:492-508`,
`build_release_provenance.py:224-239`). The checker (`check_standards_sbom_and_provenance.py:353-373`)
records `attested_source_commit` / `records_closure_head` / `reason` in its output
object only, and appends `closure:ineligible` otherwise. Live run confirms every word:
candidate `36f0e1f99bb7b09c407e870141ca18f109dfaf8a` (PASS, 1 clean REPRODUCIBLE
attestation), HEAD `a4b6029`, `records-closure-ineligible` with 18 attestation-bound
paths, sbom `--check` → 74001, provenance `--check` → 74005, both details carrying the
`records-closure-ineligible` reason and naming the rebuild command; checker FAIL with
`closure:ineligible` and the `records_closure` block naming both commits. `grep -c`
for `a4b6029` in all three documents is 0; all three bind `36f0e1f`. The contract text
and the code agree with no gap found.

**Misbinding.** A changed crate/script/spec/lockfile cannot slip through: 18 such paths
in the live diff each independently force refusal. A changed T52 inventory forces
`records-closure-bound-changed` refusal (pinned by unit + builder tests). Hand-minted
documents cannot pass: admission only derives from attestation-bound inputs, and
`--check` demands byte-identity (74003/74007) plus namespace/subject attestation
binding (pre-existing rewritten-namespace/subject tests). The model is narrow — no
general skew tolerance exists anywhere in the diff.

**Honest live-state note.** The amendment commit itself ships attestation-bound changes
(scripts/spec/bench/crates), so HEAD `a4b6029` is a non-closure of candidate `36f0e1f`
by design: 27 tracked paths changed, 18 ineligible vs 9 eligible records paths (8 under
`evidence/` + `machineresearch/sley-2.0/machine-summary.json`). Builders refuse again,
checker is red, and the machine summary keeps `standards_sbom=false`,
`release_provenance=false`, status `IMPLEMENTED_REVIEW_PENDING`. The 9 standards rows
stay open pending genuine Council review regardless of this verdict.

**FINDINGS:**

- [P2] `bench/release/tests/test_standards_sbom.py:624-632` - `test_live_tree_is_a_records_closure_of_the_attested_candidate` asserts the live tree is a closure, a property the contract guarantees at no commit; the amendment's own attestation-bound changes falsify it at its shipping commit, so the suite is red (104/105) at HEAD. Fails closed (a test failure, not an open gate), but the integration test should stub the tree state or assert the expected verdict instead of assuming HEAD is a closure. Cross-reference: vulcan lane carries the fix demand.
- [P3] `machineresearch/sley-2.0/machine-summary.json` (`standards_sbom_and_provenance` block) - summary still records `contract_revision: 4` while the spec Status line declares revision 5; the implementation-state pointer lags the contract it tracks. Cosmetic, no gate impact.
- [P4] `docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md:251-254` - the "none of the bound inputs changed" sentence names only the T52 inventory while the emitted documents and the reproducibility report are validated indirectly (re-derivation + 4-tuple gate). Accurate as written, but a reader must assemble the two-layer argument across sections 4, 5, and 7; a forward pointer would help.

**SUMMARY:** The rev 5 contract text is a faithful freeze of fail-closed, narrow,
two-layer behavior (closure admission + byte-identical re-derivation under the
attestation 4-tuple), and live execution confirms every clause including the designed
refusal on this non-closure HEAD. No contract defect found; the one red test is a
surface-hygiene issue, not a contract gap.

VERDICT: PASS-3 / SECTION: standards_sbom_and_provenance / FIELD: ariadne_contract_review / SCOPE_SHA: a4b60294e4dd75133fa08f92bf16022fb05bb807 / FINDINGS: 3
