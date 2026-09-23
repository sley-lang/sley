<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f073811914297505803f5b731cd4691e1623de84; role: nabu; field: nabu_architecture_review_revision_13; dispatched: 2026-09-23T09:01:14Z; duration_s: 545; process_exit_code: 0 -->
# Nabu Council review — protocol

Harness: claude-code
Reviewed checkpoint: f073811914297505803f5b731cd4691e1623de84

I ran the following checks for this review:
- **Scope.** `git rev-parse HEAD` returned f073811914297505803f5b731cd4691e1623de84, which matches the scope SHA. `git status --short` shows one untracked file outside this scope (`evidence/review/verdicts/complete_root_index_snapshot/ariadne_contract_review_revision_4-f073811.md`). `/tmp` inode use was 57%.
- **Diffs.** `git diff ab42a3a9..HEAD -- docs/spec/SMP1.md crates/sley-protocol docs/spec/SMP1_JSON_BRIDGE_V1.md docs/spec/SLEY_CLI_V1.md docs/spec/SESSION_HANDLE_PROFILE_V1.md` (6 files, +236/−26). I also diffed the SMP1-family checkers, the bridge table generator, ADR-0032 and ADR-0033 over the same range. `git log --oneline ab42a3a9..HEAD` lists 16 commits; the rev-13 work is in dda51a16 and a8b4cddb. `git diff --stat ab42a3a9..HEAD -- crates/` shows `sley-txn` is unchanged in this range.
- **Contract checkers, all exit 0:**
  - `check_smp1_contract.py`: PASS, revision 13, method_count 41, status `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING`.
  - `check_smp1_json_bridge_contract.py`: PASS, revision 10.
  - `check_cli_contract.py`: PASS, revision 8.
  - `check_session_handle_profile.py`: PASS, smp1_revision 13.
  - `check_complete_root_index_snapshot_profile.py`: PASS, revision 4.
- **Vector oracles.** Run through `uv run --offline --frozen --project oracle/scb1` from a python3 subprocess, because the bare interpreter has no `blake3` and exits 1 on import:
  - `check_smp1_vector.py`: exit 0, PASS (frames 3, mutations 4).
  - `check_smp1_json_bridge_vector.py`: exit 0, PASS (methods 41, rejections 36, vectors 5).
- **Bridge tables.** `generate_smp1_json_bridge_table.py --check --protocol-version 1|2|3` passed for all three versions. The SHA256SUMS files in `conformance/smp1/v1` and `conformance/smp1-json-bridge/v1`, `v2` and `v3` all verify, recomputed in Python.
- **Persistent-fuzz checkers, both exit 1** with `proof-record-predates-lane-change`:
  - `check_smp1_persistent_fuzz_slice.py`
  - `check_smp1_json_bridge_persistent_fuzz_slice.py`
  
  Both proof records have source_commit 7bcc5a89. `git diff --name-only 7bcc5a89 ab42a3a9` over the lane paths is empty. `git diff --name-only 7bcc5a89 HEAD` lists `crates/sley-protocol/src/server.rs`, `server_tests.rs` and `crates/sley-repo/src/index_cache.rs`. So this delta is what made the proofs stale.
