# Vulcan surface review, REQ-02 final re-review of the stale-attestation P1 (delta 246d5c4..e464ed4)

Baseline verified: `git rev-parse HEAD` = `e464ed49cadda55ea6ef848d7943639c67dd31f0`. Read-only; nothing written. Working tree carries only the two untracked RW-080 slices (`.forge/slices/`, 2285 + 2198 bytes), off the artifact surface. This transcript is the evidence artifact for filing at `evidence/review/verdicts/reproducibility_and_independent_conformance/vulcan_surface_review-e464ed4.md`; the `246d5c4` transcript is not rewritten.

## Assumptions and method

- Python execution was denied by the permission mode again (also `cargo --version`), so the S20-730 checker was **not executed**. `history_problems` (checker:142-187) is byte-identical to the prior round, so I recomputed its three git predicates inline; the section's other checks were recomputed with `jq`, `sha256sum`, and `git`.
- The linked worktree `~/cache/worktrees/sley2-repro-56bac4b` that minted the attestation still exists (`git worktree list`), but its `evidence.json` is outside my permitted read scope (Read denied). The operator-tree `evidence/runtime/s20-720-release-candidate/evidence.json` is gitignored local state at `5b70052`/`8ee23a8d` (dated Sep 10, `working_tree_clean: false`), so it is not the attestation's source. **The link from the S20-720 evidence record to the committed attestation values is therefore unverified by me this round**; I assessed it on internal consistency only.

## (a) Is the stale-attestation P1 cured honestly? Yes.

- **Commit binding.** The sole attestation names `56bac4b877e5…`, a linear ancestor of HEAD, 2 commits behind. Both intervening commits (`9c99e74`, `e464ed4`) touch only `evidence/release/*` and `evidence/security/T54/*`, none of which is in `ARTIFACT_INPUT_PATHS` (`crates`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `bench/release/run_demo.py`, `evidence/security/T52/pre-release-inventory.json`, `scripts/build_release_candidate.py`, three conformance subsets).
- **`history_problems` predicates, recomputed:** `merge-base --is-ancestor 56bac4b HEAD` → true; `git diff --name-only 56bac4b HEAD -- <surface>` → **0 files**; `git status --porcelain -- <surface>` → **0 entries**. The function returns `[]`. The prior `stale:84bfa9c9c5d9:43-surface-files-changed` problem is gone. Confidence: high on logic (unchanged code), not executed.
- **Integrity.** `jq -S --indent 2 'del(.report_digest)' | sha256sum` → `ec36b0ae…e033`, exact match to the recorded `report_digest`, matching `digest_of(canonical(...))` at builder:62-67. Artifact `ac4ce59a…`, 2131797 bytes, manifest `86e09320…`, `member_count: 14`, `differing_members: []`, `working_tree_clean: true`, `commits` map single-entry consistent with the attestation, `result: SINGLE_HOST_REPRODUCIBLE`, all five blockers retained, `ga_claimed`/`publication_authorized` false.
- **Toolchain.** Attested cargo/rustc strings are identical to the prior attestation and match the `rust-toolchain.toml` pin (`1.93.0`). Live `toolchain_versions()` not confirmed (cargo denied); no drift signal exists.
- **Supersession, not hiding.** The report format carries only current attestations by design (`carried_attestations` replaces same-label entries), so the `84bfa9c` attestation is superseded through git history of the file, and the `9c99e74` commit message records the old identity explicitly (`f5c48a3a`, 2094571 B, manifest `20d15438`). That is honest.
- **Downstream records.** `provenance.json` byproduct digest `a0db7adb…` equals `sha256sum` of the on-disk report; `statement_digest` recomputes exactly (`36db73a3…`). T54 `untracked_bytes_scanned: 4483` equals the two slices' byte total exactly; `candidate_files_scanned: 1065` = 1067 tracked minus the two `OUTPUT_PATHS` self-exclusions (generator:265).

## (b) Is the f5c48a3a → ac4ce59a rotation legitimate? Yes, but the stated cause is understated.

The rotation spans `84bfa9c` (2026-09-05 "re-attest the candidate clean after S20-400 closure") to `56bac4b`: **182 commits, 43 artifact-surface files, touched by 62 surface commits**. All of it is committed linear ancestry (`crates/sley-vm/tests/rw080_*`, `rw075_*`, `rw070_*`, protocol/repo/query entity-read work, plus the five Council repair commits). The report carries a single attestation on a single commit, so no host-to-host comparison existed to suppress, and the S20-710 checker (`check_standards_sbom_and_provenance.py:315-325`) still mechanically emits `dirty-candidate` and `subject-attestation-mismatch` because the provenance subject remains `8ee23a8d`/`5b70052`; `e464ed4` discloses both as "honestly red." Nothing is hidden. However, the `9c99e74` commit message and this task's framing attribute the drift to "the five repair commits' tree change." That is inaccurate: 57 of the 62 surface-touching commits predate the first Council repair (`c5973c9`). The +37226-byte size growth is consistent with the RW-070/075/080 VM additions, not with five repair commits alone.

