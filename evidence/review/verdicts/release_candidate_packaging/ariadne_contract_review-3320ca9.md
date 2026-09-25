# Council final confirmation — release_candidate_packaging/ariadne_contract_review (SCOPE_SHA 3320ca9)

Read-only; nothing written. HEAD verified equal to SCOPE_SHA at dispatch.

# VERDICT: PASS

**SECTION:** release_candidate_packaging
**FIELD:** ariadne_contract_review
**SCOPE_SHA:** `3320ca92ba6b34e983111769d6e85b3ef613d228` (verified `git rev-parse HEAD`; matches expected)

Read-only; nothing written (a `/tmp` scratch write was denied and not needed).

## Verification performed

- **Builder unchanged:** `git diff --stat cd3864a HEAD -- scripts/build_release_candidate.py` is empty. Failure-evidence (`:594-602`), invocation recording (`:578-598`), toolchain emission (`:301-308`, both strings or `BUILD_FAILED`), and version check (`:402`, `"protocol_version":1`) are as previously reviewed.
- **Checker delta** (`scripts/check_release_candidate_packaging.py:189-190,197`): exactly the three `or {}` null-toolchain guards; no other change since a12fcfb.
- **Contract delta** (`docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md:170-175`): nine-field rule sentence (commit, artifact digest, manifest digest, size, member count, cleanliness, reproducibility, toolchain cargo+rustc = the checker's 9-tuple at `:198-208`); section 13 heading collapsed to one line (`:256`); section 1 step 4 (`:45-48`) now names all five needles, matching the scan call at `build_release_candidate.py:506` (tree path, `/home/`, `/home-remapped`, username) plus `SECRET_PATTERNS`. Section 5 was already exact.
- **Four pins at the new attestation:** sole attestation in `evidence/release/reproducibility-report.json` is `9df9f9ee…` / `483124c5…` / 2131794 B / manifest `a93f612f…` / 14 members / `working_tree_clean: true` / `REPRODUCIBLE` / cargo+rustc 1.93.0. Register `candidate_*` fields are identical: the two nine-tuples hash to the same SHA-256 (`ad8e7fb9…6eb89`), so `candidate-attestation-mismatch` cannot fire. `candidate_evidence_commit` = `9df9f9e`. `artifact` block stays null.
- **Record consistency:** digest `483124c5…` present in dossier, provenance, CycloneDX, SPDX (x2); no stale digest in any tracked release record. `report_digest` recomputed (`grep -v report_digest | sha256sum`) = `bc8b3520…fd46e`, matches. `git diff --name-only 9df9f9e HEAD` touches only the seven records files, so the S20-730 freshness rule holds (records-only 3320ca9 as claimed).
- **Gate posture:** status `S20_720_MECHANICS_IMPLEMENTED_REVIEW_PENDING`, `release_check_gate: FAIL_CLOSED_NOT_IMPLEMENTED`, `ga_claimed: false`, `publication_authorized: false`, `implementation_complete: false`.

## FINDINGS

- **P3 [implementation]** `scripts/check_release_candidate_packaging.py:189-190,197` - the `or {}` guards make a `null` (or absent) toolchain on *both* attestation and register compare as `(None, None) == (None, None)`, so the nine-tuple can still bind. Contract `:170-172` says the register must *name* the toolchain. Unreachable from the builder (it raises `BUILD_FAILED` rather than emit a missing version) and pre-existing for the absent-key case; the guard only extends the same admission to explicit `null`. Follow-up: require non-empty `cargo`/`rustc` strings in the binding. Not a REVISE driver.
- **P4 [record]** `machineresearch/sley-2.0/machine-summary.json` `release_candidate_packaging.{ariadne,nabu,vulcan}_*_review` still carry round-1 `FAIL_…` strings. Correct while status is REVIEW_PENDING (completion branch at checker `:240-243` is inactive), but promotion to `S20_720_COMPLETE` requires PASS-prefixed values or `completion-without-review` fires. Follow-up at closeout.
- **P4 [record]** `mint_worktree` label `sley2-repro-56bac4b` predates reuse; already disclosed in `mint_method`. Note only.
- **P4 [record]** Gitignored `dist/sley-2.0.0-linux-x86_64.tar.gz` and `evidence/runtime/s20-720-release-candidate/evidence.json` on the operator tree name the stale dirty smoke `5b70052`/`8ee23a8d…`. Untracked, not authoritative, no leakage into tracked records. Disk-hygiene nit for the session owner.

## SUMMARY

The deltas since my a12fcfb PASS are narrow and fail-closed: the checker change is confined to null-guarding the toolchain lookups, the contract now states in prose the exact nine-field binding the checker enforces, and the builder is byte-identical since cd3864a. The canonical mint at 9df9f9e is bound end to end: attestation, register, dossier, provenance, and both SBOMs name one identity (`483124c5…`, 2131794 B, manifest `a93f612f…`, 14 members, clean, REPRODUCIBLE, toolchain 1.93.0), the report digest recomputes, and 3320ca9 is records-only. All four confirmation pins hold. The one substantive residual (P3, null-null toolchain admission) is a hardening follow-up the builder cannot trigger, not a contract break. Standing exclusions unchanged: second-host attestation, supply, candidate-validation lane, and GA remain out of scope and the gates stay fail-closed.

**Assumptions stated:** (1) `python3 scripts/check_release_candidate_packaging.py` was not executed in this session (permission gate denied subprocess-bearing scripts); the binding was verified statically by identical tuple hashes, and the closeout's `make quick` green claim was not independently re-run here. (2) `evidence/review/verdicts/…/ariadne_contract_review-a12fcfb.md` is taken as in-flight filing per the task text; only the abf4ff0 and e464ed4 transcripts exist on disk at HEAD.