- **Unit tests:**
  - `python3 -m unittest scripts/test_smp1_contract.py`: Ran 14, OK.
  - `test_cli_contract.py`: Ran 7, OK.
  - `test_session_handle_profile.py`: Ran 6, OK.
  - `python3 scripts/test_smp1_json_bridge_table.py`: 17 cases PASS.
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 2 passed, 0 failed (`workspace_open_under_version_1_never_carries_field_9`, `workspace_open_v2_discloses_only_the_materialized_head_snapshot`), exit 0.
- **Files read:**
  - `docs/spec/SMP1.md`: 1-80, 311-330, 380-392, 595-854
  - `crates/sley-protocol/src/server.rs`: 1095-1319, 2140-2210, 2440-2466, 2887-2931 (diff), 3187-3196, 3280-3316 (diff)
  - `crates/sley-protocol/src/lib.rs`: 568-580, 785-825
  - `crates/sley-protocol/src/server_tests.rs`: 81-103, 607-640, 2799-2826, 7381-7461
  - `crates/sley-txn/src/maintenance.rs`: 60-209
  - `crates/sley-txn/src/repository.rs`: 881-883, 2730-2754
  - `crates/sley-repo/src/index_cache.rs`: 108-126, 180-339, 545-604
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: 500-579
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`: 160-199
  - `docs/spec/SLEY_CLI_V1.md`: 1-45, 290-409
  - `docs/spec/SESSION_HANDLE_PROFILE_V1.md`: 1-40, 160-184
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md`: 1-50
  - `scripts/generate_smp1_json_bridge_table.py`: all
  - `scripts/check_smp1_persistent_fuzz_slice.py`: all
  - `scripts/fuzz_proof_record.py`: 150-182
  - `scripts/check_complete_root_index_snapshot_profile.py`: 170-199
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: 279-285, 584
  - `machineresearch/sley-2.0/machine-summary.json`: the `protocol` section, lines 1939-1945 and 2182
  - base protocol section via `git show ab42a3a9:…machine-summary.json`: rev-12 delta review PASS for all three lanes, status `S20_400_COMPLETE`

## Evidence checked

- **Server, version 1.** `workspace_open(body)` (`server.rs:2902-2913`) refuses a non-empty body with `PayloadInvalid` under every version. It loads the head through `accepted_head()`, which takes the shared maintenance lock and waits for it (`sley-txn/src/repository.rs:883`). It runs the probe only when `self.profile.protocol_version >= PROTOCOL_VERSION_V2` (`server.rs:2907`). Otherwise it emits `head_open_summary`: the eight `revision_summary_fields`, plus field 9 when the probe returns a value. `revision_summary` (204) keeps its own eight-field encoder. The legacy server (`Harness::new` → `Server::new`) offers only version 1, so it never gets field 9. The v1 test confirms the bytes equal `revision.read`, even with a warm cache.
- **Server, version 2.** The v2 test (VServer, v2 hellos) covers these cases:
  - Cold cache: the body equals `revision.read`, returned_entities is 1, nothing is marked omitted or truncated, and no cache file is written.
  - Unbound preimage: refused with `QUERY_SNAPSHOT_MISMATCH`, and the query path materialises the snapshot.
  - Warm cache: the record count is 9, fields 1-8 match byte for byte, and the tail is `[9, 32] || snapshot_id`, equal to the id `query.root` binds. A preimage rebuilt from the disclosed id is answered.
  - A body `[0]` is refused.
- **Server, version 3.** `Method::from_tag_versioned(201, 3)` resolves to `WorkspaceOpen`, because `introduced_in` is 1 and 201 is in `V3_ALL` (`lib.rs:568-575, 785-824`). `dispatch_admitted` has no v3 intercept for 201 (`server.rs:1221-1283`). So a v3 selection reaches the same `>= V2` gate and gets `open_summary`. No test covers 201 under v3.
- **Probe.** `acquire_shared_repository_maintenance_nonblocking` uses `try_lock_shared` (`maintenance.rs:142-146, 179-186`). It requires the existing lock directory and file and opens without `create`, so it never initialises the boundary. `cached_complete_root_snapshot_id` (`index_cache.rs:256-270`) reads the record, accepts it through `accept_cached`, and never falls back to a build or writes the record back. The sley-repo test `probe_reports_only_a_materialized_snapshot_and_never_builds` covers the cold, warm, objects-removed and corrupt-record cases, and confirms no write-back. The S20-300 stage checker limits probe callers to `server.rs`.
- **Rev-12 wording and the old server.** The revision-12 method-table row 201 said "repository path digest", while appendix A said "empty". The base server ignored the body entirely (`self.workspace_open()`). Section 4 (`SMP1.md:389-391`) already required `PROTOCOL_PAYLOAD_INVALID` for a body that does not decode as the frozen record, so the new refusal is a conformance correction. Other empty-request rows still ignore their bodies: 102, 103, 104, 210, 214 and 504 (`server.rs:1242-1257, 1265-1266, 1275`).
- **Bridge.** The bridge's generated tables carry only tag, name, family, reserved and owner. The row-201 column edit therefore does not change the v1, v2 or v3 table bytes, and all three `--check` runs pass. The bridge re-pin sentence (`SMP1_JSON_BRIDGE_V1.md:44-50`) and the checker pin (`check_smp1_json_bridge_contract.py:308`) agree.
- **Session-handle.** The session-handle profile has 201 only in its head-bound list (`:169-174`), and rev 13 does not change that list. The pin sentence (`:22-27`) matches the checker's line-anchored `SMP1_PIN`.
- **Revision history.** Revision 12's delta PASS on 2026-09-15 matches the base machine summary. `current_delta_review` is `{13, PENDING×3}` and the status is back to REVIEW_PENDING. ADR-0032 records revision 13. The appendix C grammar is unchanged.

