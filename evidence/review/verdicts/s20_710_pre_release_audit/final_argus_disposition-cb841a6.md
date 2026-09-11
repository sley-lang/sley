Baseline verified: `git rev-parse HEAD` = `cb841a60d04773fd6ff04181b12bdb9bf7cfac44`. No files were written.

## What I reviewed

| Artifact | State at cb841a6 |
|---|---|
| `docs/audits/S20_710_PRE_RELEASE_AUDIT.md` | Status BLOCKED, anchor `51863f7`, last edited 1419fcd (2026-09-03) |
| `evidence/security/T52/pre-release-inventory.json` | sha256 `bab48d8d…`, 51 packages (18 cargo ws / 30 cargo registry / 1 pypi ws / 2 pypi registry), 138 relationships, cargo lock `4b6af7…`, uv lock `cb9621…`, result BLOCKED, one blocker |
| `evidence/security/T54/secret-scan.json` | 0 findings, 8 patterns, 1059 candidate files, 499 history blobs, values not emitted |
| `evidence/release/sbom/{cyclonedx-1.6,spdx-2.3}.json` | digests match provenance `resolvedDependencies`; all 52 SPDX `licenseConcluded` = NOASSERTION; `LicenseRef-Proprietary` extracted text states the operator decision is pending |
| `evidence/release/provenance.json` | `signed:false`, `transparency_log:null`, 6 blockers enumerated, subject `8ee23a8…` @ commit `5b70052` |
| `evidence/runtime/s20-720-release-candidate/evidence.json` | commit `5b70052`, `working_tree_clean: false` |
| `evidence/release/reproducibility-report.json` | attests `f5c48a3…` @ `84bfa9c` (different artifact, 2026-09-05) |
| `machineresearch/sley-2.0/machine-summary.json` | `s20_710_pre_release_audit` counts reconcile with T52; `final_argus_disposition` = DEFERRED_FORGE_OAUTH_401; no prior Argus verdict in `reviews/verdicts.json` |

## Compliance posture assessment

**License disposition honesty: sound.** All 19 workspace components (18 crates + `sley2-scb1-oracle`) are named with `BLOCKED_MISSING_APPROVED_PROPRIETARY_LICENSE_TEXT`; 32 registry deps carry `DECLARED_PERMISSIVE_PRE_RELEASE_REVIEW` from Cargo metadata or a curated dict that the doc discloses as curated. Nothing concludes a license. The packaged candidate ships `LICENSE-PENDING.txt` and `LICENSES.json` with `root_license.status: PENDING_OPERATOR_APPROVAL`. The generator's `license_text_files()` regex would not accept `LICENSE-PENDING.txt` as a root license, so the blocker cannot be self-satisfied.

**SBOM/provenance unsigned state: sound.** `signed:false`, `transparency_log` null/false, `publication_authorized:false`, `ga_claimed:false` in every document; the checker fails if `signed` is not false. No host paths, no wall clocks.

**Audit boundary discipline: sound.** The doc disclaims acceptance record, legal opinion, standards SBOM, release provenance, and publication; the checker's success outcome is `DEFERRED`; the candidate's `release_check_gate` is `FAIL_CLOSED_NOT_IMPLEMENTED`.

**Candidate history anchoring and provenance records: two record defects** (details in FINDINGS). Neither changes the 0-findings secret result or the license posture; both break claims the landed records make about themselves.

## Assumptions

- I could not execute `scripts/check_supply_chain_audit.py` (approval was denied in this non-interactive session, not retried). The drift finding is derived from counts: tracked candidate files at HEAD minus the two excluded outputs = 1057; the committed scan records 1059; the only two untracked non-ignored files (`.forge/slices/rw-080-*.json`, mtimes 2026-09-07) predate the 2026-09-11 02:25 evidence generation and are included by `git ls-files --cached --others --exclude-standard`.
- The two P2 items are defects in landed records and their generators, not in the audit's stated claims; I have not rewritten any prior verdict.

