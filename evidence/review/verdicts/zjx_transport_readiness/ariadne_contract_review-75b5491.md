# zjx_transport_readiness / ariadne_contract_review / Ariadne (contract) lane — delta round

Delta review of the records-only revision commit answering the round-1
findings recorded in `ariadne_contract_review-161a2f3.md`. Read-only lane;
this transcript is the only file written.

## Scope verification

- `git rev-parse HEAD` = `75b5491d567faf46df96aecff2e841424fb99e3d` (equals the requested SCOPE_SHA).
- `git log --oneline 161a2f3..HEAD` = exactly one commit, `75b5491 Qualification: ZJX readiness closeout revision after round-1 council review (records only)`.
- Working tree at start: clean except the pre-existing untracked `evidence/review/verdicts/standards_sbom_and_provenance/nabu_architecture_review-70283ce.md`. During this lane a second untracked file appeared, `evidence/review/verdicts/zjx_transport_readiness/nabu_architecture_review-75b5491.md` (the concurrent Nabu delta lane; not read, not touched). No tracked file was modified by this lane (verified after every batch).
- `SLEY2_MASTER_GOAL=/home/greyforge/machineresearch/Sley2.0mastergoal.md` exported for every checker.

## Delta read in full

`git diff --stat 161a2f3 75b5491`: 10 files, +751/-53:
`docs/audits/S20_ZJX_TRANSPORT_READINESS_CLOSEOUT.md` (36), `evidence/release/decision-dossier.json` (14), `evidence/review/finding-register.json` (128), the three round-1 transcripts (new, 159/304/109), `evidence/security/T54/secret-scan.json` (6), `evidence/validation/zjx-transport-readiness-logs-v1/make-quick.log` (2), `evidence/validation/zjx-transport-readiness-v1.json` (26), `machineresearch/sley-2.0/machine-summary.json` (20).

Records-only confirmed: `git diff --stat 161a2f3 75b5491 -- crates Cargo.toml Cargo.lock docs/spec conformance oracle scripts` is empty; blob ids of `REPOSITORY_PACK_V1.md`, `REPOSITORY_EXCHANGE_V1.md`, `SCB1.md`, `ERROR_CODES_V1.md`, and `crates/sley-repo/tests/zjx_readiness_witness.rs` are identical at both commits.

I read every hunk of the closeout, the evidence record, the machine-summary section, the secret-scan record, and the `make-quick.log` diff (with the 64-hex value masked in my terminal), and the register/dossier diffs; plus `scripts/build_finding_register.py` lines 40 to 49 and 258 to 300, `docs/spec/FINDING_REGISTER_V1.md` lines 186 to 205, and `scripts/generate_supply_chain_evidence.py` lines 287 to 306.

## Tool results

| Command | Result at 75b5491 |
|---|---|
| `check_clean_room_boundary.py` | **PASS**, problems [] (was FAIL at 161a2f3). |
| `check_supply_chain_audit.py` | **DEFERRED** (`t54_high_confidence_scan: PASS`, `t52_local_lock_inventory: PASS`, `release_sbom: false`), exit 0; the drift failure at 161a2f3 is gone. Run on the committed tree state (one pre-existing untracked file). |
| `generate_supply_chain_evidence.py --check` (later in the session) | reported drift on `secret-scan.json`; recomputed in memory: the only differing fields are `untracked_files_scanned` 1 -> 2 and `untracked_bytes_scanned` 15996 -> 24850, i.e. the Nabu delta transcript that appeared meanwhile. Tracked manifest: no drift (`git ls-files --cached` = 1277, minus the two `OUTPUT_PATHS` = 1275 = recorded `candidate_files_scanned`). |
| `check_finding_register.py` | PASS (`S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING`), problems []. |
| `check_decision_dossier.py` | PASS (`S20_750_DOSSIER_IMPLEMENTED_REVIEW_PENDING`), problems []; dossier stays BLOCKED. |
| `check_error_symbol_registration.py --check`, `check_declared_limits.py --check`, `check_repository_pack_spec.py`, `check_repository_exchange_spec.py`, `check_candidate_contract_freeze.py`, `check_domain_tags_and_strings.py` | all PASS. |
| `cargo test -p sley-repo --locked --test zjx_readiness_witness` | ok, 10 passed. |
| `sha256sum -c SHA256SUMS` (pack, exchange); `check_repository_pack_vector.py`; `check_repository_exchange_vector.py` | all OK / PASS. |
| `sha256sum` of `ariadne_contract_review-161a2f3.md` (working tree) vs `git show 75b5491:…` | identical (`fc0e3ca8…d0c2`): my round-1 transcript was recorded byte-for-byte. |

