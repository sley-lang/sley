# Council verdict (delta round): section `zjx_transport_readiness`, field `nabu_architecture_review`, role Nabu (architecture)

Delta review of the owner's revision commit answering my round-1 transcript
`nabu_architecture_review-161a2f3.md`. Read-only; this transcript is the
only file written.

## 1. Scope verification

- `git rev-parse HEAD` -> `75b5491d567faf46df96aecff2e841424fb99e3d` (equals the requested SCOPE_SHA; branch `main`).
- `git status --short` -> one untracked file only, another lane's
  `standards_sbom_and_provenance` transcript (not touched, not read).
- `git log --oneline 161a2f3..HEAD` -> exactly one commit, `75b5491
  Qualification: ZJX readiness closeout revision after round-1 council review (records only)`.

## 2. Inputs read in full

- `git diff 161a2f3 75b5491` for `docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md`
  (four hunks: section 3 sentence, ZR-10 count, section 8 note, ZT-12 and
  ZT-14 rows, section 10 round-1 record), `evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log`
  (one line), `evidence/validation/zjx-transport-readiness-v1.json`,
  `machineresearch/sley-2.0/machine-summary.json` (section
  `zjx_transport_readiness` and the register block), `evidence/security/T54/secret-scan.json`.
- My round-1 transcript, for the three findings being checked.

## 3. Tool results

