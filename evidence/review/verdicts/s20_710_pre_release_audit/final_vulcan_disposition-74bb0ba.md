# Vulcan post-mint final: new artifact surfaces for the licensed candidate

Baseline verified: `git rev-parse HEAD` = `036730726eeb184bd30ff78d13df32be0cc49c80`. Mint commit `74bb0ba43203790399d05ff2fe167d6e46a9c6f1`, artifact `705a311df924ccf84c051216dbc8e432f78cec727f6952f4d69e3786b6b77512`. `git status --porcelain` is clean. Scope: the minted artifact, its attestations, and the re-derived evidence.

## What I reviewed

| Surface | State |
|---|---|
| artifact identity | 16 tar entries (15 manifest members + `MANIFEST.json`); `LICENSE`/`NOTICE` bytes equal the approved roots; no `LICENSE-PENDING.txt`; two clean builds byte-agree locally (`REPRODUCIBLE`, no differing members) and the lab build byte-agrees remotely (all six identity fields equal) |
| dual-host integrity | bundle `1bc9b6bb…` hash-verified; lab clone detached at `74bb0ba`, clean; lab toolchain pinned `1.93.0` by the tree; lab smoke `PASS` with identical evidence; attestation `4a57b1aa…` hash-verified; merge yields `MULTI_HOST_REPRODUCIBLE`, 2 hosts, `ATTESTED` |
| packaging gates | staging refusal paths live-tested pre-mint (3× `PACKAGE_INTERNAL_INVARIANT` 72007); forbidden-content scan over staged real license bytes empty; `check_release_candidate_packaging` PASS with `candidate_*` bound to the tracked attestation |
| secret surface | 21-pattern set untouched; T54 clean on the new tree (5950 blobs, 1184 candidate files incl. `LICENSE`/`NOTICE`); lab-side T54 equally clean (lab smoke regenerated it over the same tree) |
| SBOM/provenance binding | namespace binds new inventory digest + new artifact; provenance subject matches; admission gates (HEAD-or-closure, clean REPRODUCIBLE attestation, digest agreement) all exercised for real during the smoke |
| third-state closure test | `test_builders_admit_exactly_the_live_closure_verdict` failed transiently at exact-mint HEAD on both hosts (`SbomError not raised`: candidate == HEAD is legitimate admission, neither closure nor advancement) and passes on every records-advanced checkout (106/106 now); no code change made — the test's live coupling resolved procedurally per the records-closure workflow |

## Live command outputs

- primary evidence: `commit 74bb0ba`, `REPRODUCIBLE`, `705a311d`, `2187943`, `15`, `e3943eed`
- lab evidence: identical six fields, toolchain `cargo 1.93.0 (083ac5135 2025-12-15)` / `rustc 1.93.0 (254b59607 2026-01-19)`
- merged report: `MULTI_HOST_REPRODUCIBLE`, `distinct_hosts 2`, blockers `[standards_sbom_and_provenance_s20_710_full, succession_thresholds_s20_640, council_reviews]`
- `make quick`: all checkers green except the preserved symbol-registration hold; suites 106/53/9; cargo 48-ok

## Assessment

No new P0–P4. The artifact surface is authentic on two hosts. Held: SBOM/provenance approval, S20-710-full acceptance, succession/council/publication/GA.

VERDICT: PASS / SECTION: s20_710_pre_release_audit / FIELD: final_vulcan_disposition / SCOPE_SHA: 74bb0ba43203790399d05ff2fe167d6e46a9c6f1 / FINDINGS: 0_P0_0_P1_0_P2_0_P3_0_P4
