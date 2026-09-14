# Vulcan pre-mint final: licensed-candidate artifact surface

Baseline verified: `git rev-parse HEAD` = `bd5f3a381179002de80623fc1749172997e086dc` (short `bd5f3a3`, the L2 freeze). `git status --porcelain` is clean. Scope is the L2 tree. Prior transcripts stay scoped to their commits and are preserved.

## What I reviewed

| Artifact | State at bd5f3a3 |
|---|---|
| `scripts/build_release_candidate.py` | `ARTIFACT_INPUT_PATHS` gains `LICENSE` + `NOTICE` (12 entries); staging ships the installed files byte-identical and refuses (`PACKAGE_INTERNAL_INVARIANT` 72007) on unapproved file set, unapproved workspace dispositions/non-empty blockers, or digest mismatch; `LICENSE-PENDING.txt` staging removed; `LICENSES.json` records `APPROVED_OPERATOR_APACHE_2_0` with SPDX id and member digests; mint blockers drop the license item (3 remain) |
| staging dry-run (live) | 15 members (`LICENSE`, `LICENSES.json`, `NOTICE`, `SBOM.json`, `bin/sley`, conformance subsets, demo), manifest verifies, no pending file, staged `LICENSE` bytes equal the root file, `scan_forbidden_content` over the real `LICENSE`/`NOTICE` bytes with the production needles → `[]` |
| refusal paths (live) | blocked inventory → refused 72007; `["LICENSE","COPYING"]` file set → refused 72007; tampered `root_license_sha256` → refused 72007 |
| `scripts/generate_supply_chain_evidence.py` secret surface | 21-pattern set byte-unchanged; `candidate_files()` still tracked-only with outputs excluded; untracked files scanned separately; history anchor `724a899`, 5950 blobs / 380847122 bytes, 0 findings |
| history + candidate coverage | zero deletions `facfa86..724a899`; 1186 tracked files = 1184 scanned + 2 excluded outputs; `LICENSE`/`NOTICE` in the scanned set; candidate manifest `2cef8eee…5f1e1e` |
| `scripts/build_standards_sbom.py` | root/component expressions `Apache-2.0`, extracted info names the approved root license without owner/host strings (leakage negative-control test passes); `licenseConcluded`/`copyrightText` stay `NOASSERTION` |
| builder/checker gap controls | generator `--check` drift enforced and pinned by `check_supply_chain_audit`; packaging refusal errors carry partial evidence; determinism/mode/conformance/demo primitives covered by `test_packaging.py` (updated member set, same method count) |

## Adversarial assessment

**New-member smuggling: refused by construction.** A stray root license file (`COPYING`, edited `NOTICE`, extra text) breaks the exact `["LICENSE","NOTICE"]` pin in the generator (dispositions to `BLOCKED`), the checker digest pins, and the staging gate — three independent refusals, all live-tested above.

**Secret-shaped license bytes: absent.** The installed 11358-byte text and the `NOTICE` trip none of the 21 patterns and none of the packaging needles; the forbidden-content scan over the staged members is empty.

**Pattern-set regression: none.** The 21 patterns are untouched; `test_secret_patterns` passes; the `c703bd5` confirmation (admin/svcacct + GitLab/SendGrid fire with controls, PGP held with rationale) remains applicable — no pattern code changed.

**Prior findings:** no new P0–P4 in this scope. Held out of scope (unchanged): second-host attestation for the new candidate, SBOM/provenance approval, S20-710-full acceptance, succession/council/publication/GA.

## Live command outputs

- staging dry-run → `member_count: 15`, `forbidden-findings: []`, `pending-absent: True`, `license-bytes-ok: True`
- refusal probes → all three `PACKAGE_INTERNAL_INVARIANT 72007`
- `python3 scripts/generate_supply_chain_evidence.py --check` → `{"outputs": 2, "result": "PASS"}`, exit 0
- `python3 -m unittest discover -s bench/release/tests -t .` → 105/106 (sole failure the expected pre-mint SBOM-namespace staleness)
- `cargo check --workspace --locked --offline` → `Finished`, exit 0

VERDICT: PASS / SECTION: s20_710_pre_release_audit / FIELD: final_vulcan_disposition / SCOPE_SHA: bd5f3a381179002de80623fc1749172997e086dc / FINDINGS: 0_P0_0_P1_0_P2_0_P3_0_P4
