# Vulcan surface review, REQ-02 (S20-730 rev 3→4, entity-read coverage)

Baseline verified: `git rev-parse HEAD` = `c5973c90180d03402f0d2e5d2d91e941ef5dc58d`. Read-only; nothing written (a scratch-file write was refused by me before it happened; all audit ran inline).

## Assumptions and method

- Python execution (`--check`, `unittest`) is denied in this session's sandbox, so the tests and the drift check were **not executed**. Everything below comes from source reading plus independent digest recomputation with `sha256sum` and `jq`. Confidence is stated per finding.
- The prior Vulcan round (2026-09-04, `9dc78fe`) is not rewritten; this is a same-lane superseding verdict on the current tree.

## What verifies on the current tree

- `conformance/entity-read/v2/SHA256SUMS` names all three JSON files (`accepted.json`, `inputs.json`, `rejected.json`); `sha256sum -c` → 3/3 OK. Every-JSON rule satisfied for the pinned directory.
- Report digest coverage: all 79 file digests recorded in `evidence/conformance/independent-conformance-report.json` verify against disk (79 OK, 0 fail). `report_digest` recomputes from `jq -S --indent 2 'del(.report_digest)'` → `3bbc75ce…aba4`, exact match. 25/25 families, `native_only: []`, `INDEPENDENT_CONFORMANCE_COMPLETE`, all 25 `sums_file: true / sums_consistent: true`.
- `coverage_depths.semantic` = 5 directories including `conformance/entity-read/v2`; the test pin at `test_reproducibility.py:292-298` matches.
- `COVERAGE["entity-read"]` string is present verbatim in the `conformance:` recipe (`Makefile:144`).
- Forbidden-marker scan: 0 hits in `oracle/scb1/src/sley2_scb1_oracle/entity_read.py` and `scripts/check_entity_read_vectors.py`; `python_sources: 28` = 10 oracle modules + 18 distinct `scripts/check_*.py`.
- Version-map posture: unknown family → `ORACLE_DRIFT` (checked before the version lookup, tested); pinned version directory absent → `FIXTURE_UNREADABLE`; `CORPUS_VERSION` pinned exactly to `{"entity-read": "v2"}` in the tests. No default-pass for unknown families.
- Machine summary counters (25/25/[]/COMPLETE, revision 4) match the report.
- Prior P0-2 (repro report never digest-verified) is repaired: `check_reproducibility_and_independent_conformance.py:325-327` calls `verify_report`. Prior P0-1 (staleness undetected) is repaired as a mechanism: `history_problems` (lines 150-187) diffs the artifact surface. That mechanism is now firing (finding 2).

## Findings

**1. P1 [implementation]** `scripts/build_independent_conformance_report.py:206-216` – the version map admits exactly one corpus per family and silently ignores every other tracked version directory. Four families carry unpinned `v2` corpora (`bootstrap-profile/v2`, `exec-package/v2`, `host-abi/v2`, `smp1-json-bridge/v2`, landed 2026-09-06 and 2026-09-08): 8 of 87 tracked files under `conformance/` are outside the report's digest coverage, and `conformance/smp1-json-bridge/v2/methods.json` (embedded into `crates/sley-json-bridge/src/lib.rs:43` via `include_str!`) has **no SHA256SUMS anywhere**, while the report still reads `COMPLETE`. The prior round's P2 ("only v1 subtrees are read, so a v2 fixture set is silently uncovered") was hypothetical then and is concrete now; rev 4 fixed it for entity-read only. Confidence: high (file inventory and manifests verified directly).

**2. P1 [record]** `evidence/release/reproducibility-report.json` – attests `84bfa9c`, 177 commits behind HEAD with 42 artifact-surface files changed (`git diff --name-only 84bfa9c HEAD -- <ARTIFACT_INPUT_PATHS>`). Under the rev-3/4 freshness rule the staged checker emits `reproducibility-report:stale:…:42-surface-files-changed` and returns FAIL, so the section gate does not pass at the review baseline. Disclosed in the `c5973c9` message ("stays stale by rule"); cure is a clean-tree `make release-candidate-smoke`, not an edit. Confidence: high on the diff count, medium-high on the checker outcome (not executed).

