# Argus pre-mint final: re-qualification freeze (license + secret re-review)

Baseline verified: `git rev-parse HEAD` = `befd55f3156dd3a26645ac5b73f72804f6888bd7` (short `befd55f`, the P-A freeze). `git status --porcelain` is clean. Scope is the `befd55f` tree: the `74bb0ba` candidate lineage (L1 `724a899`, L2 `bd5f3a3`, mint/finals/attestation records) plus the acceptance-map records pass (`acf100e`, closure correction `96203eb`) and the P-A owner-scoped symbol amendments (`6a65331`, `520b47e`, T54 closeout `befd55f`). Prior transcripts stay scoped to their commits and are preserved, not superseded.

## What I reviewed

| Artifact | State at befd55f |
|---|---|
| `LICENSE` (root file) | official Apache-2.0 text, 11358 bytes LF-only, sha256 `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` (approved pin, unchanged) |
| `NOTICE` (root file) | 375 bytes, `Copyright 2026 Greyforge Labs`, sha256 `e7151ea0ee545a9edec91ecf963acefec4d6c2cfd92aa6080b1afe517d5a5dfa` (approved pin, unchanged) |
| `evidence/security/T52/pre-release-inventory.json` | `result PASS`, `blockers []`, anchor `724a899b1dfcc0ee8f3bf0b67f647d3156a24895`, `license_text_files ["LICENSE","NOTICE"]`, digests match the installed files, 19 `APPROVED_…` + 32 `DECLARED_PERMISSIVE_PRE_RELEASE_REVIEW`; byte-unchanged since `74bb0ba` (bound inventory intact) |
| `evidence/security/T54/secret-scan.json` | `PASS_NO_HIGH_CONFIDENCE_FINDINGS`, 0 findings, 0 blockers, 21 patterns, history 5950 blobs / 380847122 bytes through anchor `724a899`, 1189 candidate files, manifest `d2c75a306a6b01767ad82d2020d53a5b8899f53d4f8ec8be519c5ee6b24fa0cc`, 0 untracked files |
| `scripts/check_supply_chain_audit.py` | pins hold; outcome `DEFERRED` with `root_license_text_approved: true`, `t52_local_lock_inventory: PASS`, `t54_high_confidence_scan: PASS` |
| history coverage | zero deletions `facfa86..724a899` (anchor invariant); P-A changes touch 7 attestation-bound paths (6 specs + `refs.rs` test literals) — candidate-affecting, hence this re-qualification; no license/packaging/builder change |
| leakage negative controls | no `greyforge`/`/home/`/`file://` in leakage-governed JSON evidence beyond the 3 pre-existing `/home/dev/…` locator references also present at `facfa86` (out of scope, noted, not introduced here) |

## Compliance posture assessment

**License disposition honesty: sound and fail-closed.** Bytes, pins, and dispositions unchanged since the `74bb0ba` mint; nothing concludes compatibility — `licenseConcluded` stays `NOASSERTION`.

**Secret-scan integrity: sound.** `--check` reproduces both documents; 21-pattern set unchanged in code; T54 recut over the filed transcripts is committed in this scope.

**Prior findings:** the `bd5f3a3`/`74bb0ba` Argus items are preserved and re-verified applicable. No new P0–P3 in this scope.

**Held out of scope:** SBOM/provenance re-derivation (drafts pending at mint), second-host attestation for the new candidate (old attestations stay bound to `ae16df8`/`74bb0ba`), S20-710-full acceptance.

## Live command outputs

- `python3 scripts/generate_supply_chain_evidence.py --check` → `{"outputs": 2, "result": "PASS"}`, exit 0
- `python3 scripts/check_supply_chain_audit.py` → `{"machine_summary_registered": true, "release_sbom": false, "result": "DEFERRED", "root_license_text_approved": true, "t52_local_lock_inventory": "PASS", "t54_high_confidence_scan": "PASS"}`, exit 0
- `sha256sum LICENSE NOTICE` → the two pinned digests
- `python3 -m unittest discover -s bench/review/tests -t .` → `Ran 53 tests ... OK`
- `python3 -m unittest discover -s bench/release/tests -t .` → `Ran 106 tests ... OK`
- `python3 -m unittest discover -s bench/invariant/tests -t .` → `Ran 9 tests ... OK`

VERDICT: PASS / SECTION: s20_710_pre_release_audit / FIELD: final_argus_disposition / SCOPE_SHA: befd55f3156dd3a26645ac5b73f72804f6888bd7 / FINDINGS: 0_P0_0_P1_0_P2_0_P3