## Findings

[P1] [version-gate/spec-code-divergence] crates/sley-protocol/src/server.rs:2907; docs/spec/SMP1.md:55-61,659,697-698,844-846; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:176-179 - The server's gate is `protocol_version >= PROTOCOL_VERSION_V2`. A negotiated version 3 selection (live: offered at server.rs:645, resolved by `from_tag_versioned`, and 201 is in `V3_ALL`) therefore gets the nine-field `open_summary` whenever the cache is warm. The contract says field 9 is for "`workspace.open` under a version 2 selection only" and names only the v1 and v2 bodies. The S20-300 owner sanctions the probe's one consumer "under a version 2 selection". Version 3 bytes for 201 are unspecified, and the stated "only" is violated. No test exercises 201 under v3. - Closure evidence: either change the gate to `== PROTOCOL_VERSION_V2`, or amend SMP1 appendix A/D, the history entry and S20-300 section 5 to say "version 2 and later selections (version 3 per NATIVE_TEST_ADMISSION_V1 appendix D)". In either case add a v3 server test asserting the chosen cold and warm bytes.

[P2] [stale-authority-pin] docs/spec/NATIVE_TEST_ADMISSION_V1.md:542-546; machineresearch/sley-2.0/machine-summary.json:2182; scripts/generate_smp1_json_bridge_table.py:27-28 - The v3 owner contract defines protocol version 3 as the union of SMP1's v1 and v2 tables, "`docs/spec/SMP1.md`, unchanged at revision 12". Revision 13 changed row 201 (its request column, the refusal of a non-empty body under every version, and the v2 response), but this consumer was not re-pinned. The status note lists only the S20-330, S20-420 and S20-430 re-pins. The generator comment was edited to "SMP1.md stays revision 13" beside a native contract that still says 12. No checker binds this pin. - Closure evidence: a pin sentence (or revision) in NATIVE_TEST_ADMISSION_V1 appendix D naming SMP1 revision 13 and its v3 effect. Also a checker that cross-checks that pin against SMP1's anchored Status line, the way `check_session_handle_profile.py:42-47` does.

[P3] [incomplete-repin] docs/spec/SLEY_CLI_V1.md:26,341-359; scripts/check_cli_contract.py:226 - The CLI's normative composition sentence still says every frame judgment comes from "`docs/spec/SMP1.md` revision 12". The re-pin to 13 was appended only as a bullet inside the "Revision 8 (2026-09-14)" history subsection, right after "The revision pins are SMP1 revision 12". The document now pins two SMP1 revisions. The checker accepts the substring anywhere in the file, so it cannot see the stale line 26. - Closure evidence: line 26 names SMP1 revision 13, the re-pin note sits outside the revision-8 history, and the checker anchors the composition sentence (as it already does for the bridge pin at :232).

[P3] [frozen-v1-overclaim] docs/spec/SMP1.md:63-64,315-319,844-846; docs/spec/SMP1_JSON_BRIDGE_V1.md:46-50; docs/spec/SLEY_CLI_V1.md:357-359; docs/spec/SESSION_HANDLE_PROFILE_V1.md:22-24; machine-summary.json:2182 - The contract says "revision 13 still serves version 1 exactly as before, byte for byte, including `workspace.open`" and "no version 1 byte changes". Appendix D says the row-201 amendment is "for the version 2 selection only". Each re-pin sentence says revision 13 "changes only the version 2 … response body". In fact, a version 1 request with a non-empty 201 body used to be answered (the base server ignored the body, and the rev-12 method table told clients to send a "repository path digest"). It is now `PROTOCOL_PAYLOAD_INVALID` (server.rs:2903-2905). The correction is justified by section 4 (:389-391), but it is an observable version 1 change, and it applies under every version. - Closure evidence: restate in each place as "no version 1 byte change for a conforming empty-body request; a non-empty 201 body, previously ignored contrary to section 4 and appendix A, now refuses under every version". Either state that the other empty-request rows (102, 103, 104, 210, 214, 504) still ignore bodies, or align them.

