# zjx_transport_readiness / vulcan_surface_review / Vulcan (surface and security lane) — delta round

Delta review of the records-only revision commit that answers the round-1 findings recorded in `vulcan_surface_review-161a2f3.md`. Read-only lane; this transcript is the only file written.

## 1. Scope verification

- `git rev-parse HEAD` -> `75b5491d567faf46df96aecff2e841424fb99e3d` (equals the requested revision SHA; proceeded).
- `git log --oneline -3`: `75b5491` (revision, records only) on top of `161a2f3` (witness commit) on top of `4168332`. `origin/main` is still `4168332`; nothing was pushed.
- Working tree clean except the untracked concurrent-lane transcript `evidence/review/verdicts/standards_sbom_and_provenance/nabu_architecture_review-70283ce.md` (not touched).
- Environment for checkers: `SLEY2_MASTER_GOAL=/home/dev/machineresearch/Sley2.0mastergoal.md`.

## 2. Inputs read in full

- `git diff --stat 161a2f3 75b5491` and the full diffs of: `docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md`, `evidence/validation/zjx-transport-readiness-v1.json`, `machineresearch/sley-2.0/machine-summary.json` (section `zjx_transport_readiness`), `evidence/security/T54/secret-scan.json`, `evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log`, `evidence/release/decision-dossier.json`.
- The three committed round-1 transcripts (`*-161a2f3.md`) as filed by the delta.
- `scripts/check_clean_room_boundary.py` (sentinel list and inventory, re-used for an independent scan), `scripts/generate_supply_chain_evidence.py` (re-used in memory for an independent recomputation).

## 3. Tool results (commands run and exact results)