VERDICT: REVISE
SECTION: s20_710_pre_release_audit
FIELD: final_argus_disposition
SCOPE_SHA: cb841a60d04773fd6ff04181b12bdb9bf7cfac44
FINDINGS:
P2 [record] evidence/security/T54/secret-scan.json `candidate_files_scanned`/`candidate_file_manifest_sha256` - the committed candidate manifest covers 1059 files but commit cb841a6 tracks 1057 candidate files; the two extra are the untracked `.forge/slices/rw-080-s1-1-slice-{5,6}-*.json` present in the working tree at generation time, so the manifest is not reproducible from the commit and `check_supply_chain_audit.py` (`make quick` line 90) will report drift from any clean checkout or once those files change.
P2 [implementation] scripts/generate_supply_chain_evidence.py:243 `candidate_files()` - the candidate set is defined by the working tree (`--others --exclude-standard`), not by the commit tree; the anchored evidence should be derived from `git ls-files --cached` (or `git ls-tree -r HEAD`) so the recorded manifest is commit-bound, with the same fix mirrored in the doc's "deterministic regeneration" claim.
P2 [record] evidence/release/provenance.json `externalParameters.make_target` / `subject` - the statement asserts `make_target: release-candidate-smoke` for subject `8ee23a8…` @ `5b70052`, but the S20-720 evidence it derives from records `working_tree_clean: false` (built with `--allow-dirty`), which the smoke target's `--require-clean` would have rejected; the listed byproduct `reproducibility-report.json` attests a different artifact (`f5c48a3…` @ `84bfa9c`), so the provenance misdescribes its own build invocation and pairs with a reproducibility attestation of another candidate.
P2 [implementation] scripts/build_release_provenance.py:171-183 and scripts/check_standards_sbom_and_provenance.py:285-306 - the provenance builder hardcodes `make_target` and neither carries nor requires `working_tree_clean`, and the checker does not cross-check the subject digest against the reproducibility report's attested artifact; add `working_tree_clean` to the predicate, refuse to derive from a dirty candidate, and check subject/attestation agreement.
P3 [record] docs/audits/S20_710_PRE_RELEASE_AUDIT.md:13-18,74-77 - "14 workspace crates and 22 registry crates" and "15 local packages" reflect the anchor-era workspace (14 members at 51863f7) and contradict both the landed T52 inventory / machine summary (18/30/19) and the doc's own line 59 ("nineteen components"); "draft revision 1" is stale against `contract_revision: 3` and status `S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING`.
P3 [contract] scripts/generate_supply_chain_evidence.py:18 `HISTORY_ANCHOR` - the history scan covers only the 499 blobs reachable from 51863f7, which is now 678 commits behind cb841a6; blobs introduced and later removed in that span are unscanned. This is disclosed and tracked (`release_candidate_history_reanchored: false`), so it is not a hidden claim, but the audit doc does not quantify the gap and re-anchoring remains a precondition for any release disposition.
P4 [record] scripts/check_supply_chain_audit.py:200 and evidence/review/finding-register.json obligation `final_argus_disposition` - both pin `DEFERRED_FORGE_OAUTH_401`; landing this verdict requires updating the checker pin, the machine summary field, and the register in one change set or `make quick` fails.
SUMMARY: The audit's compliance posture is honest where it matters: the root license blocker is real and correctly named for all 19 workspace components, no license is concluded anywhere, the SBOM pair and in-toto statement are explicitly unsigned and unlogged with enumerated blockers, secret values are never emitted, and every gate stays fail-closed with a `DEFERRED` rather than `PASS` outcome. The disposition cannot be PASS because two landed records overstate their own anchoring: the T54 candidate manifest embeds uncommitted working-tree files and so is not reproducible from cb841a6, and the draft provenance asserts a clean-tree smoke invocation for a candidate that was built dirty while citing a reproducibility attestation of a different artifact. These are record and generator defects, not license or secret findings; fixing the candidate-set definition, carrying `working_tree_clean` into the provenance with a subject/attestation cross-check, and refreshing the doc's stale counts would clear them. Re-anchoring history at the release candidate and the operator's root license text remain the outstanding preconditions for the full S20-710 pass, unchanged from the prior deferred state.
