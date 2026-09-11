# Council final confirmation — release_candidate_packaging/nabu_architecture_review (SCOPE_SHA 3320ca9)

Read-only; nothing written. HEAD verified equal to SCOPE_SHA at dispatch.

Verification is complete. Evidence gathered (all read-only, no files written):

**Scope and delta**
- `git rev-parse HEAD` = `3320ca92ba6b34e983111769d6e85b3ef613d228`. Working tree carries only the two untracked `.forge/slices/*.json` files, outside the packaging surface.
- `git diff cd3864a HEAD -- scripts/build_release_candidate.py` is empty: builder byte-identical since cd3864a, as stated.
- `git diff a12fcfb HEAD` on the scope surface: checker `:189-190,:197` null-toolchain guards (`(x.get("toolchain") or {})`, `section.get("candidate_toolchain") or {}`); contract `:170-176` nine-field binding rule and `:256` one-line section 13 heading; closeout `:162-171` superseded-by note; `test_packaging.py:249-294` failure-evidence test. Makefile, ADR, demo, fixture unchanged.

**Attestation and record coherence**
- `reproducibility-report.json` names exactly one attestation: `9df9f9e` / `483124c5` / 2131794 B / manifest `a93f612f` / 14 members / REPRODUCIBLE / `working_tree_clean: true` / cargo+rustc 1.93.0. `report_digest` recomputed (`grep -v report_digest | sha256sum`) = `bc8b3520…d46e`, matches `:40`.
- `9df9f9e` is an ancestor of HEAD; `git diff --name-only 9df9f9e HEAD` is seven record files (dossier, provenance, repro report, both SBOMs, T54 scan, register), none in `ARTIFACT_INPUT_PATHS` (`build_release_candidate.py:40-49`). S20-730 freshness predicates hold by hand-trace.
- Nine-tuple cross-check (`check_release_candidate_packaging.py:180-209`) hand-traced against register `:3679-3694`: all nine values equal the attestation. Provenance `:24-27,:100,:108`, dossier `:37,:284`, CycloneDX `:2122,:2137,:2149`, SPDX `:11,:26`, S20-730 `attested_commit :3920`, `attestation_supersedes :3925` all bind `9df9f9e/483124c5/a93f612f`. No dissenting consumer.
- Prior-round P2s closed: `--no-keep` is an accepted flag (`:567`), invocation recorded on every path (`:594,:598,:602`), partial evidence preserved on PackageError (`:471-474,:554-557`), both toolchains sampled (`:449,:533-534`); pinned by `test_packaging.py:208-294`. Prior P3 (binding rule absent from contract) and P4 (narrow tuple) closed by the nine-field rule and the widened tuple.

**Assumptions and limits**
- Sandbox denied Python execution and any read under `~/cache/worktrees/sley2-repro-56bac4b` (same limit as both prior rounds). Checker PASS is hand-traced, not re-executed; the worktree `evidence.json` derivation link is taken from the 3320ca9 message, not independently read; artifact `483124c5` was not hashed.
- The only tracked Nabu PASS transcript for this section is `nabu_architecture_review-abf4ff0.md` (filed 237c30d). No a12fcfb Nabu transcript for this section exists in-repo; I treat the abf4ff0 PASS as the "prior PASS" the task references.
- Standing exclusions (second-host GATED, supply DEFERRED, candidate-validation FAIL, GA unclaimed) not re-litigated.

```
VERDICT: PASS
SECTION: release_candidate_packaging
FIELD: nabu_architecture_review
SCOPE_SHA: 3320ca92ba6b34e983111769d6e85b3ef613d228
FINDINGS:
- [P3] [record] docs/audits/S20_720_RELEASE_CANDIDATE_CLOSEOUT.md:167 - the superseded-by note added at 8d6664e names `cd3864a`/`c1dec862` as "current attestation"; 3320ca9 superseded it with `9df9f9e`/`483124c5`. Narrative-only (register and checker are correct), but the note's stated purpose was to stop this doc going stale per mint; drop the parenthetical or point it at the register's candidate_* fields.
- [P3] [record] machineresearch/sley-2.0/machine-summary.json:3667,3697 with evidence/review/finding-register.json:1541-1573 - the section carries `nabu_architecture_review` FAIL and `_revision_1` REVISE only; the filed PASS transcript (verdicts/release_candidate_packaging/nabu_architecture_review-abf4ff0.md, 237c30d) has no `_revision_2` register field, so the register's review-string state lags the evidence directory by one round. Close when this confirmation is filed.
- [P3] [implementation] Makefile:210-227 - carried, unrepaired: release-candidate-smoke writes S20-710/730/750 records that contract section 10 excludes and section 9 (:193-203) does not describe; no per-output owner map. Follow-up.
- [P4] [implementation] bench/release/tests/test_packaging.py:245-249 - `FailureEvidenceTests` is defined after the `if __name__ == "__main__": unittest.main()` guard; collected by the checker's `unittest discover` (:232) but silently skipped when the module is run as a script. Move the class above the guard.
- [P4] [record] machineresearch/sley-2.0/machine-summary.json:3684,3703 - `offline_tests: 12` now matches the 12 test methods; the note records that the checker does not enforce the field, so the drift class remains open. Enforce or drop.
- [P4] [contract] docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md:158-160 - carried: three-space mid-paragraph continuation indent in section 7.
- [P4] [record] git worktree list; machine-summary.json:3702 - `sley2-repro-56bac4b` is checked out at 9df9f9e and holds the only copy of artifact 483124c5; the label mismatch is now disclosed in `mint_worktree`. Retention under the disk hygiene rule still needs a deliberate pointer or removal once the artifact is archived.
SUMMARY: The architecture passes on the final confirmation, and nothing in the narrow delta since the prior PASS weakens it. The builder is byte-identical since cd3864a, so the load-bearing mechanics I passed at abf4ff0 (most-general-first remaps, deterministic PAX tar, self-describing manifest digest, clean-tree default, content-versus-archive comparison) are unchanged by construction; the two implementation deltas are fail-closed tightenings of the checker (a null toolchain object can no longer raise AttributeError and mask a mismatch as a crash, it now fails as a plain tuple miss) and contract text that finally states the nine-field binding rule the checker enforces, closing the prior P3 and P4 on the same surface. Authority holds: the reproducibility attestation is the single tracked root of candidate identity, the register's nine candidate_* fields equal it, and every downstream record (provenance subject and invocationId, dossier, both SBOMs, S20-730 attested_commit and supersedes note) binds 9df9f9e/483124c5/a93f612f with no dissenting consumer. Invocation is recorded rather than inferred on every exit path and the recorded string is replayable. Coherence holds: 9df9f9e is an ancestor, the seven files changed since are all records outside ARTIFACT_INPUT_PATHS, the report digest recomputes, the toolchain is 1.93.0 on both builds. The residuals are record hygiene (a closeout note already stale by one mint, a register that lags the filed PASS transcript by one revision field) and carried P3/P4 debt that does not bear on identity binding; none breaks a pin, so they are filed as follow-ups. Standing exclusions (second host, supply, candidate validation, GA) are unclaimed and untouched.
```