## Findings

**1. P3 [implementation] `scripts/build_decision_dossier.py:315`** (re-emitted into `evidence/release/decision-dossier.json` by `e464ed4`) – the SHA-256 entry's hard-coded note says "the attested artifact digest, which the provenance subject repeats" and cites `provenance.json` as evidence, but the provenance subject is `8ee23a8d`, not `ac4ce59a`. The dossier asserts a cross-record agreement that the S20-710 checker simultaneously reports as `subject-attestation-mismatch`. Pre-existing (was equally false for `f5c48a3a`), not a S20-730 defect, non-blocking for this field; noted because the cure re-derived it. Confidence: high.

**2. P4 [record] `9c99e74` commit message** – "drift is the five repair commits' tree change" understates a 182-commit, 43-file, 62-commit surface drift. Immutable; correct it in the filing register or next dossier note rather than by rewrite. Confidence: high.

**3. P4 [record] `evidence/release/reproducibility-report.json:5-7`** – attestation values are internally consistent and digest-bound, but their derivation from the minting worktree's S20-720 `evidence.json` could not be independently confirmed from this session (read scope and Python denied). Not a defect; an unverified link the integrator can close by reading `~/cache/worktrees/sley2-repro-56bac4b/evidence/runtime/s20-720-release-candidate/evidence.json` and matching `commit`, `artifact_sha256`, `artifact_size_bytes`, `manifest_digest`.

```
VERDICT: PASS
SECTION: reproducibility_and_independent_conformance
FIELD: vulcan_surface_review
SCOPE_SHA: e464ed49cadda55ea6ef848d7943639c67dd31f0
FINDINGS:
[P3] [implementation] scripts/build_decision_dossier.py:315 - hard-coded dossier note "which the provenance subject repeats" is false on this tree (provenance subject 8ee23a8d/5b70052 vs attested ac4ce59a/56bac4b) and cites provenance.json for a value it does not contain; S20-710 checker already reports subject-attestation-mismatch; pre-existing, non-blocking for S20-730
[P4] [record] 9c99e74 commit message - attributes the f5c48a3a->ac4ce59a rotation to "the five repair commits' tree change"; actual drift 84bfa9c..56bac4b is 182 commits, 43 artifact-surface files, 62 surface-touching commits, 57 predating the first Council repair; all committed ancestry, legitimate, but understated
[P4] [record] evidence/release/reproducibility-report.json:5-7 - attestation values digest-bound and internally consistent but their derivation from the minting worktree's S20-720 evidence.json was not independently confirmable this session (read scope and Python denied); operator-tree evidence.json is stale local state at 5b70052 by disclosed design
SUMMARY: The stale-attestation P1 is cured honestly. The sole attestation now names 56bac4b, a linear ancestor two records-only commits behind HEAD; recomputed inline, all three history_problems predicates are empty (ancestor true, zero artifact-surface files changed 56bac4b..HEAD, zero uncommitted surface entries), report_digest recomputes exactly to ec36b0ae, the toolchain matches the 1.93.0 pin and the prior attestation, the commits map and blockers are consistent, and the 84bfa9c attestation is superseded through file history with its old identity recorded in the 9c99e74 message rather than hidden. Downstream, the provenance byproduct and statement digests recompute, and the T54 untracked-bytes and file-count figures match the tree exactly. The artifact rotation f5c48a3a to ac4ce59a is legitimate committed tree change with no suppressed mismatch: the report holds one attestation on one commit, and the one real cross-record disagreement (provenance subject still 8ee23a8d from a dirty 5b70052 candidate) is mechanically flagged by the S20-710 checker and disclosed in e464ed4. The premise that the drift is "the five repair commits" is inaccurate: the span is 182 commits and 43 surface files, mostly RW-070/075/080 work; that is a record-accuracy note, not a legitimacy problem. Residual items are one pre-existing P3 (a hard-coded dossier note asserting provenance agreement that does not hold) and two P4 record notes, none of which bears on S20-730 surface correctness. The checker itself and cargo were not executed (permission denied), and the minting worktree's source evidence record was outside my read scope, so the attestation's derivation is accepted on internal consistency rather than on direct inspection.
```

## Integrator addendum (2026-09-11, operator tree)

Finding 3 closed by direct inspection: the minting worktree's
`evidence/runtime/s20-720-release-candidate/evidence.json` matches the
committed attestation exactly on all four compared values (`commit`
`56bac4b877e5…`, `artifact_sha256` `ac4ce59a…`, `artifact_size_bytes`
2131797, `manifest_digest` `86e09320…`; `member_count` 14 both sides).
The derivation link is confirmed, not merely consistent.