[P3] [grammar-convention] docs/spec/SMP1.md:641-645,693-694 - The appendix A preamble defines optionality only as the SSMC1 union `0:None | 1:Some`, which row 504 and appendix C `option(...)` use. `open_summary` introduces an optional record field encoded by omission (record count 8 or 9), written with an ad-hoc `-- field 9 optional` comment the conventions never define. The prose at :707-708 disambiguates it, but a codec author reading the preamble would encode field 9 as an option union. - Closure evidence: add a preamble convention for an omitted optional record field (the count drops and no option union appears), and write `open_summary` in that defined notation.

[P3] [evidence-binding/fuzz-proof-stale] scripts/check_smp1_persistent_fuzz_slice.py:130; scripts/check_smp1_json_bridge_persistent_fuzz_slice.py; bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:281-282 - Both SMP1-family persistent-fuzz slice checkers exit 1 at HEAD with `proof-record-predates-lane-change`. Their proof records (source 7bcc5a89) were fresh at base ab42a3a9 and went stale only because of this delta's `server.rs`, `server_tests.rs` and `index_cache.rs` changes. The gate record discloses this as "fuzz refresh not run". - Closure evidence: refreshed proof records whose source_commit includes dda51a16 and a8b4cddb, with both checkers PASS at the acceptance head.

[P4] [test-gap/determinism-note] crates/sley-protocol/src/server.rs:2919-2931; crates/sley-txn/src/repository.rs:883; docs/spec/SMP1.md:705-707,746-747 - The probe does run without waiting, but it runs after `accepted_head()` has already taken the shared lock and waited for it. The "contended → absence" branch is therefore reachable only in the window between the two acquisitions, and the method as a whole still waits. No server-level test holds an exclusive guard on a separate file descriptor, or shows that the lock file is never created. Appendix B's "equal frame lists over equal repository state produce equal answer sequences" and section 10's "byte-identical responses across runs" are not qualified for field 9, which depends on cache state. - Closure evidence: a server test with an exclusive guard held (expect 8 fields, and a bounded-time return from the probe), plus one sentence saying that v2 201 is deterministic given the accepted head plus derived cache state, and that field-9 absence is not a fact about the head.

## Assessment

The architectural direction is sound:
- **Ownership.** `open_summary` composes S20-390's frozen eight fields with S20-300's probe-supplied identity rather than inventing SMP1 semantics.
- **Separation.** `revision_summary` stays a separate encoder for caller-named revisions.
- **Fail-closed.** The probe cannot build, write, delete or initialise anything, and its only error is a guard naming another repository. The S20-300 stage checker enforces the single probe caller.
- **Evidence.** The frozen v1 frame and bridge vectors, their SHA256SUMS, and the v1/v2/v3 bridge tables all still verify. The two new server tests prove the v1 and v2 bytes, including that the disclosed id is exactly the one `query.root` binds.

On the requested ruling: pin-only re-pins, done as a stated sentence without an owner revision, are acceptable in principle for the JSON bridge and the session-handle profile. Neither owns the 201 body; the bridge carries it opaquely and its generated tables do not change. Both checkers anchor their pins. The CLI re-pin is acceptable in form but incomplete (stale line 26, and a substring-only checker). All three sentences also carry the version-1 overclaim above.

Revision 13 cannot be accepted as written for two reasons:
- **Version 3 gate.** The server answers `open_summary` under version 3, while the contract, and the S20-300 owner, say version 2 only. That is the version-gate check this review was asked to make for every version.
- **Stale v3 pin.** The v3-defining contract still composes SMP1 "unchanged at revision 12".

The remaining items are precision, grammar and evidence-freshness repairs.

