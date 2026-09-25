# Vulcan post-mint final: re-qualified artifact surfaces

Baseline verified: `git rev-parse HEAD` = `79f460c5e1c1c7cf03e4a3e8f6d5f93de2b011d1`. Mint commit `baf9a4d3dff71cc1a0fb9d9c8f64b427a6ccee3f`, artifact `fba7b81fd3eb44fcfaa2b10df3ab4420eade71cd117350d8c8c745078e469613`. `git status --porcelain` is clean. Scope: the minted artifact, its attestations, the re-derived evidence, and the re-review of the mint-sequence repairs.

## What I reviewed

| Surface | State |
|---|---|
| artifact identity | 15 members; `LICENSE`/`NOTICE` bytes equal the approved roots; no pending file; two clean primary builds byte-agree (`REPRODUCIBLE`, no differing members) and the detached-worktree build byte-agrees (all six identity fields + toolchain equal) |
| dual-host integrity | worktree detached at `baf9a4d`, clean by construction; pinned toolchain builds; attestation `73b803cf…` hash-verified; merge yields `MULTI_HOST_REPRODUCIBLE`, 2 hosts, `ATTESTED`, report digest `bec9249f`, blockers `[standards_sbom_and_provenance_s20_710_full, succession_thresholds_s20_640, council_reviews]` |
| packaging gates | `check_release_candidate_packaging` PASS with `candidate_*` bound to the tracked attestation; machine-summary candidate binding updated to `baf9a4d`/`fba7b81f`/`42847438` (precedent-authorized candidate-binding update; `candidate_evidence_commit` untouched) |
| secret surface | 21-pattern set untouched; T54 clean on the new tree (5950 blobs, 1191 candidate files incl. the filed transcripts) |
| SBOM/provenance binding | namespace binds new inventory digest + new artifact; provenance re-derived over the merged report (statement `a444196d`); admission gates (HEAD-or-closure, clean REPRODUCIBLE attestation, digest agreement) all exercised for real during the smoke |
| closure-test repair re-review (`baf9a4d`) | test-only change, 10 added lines, no builder/checker/spec touched. The new branch fires only when `records-closure-not-advanced` (HEAD == attested commit, the mint moment) and candidate == live `git_head()` — exactly the state where the builder contract admits via condition (a). Every other non-closure verdict still asserts refusal with `records-closure-` in the detail; the `is_closure` branch is untouched. Verified: 106/106 at the pre-mint stale checkout (refusal pinned), 106/106 at exact-mint HEAD on the second host (admission pinned), 106/106 at every records-advanced checkout. No weakening: the suite now asserts strictly more contract behavior than before |

## Live command outputs

- primary evidence: `commit baf9a4d`, `REPRODUCIBLE`, `fba7b81f`, `2187943`, `15`, `42847438`
- secondary evidence: identical six fields + toolchain `cargo 1.93.0 (083ac5135 2025-12-15)` / `rustc 1.93.0 (254b59607 2026-01-19)`
- merged report: `MULTI_HOST_REPRODUCIBLE`, `distinct_hosts 2`, `second_host ATTESTED`
- `make evidence-refresh` → exit 0; `make quick` → exit 0, 96 PASS, 0 FAIL

## Assessment

No new P0–P4. The artifact surface is authentic on two hosts; the mint-sequence repairs are re-reviewed and sound. Held: SBOM/provenance approval, S20-710-full acceptance (clauses C, E, F, G), P-B wording, P-C reviews, signing/transparency, re-anchor, succession, independent review, council, publication, GA.

VERDICT: PASS / SECTION: s20_710_pre_release_audit / FIELD: final_vulcan_disposition / SCOPE_SHA: baf9a4d3dff71cc1a0fb9d9c8f64b427a6ccee3f / FINDINGS: 0_P0_0_P1_0_P2_0_P3_0_P4