**3. P2 [contract]** `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:157-166` – rev 4 scopes the SUMS and digest rules "inside the versioned directory" while the adjacent rule still says a manifest "must name every JSON file of its family"; the two readings diverge exactly on finding 1. Section 3 also still promises `sums_consistent: null` / `sums_file: bool` for a family without a manifest, but the builder fails closed with `FIXTURE_UNREADABLE` (lines 245-251), making `null`/`false` unreachable. Prior P2, unrepaired in rev 4.

**4. P2 [implementation]** `bench/release/tests/test_reproducibility.py` – no test exercises the new fail-closed path (pinned version directory absent → `FIXTURE_UNREADABLE`) or a manifest that omits a JSON file; only the wrong-digest case (`test_mismatched_sums_fail_closed`) is covered.

**5. P3 [implementation]** builder – `CORPUS_VERSION` keys are not validated against `COVERAGE` (a stale or misspelled key is silently ignored); `DEPTH` gets a set-equality test, `CORPUS_VERSION` relies only on the exact-map pin.

**6. P3 [record]** `79a1209` landed the builder, contract and tests without refreshing the tracked report or machine summary (still 24 families / revision 3); `--check` would have returned `REPORT_DRIFT` at that commit. The report had already been stale since `6d59f1f` (2026-09-08, repository-exchange corpus change). Repaired in `c5973c9`; the baseline is consistent.

**7. P3 [record]** `docs/adr/ADR-0040-…:3` still says "draft at revision 3".

**8. P4 [implementation]** `scripts/build_independent_conformance_report.py:5` docstring still says `conformance/<family>/v1`.

**9. P4 [implementation]** `read_sums` (lines 184-195) collapses duplicate manifest lines for one file to the last digest, so a manifest that `sha256sum -c` would reject can pass.

```
VERDICT: REVISE
SECTION: reproducibility_and_independent_conformance
FIELD: vulcan_surface_review
SCOPE_SHA: c5973c90180d03402f0d2e5d2d91e941ef5dc58d
FINDINGS:
[P1] [implementation] scripts/build_independent_conformance_report.py:206-216 - version map covers one corpus per family; 4 tracked unpinned v2 corpora (8/87 files) are outside report digest coverage and conformance/smp1-json-bridge/v2/methods.json (crate-embedded) has no SHA256SUMS at all while the report reads COMPLETE
[P1] [record] evidence/release/reproducibility-report.json - attests 84bfa9c, 177 commits behind with 42 artifact-surface files changed; staged checker FAILs stale at baseline (disclosed in c5973c9); cure is clean-tree make release-candidate-smoke
[P2] [contract] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:157-166 - "inside the versioned directory" vs "every JSON file of its family" diverge on finding 1; sums_consistent:null/sums_file:false still promised though builder fails closed
[P2] [implementation] bench/release/tests/test_reproducibility.py - no test for absent pinned version dir -> FIXTURE_UNREADABLE or for a manifest omitting a JSON file
[P3] [implementation] scripts/build_independent_conformance_report.py:82 - CORPUS_VERSION keys not validated against COVERAGE in the builder
[P3] [record] 79a1209 - delta commit shipped a drifted report and machine summary (24 families, rev 3); REPORT_DRIFT at that commit; repaired in c5973c9
[P3] [record] docs/adr/ADR-0040-reproducibility-and-independent-conformance-boundary.md:3 - status still names revision 3
[P4] [implementation] scripts/build_independent_conformance_report.py:5 - docstring still says conformance/<family>/v1
[P4] [implementation] scripts/build_independent_conformance_report.py:184-195 - duplicate manifest lines collapse to the last digest
SUMMARY: The delta's stated surface is verified on the baseline tree: entity-read/v2 SHA256SUMS names accepted.json, inputs.json and rejected.json with correct digests; all 79 report-recorded digests verify against disk; report_digest recomputes; 25/25 families at their pinned version are independently covered with the entity-read checker in the Makefile recipe and clean under the forbidden-marker scan; CORPUS_VERSION and the semantic set are pinned exactly in the tests and unknown families fail closed. Both prior P0s are repaired as mechanisms (verify_report, artifact-surface freshness), so this supersedes the 2026-09-04 FAIL. It is not a PASS because the version map's one-corpus-per-family model leaves four tracked v2 corpora outside digest coverage, one with no manifest at all, under a COMPLETE result the rev-4 wording now legitimizes (P1 implementation, P2 contract), and because the section's own staged gate fails at HEAD on the stale reproducibility attestation (P1 record). Python was not executable in this session, so unit tests and --check are unexecuted; digests and report integrity were recomputed independently with sha256sum and jq.
```