## Finding-by-finding delta check

| Round-1 finding | Delta | Verified | Status |
|---|---|---|---|
| F1 [P2] `make-quick.log:823` clean-room sentinel | line 823 now reads `"artifact_sha256": "<legacy artifact digest redacted: clean-room sentinel, see scripts/check_clean_room_boundary.py>"`; only that line changed in the log | `check_clean_room_boundary.py` PASS; closeout section 8 and ZT-12 row now disclose the round-1 defect and its correction | **Resolved** |
| F2 [P3] T54 drift, "repaired here" claim | `secret-scan.json` regenerated (1275 tracked files, manifest `7d45697b…`); closeout section 8 states the scan "is regenerated as the last staged step"; evidence record `round_1_corrections` records it | `check_supply_chain_audit.py` passes its T54 branch at the committed tree; tracked manifest has no drift. The remaining sensitivity is only to untracked files (recorded `untracked_*` counters), a pre-existing property of the generator outside this section's scope | **Resolved** |
| F3 [P4] ZR-10 "7 tests" | row now reads "`*_fails_before_promotion` (6 tests) and `object_verifier_failure_precedes_all_promotions`" | matches `lib.rs` (six matching tests) | **Resolved** |
| F4 [P4] ZT-14 wording / `PENDING_COMMIT` | ZT-14 row now names witness commit `161a2f3…` as the reviewed scope and says the follow-up pins the fields; evidence record `witness_commit` and `independent_review.scope_sha` = `161a2f3480dfca3df22019a9df4e3e3e4a38674f`; machine summary `witness_commit` and `review_scope_sha` = same; `review_round: 1`; the three lane fields carry the round-1 REVISE tokens and the three transcript paths | all values checked against the diff; my token `REVISE_0_P0_0_P1_1_P2_2_P3_2_P4` is recorded verbatim in the closeout section 10, the evidence record, the machine summary, and the register | **Resolved** |
| F5 [P3] section 3 "produced only by the owning functions" | paragraph restated: no persistence or execution API consumes an accepted-fact value; `seal_mutated_conformance_pack_for_testing` and the constructible report structs confer no acceptance; the only byte ingress is the importers; `PreflightedPack`/`promote_pack_objects` crate-private; clone API re-verifies under the maintenance guard and the incomplete-clone marker | this is exactly the argument I re-derived from the code in round 1 (`lib.rs:207/224/461/471/527`, `exchange.rs:369/407/1890`, `sley-protocol/src/server.rs:1838`) | **Resolved** |

Round-1 verdicts recorded: all three lane transcripts are tracked at 75b5491; the register carries three `PENDING`-state obligations with the round-1 REVISE dispositions and the section status `S20_ZJX_READINESS_READY_EXISTING_REVIEW_PENDING`; `open_reviews` lists them; the dossier's register entry reflects 383 obligations / 4 open reviews.

The contract position is unchanged from round 1 and re-verified: contracts, fixtures, symbols, limits, and the witness are byte-identical; the insertion point, non-dependency, and non-bypass statements stand.

## New item found in the delta