| Command | Result |
|---|---|
| `git diff --stat 161a2f3 75b5491` | 10 files, 751 insertions, 53 deletions: closeout (36), decision-dossier (14), finding-register (128), three round-1 transcripts (159/304/109, new), T54 secret-scan (6), make-quick.log (2), readiness evidence record (26), machine-summary (20) |
| `git diff --stat 161a2f3 75b5491 -- 'crates/*/src' 'crates/*/Cargo.toml' Cargo.toml Cargo.lock docs/spec conformance scripts Makefile crates/sley-repo/tests` | empty: no crate source, manifest, lock, contract, fixture, script, or test changed |
| `sha256sum crates/sley-repo/tests/zjx_readiness_witness.rs` | `912f52edee6529d84c18ad0f6f48443acf32f07a5848a4ca0d4c88ba7e5902c2` (equals `witness.file_sha256` in the evidence record; unchanged since `161a2f3`) |
| `cargo test -p sley-repo --locked --test zjx_readiness_witness` | `ok. 10 passed; 0 failed; 0 ignored` |
| `git diff 161a2f3 75b5491 -- .../make-quick.log` | exactly one line: the `check_legacy_runner.py` JSON field `artifact_sha256` now reads `<legacy artifact digest redacted: clean-room sentinel, see scripts/check_clean_room_boundary.py>`; nothing else in the log changed |
| Independent sentinel scan of every file in the delta (checker's own `SENTINELS` tuple) | all clean except `evidence/release/decision-dossier.json` and `machineresearch/sley-2.0/machine-summary.json`, which carry sentinel indices 4 and 5 and are both on `SENTINEL_INVENTORY` (pre-existing, admitted) |
| `python3 scripts/check_clean_room_boundary.py` | `PASS`, `problems: []`, `S20_780_REGISTER_ACCEPTED` (exit 0). Round-1 F1 closed. |
| `git diff 161a2f3 75b5491 -- evidence/security/T54/secret-scan.json` | only the three candidate fields: files 1268 -> 1275, bytes 47,952,261 -> 48,196,755, manifest `6345b185...` -> `7d45697b...`; `result` `PASS_NO_HIGH_CONFIDENCE_FINDINGS`, `findings: []`, `blockers: []`, anchor unchanged |
| In-memory `build_outputs()` recomputation at this tree (no write) | `evidence/security/T52/pre-release-inventory.json MATCH`; `evidence/security/T54/secret-scan.json MATCH`. `git ls-files` = 1277; minus the generator's 2 output paths = 1275, equal to the recorded count. The scan therefore corresponds to this exact tree, i.e. it was regenerated after every other file was staged. |
| `python3 scripts/check_supply_chain_audit.py` | `{"result": "DEFERRED", "t52_local_lock_inventory": "PASS", "t54_high_confidence_scan": "PASS", "release_sbom": false, ...}` (exit 0; `DEFERRED` is the checker's pre-existing release-SBOM gate, unrelated to this amendment). Round-1 F2 closed. |
| `python3 scripts/check_finding_register.py` | `PASS`, `problems: []`, `S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING` (exit 0) |
| `python3 scripts/check_decision_dossier.py` | `PASS`, `problems: []`, 34 required items, `S20_750_DOSSIER_IMPLEMENTED_REVIEW_PENDING` (exit 0) |
| `python3 scripts/check_error_symbol_registration.py --check` | `PASS` (495 symbols, 315 codes, nothing unregistered) |
| `python3 scripts/check_declared_limits.py --check` | `PASS` (156 limits, 0 undocumented) |
| `python3 scripts/check_domain_tags_and_strings.py` | `PASS` (50 domains, `problems: []`) |
| `git diff --quiet 75b5491 -- .../vulcan_surface_review-161a2f3.md` | committed round-1 transcript is byte-identical to the one I wrote (SHA-256 `e650cf7e...74b7` both sides) |

## 4. Findings from round 1 checked against the delta

| Round-1 finding | Delta | Status |
|---|---|---|
| F1 [P2] `make-quick.log:823` carried the legacy freeze-digest sentinel; clean-room checker FAIL | The one offending value is replaced by an explicit redaction note; no other line changed; no other delta file introduces a sentinel outside the admitted inventory; `check_clean_room_boundary.py` PASS. Closeout ZT-12 and section 8 now state the correction honestly and attribute it to the round-1 review; the evidence record adds `round_1_corrections`. | Closed |
| F2 [P3] T54 record generated before the tree was final; supply-chain checker FAIL | Regenerated record reproduces byte-for-byte from this tree (independent in-memory recomputation MATCH; file count 1275 = 1277 tracked minus 2 generator outputs), which is only possible if it was the last staged step. `check_supply_chain_audit.py` exit 0 with `t54_high_confidence_scan: PASS`. Closeout section 8 states this ordering. | Closed |
| F3 [P4] ZT-14 claimed the machine summary named the witness commit/scope SHA while both were `PENDING_COMMIT` | Machine summary now pins `witness_commit` and `review_scope_sha` to `161a2f3480dfca3df22019a9df4e3e3e4a38674f`, records the three round-1 verdict tokens and transcript paths, `review_round: 1`; the evidence record pins `witness_commit` and `independent_review.scope_sha` likewise with `result: ROUND_1_REVISE_RECORDS_ONLY_DELTA_PENDING` and the per-lane verdicts. The ZT-14 row is restated to name the reviewed witness commit and describe the follow-up truthfully. | Closed |

## 5. Surface check of the delta itself

- No production, manifest, lock, contract, fixture, script, or test byte changed; the witness digest is unchanged and the witness still passes, so the transport claim reviewed in round 1 is untouched.
- The revised section-3 "cannot bypass" paragraph (answering another lane) is accurate against the code I read in round 1: `seal_mutated_conformance_pack_for_testing` and the public report structs are constructible values, no persistence or execution API consumes them, and the only ingress to a store or target is the `&[u8]`-taking importers with crate-private `PreflightedPack` / `promote_pack_objects`. The ZR-10 test-count correction (7 -> 6 `*_fails_before_promotion`) is a records fix and does not affect this lane.
- The register and dossier grow only by the lane obligations (379 -> 383 obligations; 3 -> 7 `OTHER`/unclassified dispositions, which are the REVISE tokens awaiting the delta round); both checkers PASS. No GA state, release disposition, or authority statement changed; the release stays BLOCKED and operator-held; nothing was pushed, minted, or published.
- The delta carries no new `sley2.` domain, `MAX_*` declaration, magic, MIME, feature, dependency, or public error symbol (three surface checkers PASS; the diff touches only Markdown and JSON records).

Nothing actionable remains for this lane.

```
VERDICT: PASS
SECTION: zjx_transport_readiness
FIELD: vulcan_surface_review
SCOPE_SHA: 75b5491d567faf46df96aecff2e841424fb99e3d
FINDINGS:
SUMMARY: The records-only revision answers all three round-1 Vulcan findings at the reviewed tree: the legacy freeze-digest sentinel in make-quick.log is replaced by an explicit redaction note and check_clean_room_boundary.py passes; the T54 secret scan reproduces byte-for-byte from this tree (independent in-memory recomputation, 1275 candidate files = 1277 tracked minus the two generator outputs), proving it was regenerated as the last staged step, and check_supply_chain_audit.py exits 0 with the T54 scan PASS; the machine summary and evidence record pin witness_commit and review_scope_sha to 161a2f3 with the round-1 verdicts and transcripts, and the ZT-14 row is restated truthfully. The delta touches no crate source, manifest, lock, contract, fixture, script, or test (the witness digest is unchanged and still passes 10/10), check_finding_register.py and check_decision_dossier.py pass, the error-symbol, declared-limit, and domain-tag checkers pass, and no authority beyond RESUME.md is claimed (HEAD remains unpushed, release BLOCKED and operator-held).
```
