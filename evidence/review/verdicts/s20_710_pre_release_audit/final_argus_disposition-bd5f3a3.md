# Argus pre-mint final: licensed candidate (license + secret re-review)

Baseline verified: `git rev-parse HEAD` = `bd5f3a381179002de80623fc1749172997e086dc` (short `bd5f3a3`, the L2 freeze). `git status --porcelain` is clean. Scope is the L2 tree: L1 (`724a899`, license landing + implementation) plus the re-anchor and regenerated evidence. Prior transcripts (`final_argus_disposition-21acb7f.md`, `final_vulcan_disposition-21acb7f.md`, `final_vulcan_confirmation-c703bd5.md`) stay scoped to `21acb7f`/`c703bd5` + anchor `db1bc62` and are preserved, not superseded.

## What I reviewed

| Artifact | State at bd5f3a3 |
|---|---|
| `LICENSE` (new root file) | official Apache-2.0 text, byte-identical to `https://www.apache.org/licenses/LICENSE-2.0.txt` (`cmp` clean, 11358 bytes LF-only), sha256 `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` |
| `NOTICE` (new root file) | identifies Sley 2.0, carries `Copyright 2026 Greyforge Labs` exactly, points at `LICENSE`, declares third-party licenses unrelicensed; sha256 `e7151ea0ee545a9edec91ecf963acefec4d6c2cfd92aa6080b1afe517d5a5dfa` |
| `Cargo.toml` / `oracle/scb1/pyproject.toml` | `Apache-2.0`; `cargo metadata --offline --no-deps`: 18 packages, one license value; with the oracle package the 19 first-party packages are consistent; registry dependencies untouched |
| `scripts/generate_supply_chain_evidence.py` | `HISTORY_ANCHOR = 724a899` (L1); `APPROVED_WORKSPACE_LICENSE` + `APPROVED_ROOT_LICENSE_FILES` digest pins; workspace disposition `APPROVED_OPERATOR_APACHE_2_0_ROOT_LICENSE` only when declaration and exact bytes both verify, else `BLOCKED_*`; inventory carries `root_license_sha256`/`notice_sha256` |
| `evidence/security/T52/pre-release-inventory.json` | `result PASS`, `blockers []`, anchor `724a899`, `license_text_files ["LICENSE","NOTICE"]`, digests match the installed files, 19 `APPROVED_…` + 32 `DECLARED_PERMISSIVE_PRE_RELEASE_REVIEW` |
| `evidence/security/T54/secret-scan.json` | `PASS_NO_HIGH_CONFIDENCE_FINDINGS`, 0 findings, 0 blockers, 21 patterns, history 5950 blobs / 380847122 bytes through L1, 1184 candidate files (1186 tracked minus the 2 excluded outputs, including `LICENSE` and `NOTICE`), manifest `2cef8eee8e9008cea178908ae86e1b7ab2ee61d64f82b3da32f784595c5f1e1e` |
| `scripts/check_supply_chain_audit.py` | pins the approved digests, the `["LICENSE","NOTICE"]` set, per-package declarations/dispositions, anchor and history counts; outcome `DEFERRED` with `root_license_text_approved: true` |
| history coverage | `git diff --diff-filter=D facfa86..724a899` empty; 813 commits reachable from the anchor; L2 adds only anchor constants and regenerated evidence, covered by the candidate scan |
| leakage negative controls | `grep -c "greyforge\|/home/\|file://"` = 0 in T52, T54, dossier, register; machine-summary carries 3 pre-existing `/home/greyforge/…` locator references also present at `facfa86` (out of scope, noted, not introduced here); owner string appears only in `NOTICE`, audit prose, and transcripts — never in leakage-governed JSON evidence |

## Compliance posture assessment

**License disposition honesty: sound and fail-closed.** Approval is pinned to exact bytes in two places (generator dispositions + checker expectations). A missing, added, or edited root license file returns every workspace disposition to `BLOCKED` and fails the deterministic-regeneration check; staging refuses with `PACKAGE_INTERNAL_INVARIANT`. Nothing concludes compatibility — `licenseConcluded` stays `NOASSERTION`.

**Secret-scan integrity: sound.** `--check` reproduces both documents; 21-pattern set unchanged in code; `test_secret_patterns` passes inside the 105/106 release run (sole failure is the stale-SBOM namespace pin, which only the post-mint smoke resolves).

**Prior findings:** the `21acb7f` Argus items closed in that scope; the `21acb7f`/`c703bd5` Vulcan items (pattern gap repaired to 21 patterns, PGP-armour declined with rationale, dirty-refusal pinned) are preserved and re-verified applicable — pattern count still 21, refusal paths live-tested. No new P0–P3 in this scope.

**Held out of scope:** SBOM/provenance approval (drafts pending re-derivation at mint + full acceptance), second-host attestation for the new candidate (old attestations stay bound to `ae16df8`), S20-710-full acceptance.

## Live command outputs

- `python3 scripts/generate_supply_chain_evidence.py --check` → `{"outputs": 2, "result": "PASS"}`, exit 0
- `python3 scripts/check_supply_chain_audit.py` → `{"machine_summary_registered": true, "release_sbom": false, "result": "DEFERRED", "root_license_text_approved": true, "t52_local_lock_inventory": "PASS", "t54_high_confidence_scan": "PASS"}`, exit 0
- `cmp LICENSE /tmp/opencode/LICENSE-2.0.upstream` → identical; `sha256sum LICENSE NOTICE` → the two pinned digests
- `cargo metadata --format-version 1 --offline --no-deps` → 18 packages, `['Apache-2.0']`
- `python3 -m unittest discover -s bench/review/tests -t .` → `Ran 53 tests ... OK`
- `python3 -m unittest discover -s bench/release/tests -t .` → 105/106, sole failure `test_the_current_pair_validates` (`namespace-not-attestation-bound`: tracked SBOM still binds the pre-license inventory digest; resolves when the smoke re-derives it — expected pre-mint staleness, not a license defect)

VERDICT: PASS / SECTION: s20_710_pre_release_audit / FIELD: final_argus_disposition / SCOPE_SHA: bd5f3a381179002de80623fc1749172997e086dc / FINDINGS: 0_P0_0_P1_0_P2_0_P3