VERDICT: REVISE_0_P0_1_P1_1_P2_4_P3_1_P4
SECTION: protocol
FIELD: nabu_architecture_review_revision_13
SCOPE_SHA: f073811914297505803f5b731cd4691e1623de84
FINDINGS: [P1] [version-gate/spec-code-divergence] crates/sley-protocol/src/server.rs:2907; docs/spec/SMP1.md:55-61,659,697-698,844-846; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:176-179 - `>= PROTOCOL_VERSION_V2` gives a live version 3 selection the nine-field open_summary on a warm cache, while SMP1 rev 13 and the S20-300 sanctioned-consumer text say version 2 selection only; v3 bytes for 201 are unspecified and untested - gate to == V2 or amend SMP1 appendix A/D, history and S20-300 section 5 to name version 2 and later (v3 per NATIVE_TEST_ADMISSION_V1 appendix D), plus a v3 cold/warm server test | [P2] [stale-authority-pin] docs/spec/NATIVE_TEST_ADMISSION_V1.md:542-546; machineresearch/sley-2.0/machine-summary.json:2182; scripts/generate_smp1_json_bridge_table.py:27-28 - the v3-defining contract still composes SMP1 "unchanged at revision 12" though rev 13 changed row 201 for every version; it was omitted from the re-pin set and no checker binds it - pin sentence or revision naming SMP1 rev 13 and its v3 effect, plus a status-line-anchored checker pin | [P3] [incomplete-repin] docs/spec/SLEY_CLI_V1.md:26,341-359; scripts/check_cli_contract.py:226 - CLI composition sentence still pins SMP1 revision 12 while an appended bullet inside the revision-8 history pins 13; the substring checker cannot see it - update line 26, move the note out of the rev-8 history, anchor the checker | [P3] [frozen-v1-overclaim] docs/spec/SMP1.md:63-64,315-319,844-846; docs/spec/SMP1_JSON_BRIDGE_V1.md:46-50; docs/spec/SLEY_CLI_V1.md:357-359; docs/spec/SESSION_HANDLE_PROFILE_V1.md:22-24; machine-summary.json:2182 - "version 1 exactly as before, byte for byte" / "version 2 selection only" / "changes only the version 2 response body" are false for a non-empty 201 body, previously ignored under v1 and now PROTOCOL_PAYLOAD_INVALID under every version - restate as a conformance correction applying to all versions and state or align the other empty-request rows | [P3] [grammar-convention] docs/spec/SMP1.md:641-645,693-694 - appendix A defines optionality only as the SSMC1 option union; open_summary's omission-encoded field 9 uses an undefined `-- field 9 optional` notation - add a preamble convention for omitted optional record fields and use it | [P3] [evidence-binding/fuzz-proof-stale] scripts/check_smp1_persistent_fuzz_slice.py:130; scripts/check_smp1_json_bridge_persistent_fuzz_slice.py; bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:281-282 - both SMP1-family fuzz slice checkers exit 1 (proof-record-predates-lane-change); the proofs were fresh at ab42a3a9 and were staled by this delta - refreshed proof records covering dda51a16/a8b4cddb with both checkers PASS | [P4] [test-gap/determinism-note] crates/sley-protocol/src/server.rs:2919-2931; crates/sley-txn/src/repository.rs:883; docs/spec/SMP1.md:705-707,746-747 - the contended-absence branch is reachable only after the head load has already waited for the lock, no server test holds an exclusive guard or proves no lock creation, and the determinism statements are not qualified for cache-dependent field 9 - exclusive-guard server test plus one qualifying sentence
SUMMARY: SMP1 revision 13's composition is architecturally sound: S20-390 owns fields 1-8, S20-300 owns the read-only probe behind field 9, the probe fails closed, and all frozen SMP1 and bridge vectors, sums and generated tables still verify, as do the new v1/v2 server tests. The server's `>= V2` gate gives protocol version 3 an `open_summary` that the contract and the S20-300 owner restrict to version 2, and the v3-defining NATIVE_TEST_ADMISSION_V1 still pins SMP1 revision 12. Pin-only re-pins are acceptable as stated sentences for the bridge and session-handle contracts, but the CLI re-pin is incomplete, and all three sentences repeat an inaccurate version-1-unchanged claim. The grammar notation needs a defined convention, and two SMP1-family fuzz proof records went stale because of this delta.
