# Vulcan surface review, REQ-02 second confirmation — carry-over to the 9115bd0 re-mint (SCOPE_SHA 1db97b1)

Read-only; nothing written. HEAD verified equal to SCOPE_SHA at dispatch.

Those three are pre-existing pointers to external machineresearch docs (same count at e464ed4), not the invocation leak; `provenance.json` carries the flag-only invocation string. Review complete.

# Vulcan surface review, confirmation re-review of the second re-mint (delta e464ed4..1db97b1)

Baseline verified: `git rev-parse HEAD` = `1db97b16a7b21ed305ded853525785099a42a31d`. Read-only; nothing written.

## Assumptions and method

- Python and `cargo`/`rustc` execution were denied by the permission mode, so the S20-730 checker was **not executed** and the live toolchain was **not observed**. `history_problems` (`scripts/check_reproducibility_and_independent_conformance.py:142-187`) is unchanged; I recomputed its three git predicates inline against the exact `ARTIFACT_INPUT_PATHS` surface (`scripts/build_release_candidate.py:34,40-49`). All digests were recomputed with `jq -S --indent 2` + `sha256sum`, matching the builder's canonical form as verified last round.
- The minting worktree's S20-720 `evidence.json` was not inspected (outside my read scope, as before). The operator-tree `dist/` artifact is `8ee23a8d` / 2130840 B and `evidence/runtime/` is gitignored local state (`.gitignore:5-6`); neither is the attestation's source, so the derivation link is again accepted on internal consistency plus the `1db97b1` commit-message record. The integrator closed the equivalent link by direct inspection last round.
- The intermediate `dbff612` / `ace11bbe` build: I could not observe it (never committed, never attested). I verified the stated cause is structurally true: `MANIFEST.json` embeds the commit hex (`build_release_candidate.py:152,448`), so any commit change rotates the artifact identity even with byte-identical members. The +2 B delta (2131799 vs 2131797) is consistent with that.

## Carry-over checks (all PASS)

| Check | Result |
|---|---|
| Sole attestation names `9115bd0` / `ce87e6cc` / 2131792 B / manifest `b004b02f` / 14 members / `differing_members: []` / `working_tree_clean: true` | yes (`reproducibility-report.json:3-19`) |
| `merge-base --is-ancestor 9115bd0 HEAD` | true (1 commit behind, `1db97b1`) |
| `git diff --name-only 9115bd0 HEAD -- <surface>` | **0 files** |
| `git status --porcelain -- <surface>` | **0 entries**; full porcelain is only the two untracked `.forge/slices/rw-080-*` files, off-surface |
| `1db97b1` file set | 7 files: `evidence/release/{decision-dossier,provenance,reproducibility-report}.json`, both SBOMs, T54 scan, `machine-summary.json`. No source, checker, or contract touched, as the message claims |
| `9115bd0` surface change | only `scripts/build_release_candidate.py` (+13 lines, invocation recording, flags-only string). Confirms the "trailed by one file" premise |
| `report_digest` recompute | `a8e24937…` exact |
| Provenance byproduct digest of report | `16d52ac8…` = `sha256sum` of on-disk file |
| Provenance byproduct digest of conformance report | `64572a64…` exact |
| `statement_digest` recompute | `94146281…` exact |
| Provenance subject | `ce87e6cc`, commit `9115bd0` in both `externalParameters` and `resolvedDependencies`; `invocationId` = manifest `b004b02f`; invocation string contains no path |
| Resolved-dependency digests (Cargo.lock, uv.lock, T52 inventory, both SBOMs) | all 5 exact |
| SPDX namespace | `urn:sley2:spdx:<T52 inventory bab48d8d>:<ce87e6cc>`, checksum `ce87e6cc` |
| CycloneDX | hash `ce87e6cc`, `sley2:commit` `9115bd0`, `sley2:manifest-digest` `b004b02f`, signed/publication false |
| Dossier | commit `9115bd0` (:37), SHA-256 `ce87e6cc` (:284); prior P3 note is rewritten to "the provenance subject must name it ... it is not assumed to repeat it" (`build_decision_dossier.py:315`), and the assertion is now actually true |
| Register `release_candidate_packaging.candidate_*` | all 10 fields re-bound to `9115bd0` / `ce87e6cc` / 2131792 / `b004b02f` / 1.93.0 toolchain; `attested_commit` `9115bd0`, `p1_open_count: 0` |
| T54 | `candidate_files_scanned: 1087` = 1089 tracked − 2 self-exclusions; `untracked_bytes_scanned: 4483` = the two slices (unchanged from last round) |
| Toolchain | attested strings identical to the 56bac4b attestation and consistent with `rust-toolchain.toml` `1.93.0`; live not observed |
| Supersession honesty | prior identity (`ac4ce59a`, 2131797, `86e09320`, `56bac4b`) recorded in the `1db97b1` message and in file history; intermediate `ace11bbe` disclosed with cause, never attested |
| Blockers, `ga_claimed`, `publication_authorized` | 5 blockers retained; both false |

```
VERDICT: PASS
SECTION: reproducibility_and_independent_conformance
FIELD: vulcan_surface_review
SCOPE_SHA: 1db97b16a7b21ed305ded853525785099a42a31d
FINDINGS:
[P4] [record] machineresearch/sley-2.0/machine-summary.json (reproducibility_and_independent_conformance.blockers) - lists 4 blockers while evidence/release/reproducibility-report.json:21-27 lists 5 (section omits succession_thresholds_s20_640); pre-existing at e464ed4, not compared by the checker, not introduced by the carry-over
[P4] [record] evidence/release/reproducibility-report.json:5-11 - attestation values are digest-bound and agree with provenance, both SBOMs, dossier, register, and the 1db97b1 message, but derivation from the minting worktree's S20-720 evidence.json was again not directly inspected this session (read scope); live cargo/rustc also not observed (denied); no drift signal exists
SUMMARY: The P1-cured state carries over honestly to the second re-mint. The sole primary attestation names 9115bd0 and artifact ce87e6cc (2131792 B, manifest b004b02f, 14 members, no differing members, clean tree); recomputed inline, all three history_problems predicates are empty (9115bd0 is an ancestor one records-only commit behind HEAD, zero artifact-surface files changed 9115bd0..HEAD, zero uncommitted surface entries, with only the two off-surface untracked RW-080 slices in the tree). The one surface file that made 56bac4b trail (the builder's invocation recording) is exactly the 9115bd0 change and records flags only, so the path-leak fix is confirmed on the tracked provenance. report_digest, the provenance byproduct and statement digests, all five resolved-dependency digests, the SPDX namespace and checksum, the CycloneDX hash and properties, the dossier entries, and every register candidate_* field agree with the new attestation, and the prior-round P3 dossier note is cured and now true. Supersession is disclosed rather than hidden: the 1db97b1 message records the prior ac4ce59a identity and the never-attested intermediate ace11bbe with a cause that is structurally correct (MANIFEST.json embeds the commit hex). Residuals are two P4 record notes, one pre-existing blocker-list mismatch outside carry-over scope and the same unverified-derivation caveat as last round; the checker and live toolchain were not executed because permission was denied.
```
