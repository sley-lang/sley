# Argus post-mint final: new S20-710 surfaces for the licensed candidate

Baseline verified: `git rev-parse HEAD` = `036730726eeb184bd30ff78d13df32be0cc49c80` (attestation-merge records on top of the mint). Mint commit `74bb0ba43203790399d05ff2fe167d6e46a9c6f1` (clean tree at mint). `git status --porcelain` is clean. Artifact `705a311df924ccf84c051216dbc8e432f78cec727f6952f4d69e3786b6b77512` (2187943 bytes, 15 members, manifest `e3943eed26edd51429179438d40bb24321d1205f7a65495fde06572a8acf3fc7`). Prior transcripts and the `ae16df8`/`fbaddbf1` record are preserved in history; the live report now describes the new candidate by label rotation (designed behavior, not deletion).

## What I reviewed

| Surface | State |
|---|---|
| license packaging | artifact members `LICENSE` (`cfc7749b…523d30`) + `NOTICE` (`e7151ea0…5a5dfa`) byte-match the approved roots; `LICENSES.json` records `APPROVED_OPERATOR_APACHE_2_0` with SPDX id and digests; no pending-license staging anywhere in the 15 members |
| history coverage | anchor `724a899` (L1); T54 `PASS_NO_HIGH_CONFIDENCE_FINDINGS`, 0 findings, 21 patterns, 5950 blobs / 380847122 bytes; zero deletions `facfa86..724a899`; mint + records commits covered by the candidate scan (1186+ tracked files) |
| SBOM/provenance drafts | CycloneDX root `Apache-2.0`, blocked-count `0`, candidate-bound to `74bb0ba`/`705a311d`; SPDX `licenseDeclared Apache-2.0`, `licenseConcluded NOASSERTION`, extracted `Apache-2.0` entry; namespace `urn:sley2:spdx:285c97b1…:705a311d…`; provenance subject `705a311d`, unsigned, 4 remaining blockers (license + reanchor dropped honestly); `check_standards_sbom_and_provenance` PASS |
| reproducibility | `MULTI_HOST_REPRODUCIBLE`, distinct_hosts 2, second_host ATTESTED; primary and secondary agree on all six identity fields (commit, artifact, size, members, manifest, toolchain `1.93.0 (083ac5135/254b59607)`); lab transfer hash-verified both ways (`1bc9b6bb…` bundle, `4a57b1aa…` attestation); old `ae16df8` attestations superseded by label rotation, preserved in git history |
| finding dispositions | register 291 obligations, states 53/3/189/46 unchanged (no license obligation existed there); dossier 34 entries / 24 evidenced / 10 gated, BLOCKED only on 46 open reviews + succession + fail-closed gates; `check_finding_register` + `check_decision_dossier` PASS |
| leakage negative controls | release + review suites green (106/106, 53/53) including the `greyforge`-marker guards over generated documents; owner string confined to `NOTICE`, audit prose, transcripts |

## Live command outputs

- `python3 scripts/generate_supply_chain_evidence.py --check` → `{"outputs": 2, "result": "PASS"}`
- `python3 scripts/check_supply_chain_audit.py` → `DEFERRED`, `root_license_text_approved: true`
- `python3 scripts/check_standards_sbom_and_provenance.py` → `PASS`
- `python3 scripts/check_release_candidate_packaging.py` → `PASS`
- `python3 scripts/check_reproducibility_and_independent_conformance.py` → `PASS`
- `python3 -m unittest discover -s bench/release/tests -t .` → 106/106 OK; `-s bench/review/tests` → 53/53 OK; `-s bench/invariant/tests` → 9/9 OK
- `cargo test --workspace --locked` → 48 result lines, all ok, 0 failures
- `make quick` → green through every checker except the preserved `check_error_symbol_registration --check` hold (owned-elsewhere rows, unchanged by this wave)

## Assessment

No new P0–P3. S20-710-full is NOT claimed: `standards_sbom`/`release_provenance`/`full_s20_710_complete` stay false pending full acceptance; succession, council, publication, and GA remain held; `ga_claimed=false` throughout.

VERDICT: PASS / SECTION: s20_710_pre_release_audit / FIELD: final_argus_disposition / SCOPE_SHA: 74bb0ba43203790399d05ff2fe167d6e46a9c6f1 / FINDINGS: 0_P0_0_P1_0_P2_0_P3
