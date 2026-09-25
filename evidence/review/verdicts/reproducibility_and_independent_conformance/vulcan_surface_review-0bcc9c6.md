# Vulcan surface review, REQ-02 re-review (S20-730 rev 4→5, sibling enumeration)

Baseline verified: `git rev-parse HEAD` = `0bcc9c646f110e2e433e8fe3e9b913d49d84c531`. Read-only; nothing written, no scratch files (the one `/tmp` write I composed was refused by the sandbox before it ran; all comparisons were redone inline). This transcript is the evidence artifact; the integrator files it at `evidence/review/verdicts/reproducibility_and_independent_conformance/vulcan_surface_review-0bcc9c6.md`.

## Assumptions and method

- Python is not executable in this session's sandbox, so `unittest`, `--check`, and the S20-730 checker were **not executed**. Findings come from source reading plus independent recomputation with `sha256sum`, `jq`, and `git`. Confidence is stated per finding.
- The working tree carries in-flight state from the concurrent REQ-01 round (`M machine-summary.json`, three untracked `complete_entity_impact_profile/*-0bcc9c6.md` transcripts) plus the two retained untracked RW-080 slices. None of it is on the review baseline; it matters only for the clean-tree adjudication below.
- The prior verdict on `c5973c9` is not rewritten; this supersedes it in the same lane.

## What verifies on the current tree

- **Prior P1 (one-corpus map) repaired.** `family_record` (builder:296-343) now enumerates every `v*` directory of a family, builds a `version_record` for each (digests, mandatory manifest, `SUMS_MISMATCH` on any divergence), claims depth only for the pinned version, and records siblings under `coverage.kind = tracked_sibling` naming the pinned version. On disk the report lists 4 families with `tracked_versions = [v1, v2]` (`bootstrap-profile`, `exec-package`, `host-abi`, `smp1-json-bridge`), matching the tracked directory set exactly.
- **Digest coverage is now total.** 88 tracked files under `conformance/` (87 prior + the new `smp1-json-bridge/v2/SHA256SUMS`); the report records 88 digests across pinned and sibling records; `sha256sum -c` over all 88 → 88 OK, 0 FAIL; set difference tracked-vs-report is empty in both directions. The crate-embedded `smp1-json-bridge/v2/methods.json` (`include_str!` in `crates/sley-json-bridge/src/lib.rs`) is now summed (`ae4746da…db7d`, verifies).
- `report_digest` recomputes from `jq -S --indent 2 'del(.report_digest)'` → `e29f364f…b6b7`, exact match. 25/25 families, `native_only: []`, `COMPLETE`, every pinned and sibling record `sums_file: true / sums_consistent: true`.
- **Prior P3 (key validation) repaired.** `validate_corpus_versions()` (builder:206-217) runs first in `build_report()`; a pin outside `COVERAGE` is `ORACLE_DRIFT`. Tested by `test_a_corpus_pin_outside_coverage_fails_closed` (module global restored in `finally`).
- **Prior P2 (tests) mostly repaired.** Three new tests: missing pinned directory → `FIXTURE_UNREADABLE`; pin outside coverage → `ORACLE_DRIFT`; sibling enumeration without depth. Residual gap is finding 3.
- **Prior P2 (contract wording) repaired.** Section 3 now says a manifest "must name every JSON file of its version directory", every tracked version directory carries one, absence is `FIXTURE_UNREADABLE` "never a silent gap"; the schema block now types `sums_file: true` / `sums_consistent: true`, matching the builder's unreachable-null behaviour. Section 10 records the `v2` pin rationale (306/307 are protocol-v2 methods; no v1 corpus ever existed) and why siblings carry no depth.
- **Prior P3/P4 records repaired.** ADR-0040:3 says revision 5; builder docstring says `v<N>`; `runner_label` now labels `check_entity_read_vectors.py` as `oracle/scb1` because it imports `sley2_scb1_oracle` (verified: the other `uv run --project oracle/scb1 python scripts/…` checkers do not import the package, so their `scripts/` label is honest).
- Oracle refresh is promote-safe: `entity_read.refresh` (oracle:3145-3154) stages `inputs.json` beside `accepted`/`rejected` and sums every `*.json` in sorted order; the oracle test asserts the three-line manifest.
- `git status --porcelain -- <ARTIFACT_INPUT_PATHS>` is empty at HEAD, so the checker's `uncommitted-surface-changes` branch does not fire; only the `stale` branch does.
- Machine summary S20-730 section: 25/25/[]/COMPLETE, `contract_revision: 5`, `attested_commit` now equals the report's `84bfa9c9…` (prior mismatch closed); review revision chain preserved (`vulcan_surface_review` FAIL + `_revision_1` REVISE with transcript pointer).

