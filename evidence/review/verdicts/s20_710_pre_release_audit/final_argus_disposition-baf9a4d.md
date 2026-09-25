# Argus post-mint final: re-qualified candidate surfaces

Baseline verified: `git rev-parse HEAD` = `79f460c5e1c1c7cf03e4a3e8f6d5f93de2b011d1` (attestation-merge records on top of the mint). Mint commit `baf9a4d3dff71cc1a0fb9d9c8f64b427a6ccee3f` (clean tree at mint). `git status --porcelain` is clean. Artifact `fba7b81fd3eb44fcfaa2b10df3ab4420eade71cd117350d8c8c745078e469613` (2187943 bytes, 15 members, manifest `428474380074fb16de3876f7f75676ebe19a7a0b2d8c90a2e222830034f789b4`). Prior transcripts and the `74bb0ba`/`705a311d` record are preserved in history; the live report now describes the new candidate by label rotation (designed behavior, not deletion).

## What I reviewed

| Surface | State |
|---|---|
| license packaging | artifact members `LICENSE` (`cfc7749b…523d30`) + `NOTICE` (`e7151ea0…5a5dfa`) byte-match the approved roots; no pending-license staging anywhere in the 15 members |
| history coverage | anchor `724a899` (L1); T54 `PASS_NO_HIGH_CONFIDENCE_FINDINGS`, 0 findings, 21 patterns, 5950 blobs / 380847122 bytes, 1191 candidate files; zero deletions `facfa86..724a899` |
| SBOM/provenance drafts | SPDX `licenseDeclared Apache-2.0`, `licenseConcluded NOASSERTION`, namespace `urn:sley2:spdx:285c97b1…`; SBOM components 51, `license_disposition_blocked 0`, `root_license_text_approved true`; provenance subject `fba7b81f`, statement `a444196d`, unsigned, candidate-bound to `baf9a4d`; `check_standards_sbom_and_provenance` PASS |
| reproducibility | `MULTI_HOST_REPRODUCIBLE`, distinct_hosts 2, second_host ATTESTED, report digest `bec9249f`; primary and secondary agree on all six identity fields plus toolchain `cargo/rustc 1.93.0 (083ac5135/254b59607)`; attestation `73b803cf…` hash-verified both ways; old `74bb0ba` attestations superseded by label rotation, preserved in git history |
| finding dispositions | register `FINDING_REGISTER_OPEN`, 291 obligations, states 53/3/189/46 unchanged; dossier 34 entries / 24 evidenced / 10 gated, BLOCKED only on 46 open reviews + succession + fail-closed gates; `check_finding_register` + `check_decision_dossier` PASS |
| leakage negative controls | release + review + invariant suites green (106/106, 53/53, 9/9) including the marker guards over generated documents |

## Mint attempts (honest count)

- Attempt 1 at `ed6135a`: mint builders green; `check_release_candidate_packaging` caught the third-state closure-test coupling (`SbomError not raised` at exact-mint HEAD). Records discarded, no candidate claimed. Root cause: the test's two-state model missed the legitimate candidate==HEAD admission (builder-contract condition (a)); the same transient was documented without repair in the `74bb0ba` lineage.
- Repair `baf9a4d` (test-only, 10 lines): the test now pins admission exactly when `records-closure-not-advanced` and candidate == live `git_head()`; refusal still covers every other non-closure verdict; no builder/checker/spec changed. Re-reviewed in the Vulcan final below.
- Attempt 2 at `baf9a4d`: all builders green, packaging PASS; only `check_reproducibility` flagged the expected pre-merge staleness (carried `secondary@74bb0ba`), resolved by the merge. One claimed primary mint.

## Live command outputs

- `python3 scripts/generate_supply_chain_evidence.py --check` → `{"outputs": 2, "result": "PASS"}`
- `python3 scripts/check_supply_chain_audit.py` → `DEFERRED`, `root_license_text_approved: true`, T52 PASS, T54 PASS
- `python3 scripts/check_standards_sbom_and_provenance.py` → `PASS`
- `python3 scripts/check_release_candidate_packaging.py` → `PASS`
- `python3 scripts/check_reproducibility_and_independent_conformance.py` → `PASS`, problems []
- release/review/invariant suites → 106/106, 53/53, 9/9 OK
- `cargo test --workspace --locked` → 48 result lines, all ok, 0 failures
- `make evidence-refresh` → exit 0; `make quick` → exit 0, 96 PASS results, 0 FAIL (first fully-green aggregate: the P-A symbol gate passes)

## Assessment

No new P0–P3. S20-710-full is NOT claimed: `s20_710_audit_complete` stays false; succession, council, publication, and GA remain held; `ga_claimed=false` throughout. This is candidate qualification, not acceptance or release readiness.

VERDICT: PASS / SECTION: s20_710_pre_release_audit / FIELD: final_argus_disposition / SCOPE_SHA: baf9a4d3dff71cc1a0fb9d9c8f64b427a6ccee3f / FINDINGS: 0_P0_0_P1_0_P2_0_P3