| Command | Result |
|---|---|
| `git diff --stat 161a2f3 75b5491` | 10 files, +751/-53: the closeout, decision dossier, finding register, the three round-1 transcripts (now tracked), T54 secret scan, `make-quick.log`, the evidence record, machine summary |
| same, restricted to `crates/*/src`, `crates/*/tests`, `Cargo.toml`, `Cargo.lock`, `crates/*/Cargo.toml`, `docs/spec`, `conformance`, `oracle`, `scripts` | empty: records and docs only, as claimed |
| `sha256sum crates/sley-repo/tests/zjx_readiness_witness.rs` | `912f52ed…5902c2`, unchanged from round 1 and equal to the evidence record |
| `cargo test -p sley-repo --locked --test zjx_readiness_witness` | `10 passed; 0 failed` |
| `python3 scripts/check_clean_room_boundary.py` | **PASS**, `problems: []` (round 1: FAIL on `make-quick.log`) |
| `python3 scripts/check_supply_chain_audit.py` | `t54_high_confidence_scan: PASS`, `t52_local_lock_inventory: PASS`, overall `DEFERRED` (pre-existing `release_sbom: false`, unrelated to this amendment) |
| `python3 scripts/generate_supply_chain_evidence.py --check` | `{"outputs": 2, "result": "PASS"}`: the tracked T54 scan matches the tracked set at HEAD (1,275 files, manifest `7d45697b…`) |
| `python3 scripts/check_finding_register.py` | PASS, `problems: []` |
| `python3 scripts/check_decision_dossier.py` | PASS, `problems: []`, 34 required items |
| sentinel scan (checker's own `SENTINELS`) over every file the revision touched | zero hits in all files except `decision-dossier.json` and `machine-summary.json`, both of which are in the checker's `SENTINEL_INVENTORY` (pre-existing, permitted); the three now-tracked transcripts carry none |
| `grep -c "fn .*_fails_before_promotion" crates/sley-repo/src/lib.rs` | 6 (matches the corrected ZR-10 row) |

All checkers ran with `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md`.

## 4. Round-1 findings against the delta

### F1 (P2, evidence): `make-quick.log:823` clean-room sentinel; clean-room gate failing; ZT-12 misreported

- Line 823 now reads `"artifact_sha256": "<legacy artifact digest redacted: clean-room sentinel, see scripts/check_clean_room_boundary.py>"`; the rest of the `check_legacy_runner.py` block is intact, so the log still shows what the step reported.
- `check_clean_room_boundary.py` PASSes at HEAD with an empty problem list; nothing else the revision tracked introduces a sentinel.
- ZT-12 row now records the correction explicitly, attributing it to the round-1 finding and stating that the checker passes at the revision; section 8 carries the same note. The evidence record adds `round_1_corrections` naming the redaction and the T54 regeneration.
- The T54 scan (which Ariadne and Vulcan flagged as regenerated before final staging) was regenerated last: `generate_supply_chain_evidence.py --check` PASSes against HEAD's tracked set and `check_supply_chain_audit.py` reports the scan PASS.
- **Closed.**

### F2 (P4, record): `witness_commit: PENDING_COMMIT`; ZT-14 overstated

- Machine summary `zjx_transport_readiness.witness_commit` and
  `review_scope_sha` are both `161a2f3480dfca3df22019a9df4e3e3e4a38674f`;
  the three lane fields carry the round-1 REVISE tokens with transcript
  paths; `review_round: 1` and a round-1 note are recorded.
- Evidence record: `witness_commit` pinned, `independent_review.scope_sha`
  pinned, `round_1` block with each lane's verdict and transcript.
- ZT-14 row restated: names the witness commit as the reviewed scope and
  says the records-only follow-up pins `witness_commit`, `review_scope_sha`,
  and the lane verdicts, which is now what the summary contains.
- Observation (not actionable): the section's `note` still describes the
  pinning as future tense; it is a description of the mechanism and no
  longer contradicts the data. The delta-round verdicts will need the same
  follow-up to pin round 2, which the closeout's section 10 already says.
- **Closed.**

### F3 (P4, record): closeout section 3 "produced only by the owning functions"

- Restated as: no persistence or execution API consumes an
  `AcceptedRepositoryPack`, `AcceptedRepositoryExchange`, `ImportReport`, or
  `ExchangeImportReport`; the public
  `seal_mutated_conformance_pack_for_testing` builds a pack value and the
  report structs are constructible, but neither confers acceptance; bytes
  reach a store or target only through the `&[u8]` importers;
  `PreflightedPack` and `promote_pack_objects` are crate-private; the clone
  API re-verifies every receipt under the exclusive maintenance guard and
  the incomplete-clone marker.
- This is exactly the barrier I derived from the code in round 1 (section
  4.3 of that transcript) and it is accurate against `lib.rs:413-545`,
  `exchange.rs:1358-1490` and `:1885-1980`, and `repository.rs:2493-2600`.
- **Closed.**

## 5. Architecture position, restated for this round

Unchanged from round 1 and unaffected by a records-only commit: the
insertion point is `import_conformance_pack` (`lib.rs:413`),
`preflight_repository_exchange` (`exchange.rs:1439`), and
`import_repository_exchange` (`exchange.rs:1885`), fed by
`export_conformance_pack` (`lib.rs:343`) and `export_repository_exchange`
(`exchange.rs:954`). A composition layer over those symbols depends on
`sley-repo` and is depended on by nothing in the kernel (ARCHITECTURE.md
dependency law, ADR-0003, ADR-0025; 27-crate locked normal closure with no
transport, compression, network, or loader crate), and it cannot bypass
validation because only `&[u8]` enters the importers, the preflight and
promotion halves are crate-private, and the `sley-txn` clone phases
re-verify under the maintenance guard and marker. The witness is unchanged
by digest and still passes; the test-only framing remains outside every
library, binary, and feature.

## 6. Findings

None remaining. The other closeout items I examined in round 1
(resource-boundary map, identity comparisons, prohibited-expansion scan,
clean-room redaction of the amendment copy, release-impact map, separate GA
disposition) are untouched by the revision and stand.

```
VERDICT: PASS
SECTION: zjx_transport_readiness
FIELD: nabu_architecture_review
SCOPE_SHA: 75b5491d567faf46df96aecff2e841424fb99e3d
FINDINGS:
SUMMARY: Records-only revision (no crates/*/src, tests, manifests, lock, contracts, fixtures, or scripts changed; witness digest and 10/10 result unchanged). All three round-1 findings are closed: make-quick.log:823 is redacted and scripts/check_clean_room_boundary.py PASSes with no problems while no newly tracked file carries a sentinel; the T54 scan was regenerated last and generate_supply_chain_evidence.py --check and check_supply_chain_audit.py confirm it matches HEAD; witness_commit and review_scope_sha are pinned to 161a2f3 in both the machine summary and the evidence record with the ZT-14 row restated; the closeout's section 3 non-bypass sentence now states the barrier the code actually provides (no persistence/execution API consumes the accepted or report types; only &[u8] enters the importers; PreflightedPack/promote_pack_objects crate-private; clone API re-verifies under the guard and marker). check_finding_register.py and check_decision_dossier.py PASS. The architecture verdict from round 1 stands: the insertion point is import_conformance_pack / preflight_repository_exchange / import_repository_exchange, a composition layer on them is neither a kernel dependency nor a validation bypass, and the closeout's documentation-and-test-only claim holds.
```