## Adjudication of the standing P1 (stale reproducibility attestation)

**It stands as an open P1 [record] and it holds the section at REVISE. It is not an implementation defect and no code change is requested for it.**

Facts, all recomputed: `evidence/release/reproducibility-report.json` attests `84bfa9c`, now 178 commits behind HEAD; `git diff --name-only 84bfa9c HEAD -- <ARTIFACT_INPUT_PATHS>` lists 42 files (unchanged from last round: the delta's crate edits were already inside the changed set). `history_problems` (checker:142-187) therefore emits `reproducibility-report:stale:84bfa9c9c5d9:42-surface-files-changed:…` and `main` returns FAIL. That checker is the tenth line of the `quick:` target (Makefile:79) and the fifth check of `release-candidate-smoke` (Makefile:221), so the Tier-1 gate for this repository is red at the baseline on this section's own rule. Confidence: high on the diff and the code path; medium-high on the exit status (not executed).

Why it cannot be cured by the delta: the only path that refreshes the attestation is `build_release_candidate.py --require-clean` (Makefile:208), whose `git_state()` (builder:430-434) runs a whole-tree `git status --porcelain` and raises `PACKAGE_TREE_DIRTY` on any entry, untracked included. The retained RW-080 slices are untracked, so under the standing operator constraint (neither the clean-tree rule nor the slices may be disturbed) re-attestation is mechanically impossible. The freshness mechanism is doing exactly what the prior P0-1 repair asked of it: refusing to let an old candidate read as current.

What the operator can do (no rule change): commit or relocate the two `.forge/slices/rw-080-*.json` files and the in-flight review artifacts, then run `make release-candidate-smoke` on a clean tree. Finding 2 records the rule tension for a contract decision; I do not recommend loosening `require_clean` unilaterally, since weakening a release gate to make a review pass is the wrong direction.

## Findings

**1. P1 [record]** `evidence/release/reproducibility-report.json` – attests `84bfa9c`, 178 commits behind, 42 artifact-surface files changed; `scripts/check_reproducibility_and_independent_conformance.py` FAILs `stale` at HEAD, and it is in `make quick` (Makefile:79). Adjudicated above: open, record-class, constraint-blocked, holds REVISE. Confidence: high.

**2. P3 [contract]** `scripts/build_release_candidate.py:430-440` – `require_clean` tests the whole tree including untracked, non-surface files, while the S20-730 freshness rule (checker:168-177) scopes staleness to `ARTIFACT_INPUT_PATHS`. The two contracts disagree on what a clean candidate needs, and that disagreement is the exact reason retained slices block re-attestation indefinitely. Advisory for Nabu/operator: decide in the S20-720 contract whether "clean" means the artifact surface plus tracked modifications, or keep whole-tree and treat slice retention as incompatible with attestation. Not a builder change to make on review authority.

**3. P3 [implementation]** `bench/release/tests/test_reproducibility.py` – residual from prior finding 4: no test for a version directory lacking `SHA256SUMS` (builder:271-277, `FIXTURE_UNREADABLE`) or a manifest that omits a JSON file (builder:278-284, caught by dict inequality). Downgraded from P2 because the pinned-directory-absent and out-of-coverage-pin paths now have tests and the omission path shares the mismatch code path. Confidence: high (read).

**4. P4 [implementation]** `scripts/build_independent_conformance_report.py:310-312` – `tracked_versions` is enumerated from the working tree (`iterdir`, `name.startswith("v")`), not from `git ls-files`; an untracked `v3/` or a `vectors/` directory would be reported as "tracked". Fail-closed in practice: `--check` returns `REPORT_DRIFT` against the tracked report, and a `v`-prefixed non-version directory fails on the missing manifest. Docstring says "tracked files only"; the label is one step stronger than the mechanism.

**5. P4 [implementation]** `scripts/build_independent_conformance_report.py:192-203` – `read_sums` collapses duplicate manifest lines to the last digest (carried from prior finding 9, unrepaired, unchanged severity).

**6. P4 [contract]** `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:317` – revision 5 dropped the article: "reading as semantically judged one" (revision 4 read "as a semantically judged one").

```
VERDICT: REVISE
SECTION: reproducibility_and_independent_conformance
FIELD: vulcan_surface_review
SCOPE_SHA: 0bcc9c646f110e2e433e8fe3e9b913d49d84c531
FINDINGS:
[P1] [record] evidence/release/reproducibility-report.json - attests 84bfa9c, 178 commits behind with 42 artifact-surface files changed; check_reproducibility_and_independent_conformance.py FAILs stale at HEAD and sits in make quick (Makefile:79); adjudicated open and constraint-blocked, not an implementation defect; cure is committing or relocating the untracked RW-080 slices then clean-tree make release-candidate-smoke
[P3] [contract] scripts/build_release_candidate.py:430-440 - require_clean tests the whole tree incl. untracked non-surface files while the S20-730 freshness rule scopes to ARTIFACT_INPUT_PATHS; the rule mismatch is why retained slices block re-attestation; resolve by S20-720 contract decision, not a unilateral builder change
[P3] [implementation] bench/release/tests/test_reproducibility.py - still no test for a version directory without SHA256SUMS (FIXTURE_UNREADABLE) or a manifest omitting a JSON file (SUMS_MISMATCH); residual of prior P2, downgraded
[P4] [implementation] scripts/build_independent_conformance_report.py:310-312 - tracked_versions enumerated from the working tree (iterdir, startswith v), not git ls-files; --check REPORT_DRIFT makes it fail-closed in practice
[P4] [implementation] scripts/build_independent_conformance_report.py:192-203 - duplicate manifest lines collapse to the last digest (carried, unrepaired)
[P4] [contract] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:317 - revision 5 dropped the article in "reading as semantically judged one"
SUMMARY: The delta c5973c9..0bcc9c6 repairs every implementation and contract finding of the prior REVISE: family_record now enumerates every tracked corpus version with a mandatory manifest per version and tracked_sibling coverage for non-pinned versions; all 88 tracked conformance files (including the crate-embedded smp1-json-bridge/v2/methods.json, now summed) are inside report digest coverage and verify on disk with report_digest recomputing exactly; CORPUS_VERSION pins outside COVERAGE fail closed with a test, the missing-pinned-directory path has a test, contract section 3 and the schema block match the builder, section 10 records the v2 pin and sibling rationale, ADR-0040 and the docstring are current, and the machine summary attested_commit matches the report. The section is held at REVISE by one item only: the reproducibility attestation is 178 commits stale with 42 artifact-surface files changed, so the section's own checker FAILs in make quick at the baseline; this is adjudicated as an open record defect, functioning as designed, blocked by the standing constraint because the candidate builder's whole-tree clean rule counts the retained untracked slices, and curable by committing or relocating those slices and running a clean-tree make release-candidate-smoke. Remaining findings are a P3 contract note on the clean-rule versus freshness-surface mismatch, a P3 residual test gap, and three P4s. Python was not executable in this session: tests, --check, and the checker are unexecuted; digests, file inventories, surface diffs, and report integrity were recomputed independently with sha256sum, jq, and git.
```