The revision added four string fields to the `zjx_transport_readiness` machine-summary section whose names contain `review`: `ariadne_contract_review_transcript`, `nabu_architecture_review_transcript`, `vulcan_surface_review_transcript`, and `review_scope_sha`. `build_finding_register.py::is_obligation` collects every string field whose name contains `review` or `disposition` unless it matches `SKIP_FIELD` (`(^|_)reviewer_role$|reviews$|_session_id$|_at$|_by$|_id$|_timestamp$|_note$`); a path or a SHA classifies as state `OTHER`. The register at 75b5491 therefore carries four new `OTHER` obligations for this section (`unclassified_dispositions` 3 -> 7 in the machine summary's `finding_register` block; dossier `states.OTHER` 3 -> 7). Per `FINDING_REGISTER_V1.md`: `FINDING_REGISTER_CLEAR` requires that no obligation is `OTHER`, and a section whose status ends in `COMPLETE` with an `OTHER` obligation is `REGISTER_COMPLETION_VIOLATION` and no register is written. So even after all three lanes carry the bare `PASS`, this section cannot move to a `…COMPLETE` status without the builder refusing, and the register cannot clear. No other section records transcript paths or a scope SHA under a `review`-bearing field name (checked: the only such fields in the register are these four). Fix: rename the four fields so `is_obligation` does not collect them (for example `transcript_ariadne` / `transcript_nabu` / `transcript_vulcan` and `scope_sha`, or a suffix `SKIP_FIELD` already skips such as `_note`), rebuild the register and dossier. Records-only; no contract impact. Severity P3: it does not affect the readiness claim but blocks the section's own terminal transition by construction.

## Findings

1. **[P3] [register hygiene, actionable]** `machineresearch/sley-2.0/machine-summary.json` section `zjx_transport_readiness`: the fields `ariadne_contract_review_transcript`, `nabu_architecture_review_transcript`, `vulcan_surface_review_transcript`, and `review_scope_sha` are collected by `scripts/build_finding_register.py` as review obligations and classify `OTHER` (four new unclassified dispositions in `evidence/review/finding-register.json`). Any `OTHER` obligation blocks `FINDING_REGISTER_CLEAR`, and a `…COMPLETE` section status with an `OTHER` obligation makes the builder refuse to write the register, so the section cannot terminate cleanly after the lanes PASS. Rename the fields outside the `review`/`disposition` collection pattern and rebuild the register and dossier.

Non-actionable observations: the T54 record's `untracked_*` counters make `generate_supply_chain_evidence.py --check` sensitive to any untracked file in the working tree (it drifted the moment a concurrent lane's transcript appeared); that is a pre-existing generator property, not introduced by this amendment, and the committed tree passes. The closeout's ZT-12 cell still carries the original "repaired here" sentence followed by the correction note; with section 8's disclosure that is an accurate history, not a misstatement.

```
VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3
SECTION: zjx_transport_readiness
FIELD: ariadne_contract_review
SCOPE_SHA: 75b5491d567faf46df96aecff2e841424fb99e3d
FINDINGS:
[P3] [register hygiene] machineresearch/sley-2.0/machine-summary.json:4364 - the revision's four new fields ariadne_contract_review_transcript, nabu_architecture_review_transcript, vulcan_surface_review_transcript and review_scope_sha contain "review" and are collected by scripts/build_finding_register.py as obligations that classify OTHER (register unclassified 3 -> 7); any OTHER obligation blocks FINDING_REGISTER_CLEAR and a COMPLETE section status with an OTHER obligation is REGISTER_COMPLETION_VIOLATION, so the section cannot terminate after the lanes PASS; rename the fields outside the review/disposition collection pattern (e.g. transcript_<lane>, scope_sha) and rebuild the register and dossier
SUMMARY: HEAD equals 75b5491; the revision is records-only (no crate, manifest, lock, spec, fixture, oracle, or script change; frozen contracts and the witness blob-identical). All five round-1 findings are answered and verified at the revision: the make-quick.log sentinel is redacted and check_clean_room_boundary passes; the T54 scan was regenerated last and check_supply_chain_audit passes at the committed tree; section 3's bypass paragraph now states the argument the code supports; the ZR-10 count reads six; ZT-14, the evidence record, and the machine summary pin witness_commit and review_scope_sha to 161a2f3 and record all three round-1 verdicts verbatim, with my transcript tracked byte-identically; check_finding_register and check_decision_dossier pass and the dossier stays BLOCKED. One new records-only item prevents a bare PASS: the four review-named metadata fields the revision added are collected as OTHER-state register obligations, which by the register contract blocks clearance and would make the section's own COMPLETE transition a REGISTER_COMPLETION_VIOLATION; renaming them resolves it. The transport-readiness claim, the insertion point, and the contract preservation are unchanged and re-verified.
```
