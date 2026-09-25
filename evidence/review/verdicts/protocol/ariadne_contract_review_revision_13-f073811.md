<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f073811914297505803f5b731cd4691e1623de84; role: ariadne; field: ariadne_contract_review_revision_13; dispatched: 2026-09-23T08:55:06Z; duration_s: 779; process_exit_code: 0 -->
# Ariadne Council review — protocol

Harness: claude-code
Reviewed checkpoint: f073811914297505803f5b731cd4691e1623de84

**Scope.** `git rev-parse HEAD` returned `f073811914297505803f5b731cd4691e1623de84`, matching SCOPE_SHA. The tree was clean at the start.

**Git commands run:**
- `git log --oneline ab42a3a9..HEAD`: 16 commits. The protocol delta is `dda51a16` (Rust) and `a8b4cddb` (specs).
- `git diff ab42a3a9..HEAD` over the five named paths: SMP1.md +54/−?, server.rs +81, server_tests.rs +112, bridge +8, CLI +3, session profile +4.
- `git diff` of `crates/sley-repo/src/index_cache.rs` (the probe), `COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`, ADR-0032, ADR-0033, and the changed checkers.
- `git log -S` to date when version 3 entered `negotiate_versioned`: `116d031e`, 2026-09-16.

**Checkers run (exit code and result line):**
- `check_smp1_contract.py`: 0, PASS, revision 13.
- `check_smp1_json_bridge_contract.py`: 0, PASS, revision 10.
- `check_session_handle_profile.py`: 0, PASS, `smp1_revision` 13.
- `check_cli_contract.py`: 0, PASS, revision 8.
- `check_cli_rules.py`: 0, PASS, 46 method tags audited.
- `check_required_contract_index.py`: 0, PASS.
- `check_complete_root_index_snapshot_profile.py`: 0, PASS, revision 4.
- `generate_smp1_fixtures.py --check`: 0, PASS, no drift.
- `generate_smp1_json_bridge_table.py --check`: 0, PASS.
- Vector checkers with system `python3`: exit 1 each, `ModuleNotFoundError: blake3`. I re-ran them with the existing `oracle/scb1/.venv/bin/python` (what the Makefile uses, without writing anything):
  - `check_smp1_vector.py`: 0, PASS (3 frames, 4 mutations).
  - `check_smp1_json_bridge_vector.py`: 0, PASS (41 methods, 5 vectors, 36 rejections).
  - `check_entity_read_vectors.py`: 0, PASS (23 cases).
  - `check_complete_root_index_snapshot_vector.py`: 0, PASS.
  - `check_root_backed_query_vector.py`: 0, PASS (27 vectors).
  - `check_context_capsule_vector.py`: 0, PASS (24 vectors).
- `check_smp1_persistent_fuzz_slice.py`: **1**, `proof-record-predates-lane-change`.
- `check_smp1_json_bridge_persistent_fuzz_slice.py`: **1**, `proof-record-predates-lane-change`.

**Unit tests run (Python and Rust):**
- `python3 -m unittest`:
  - `scripts/test_smp1_contract.py`: 0, 14 OK.
  - `scripts/test_current_contract_review.py`: 0, 6 OK.
  - `scripts/test_session_handle_profile.py`: 0, 6 OK.
  - `scripts/test_cli_contract.py`: 0, 7 OK.
  - `scripts/test_smp1_json_bridge_table.py`: exit 5, no tests collected. Run as a script it reports 17 cases PASS.
- Cargo:
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 0, 2 passed.
  - `cargo test --locked --offline -p sley-repo --lib index_cache`: 0, 9 passed.
  - `cargo test --locked --offline -p sley-cli --test cli v3_serve_reports_the_actual_selected_version`: 0, 1 passed. This confirmed `debug/sley` is built from HEAD.

**Runtime probe (scratch driver in `/tmp`, removed afterwards, nothing written to the tree).** I drove `sley serve --json` using `bench/sley2/runner.py` helpers and `bench/live/mediated_client.root_query_preimage`, under `--protocol-profile v2-capable` and `v3-capable`:
- Seeded with the repository-exchange fixture: the warm-up query was refused `INDEX_SNAPSHOT_ROOT_INCOMPLETE`, so there was no field 9 under either version. A non-empty body was refused under both versions.
- Seeded with the complete-root `bench/fixtures/sley2/S2B-CONTEXT-001/base.pack`:
  - v2: selected 2. Cold open returned 8 fields, bounds 1/0. The warm-up was refused `QUERY_SNAPSHOT_MISMATCH`. Warm open returned **9 fields**, field 9 = `c9fc1f53…ae87a7e`, bounds 1/0/not truncated. `revision.read` returned 8 fields.
  - v3: selected 3, frame version 3. The result was **identical to v2: warm open returned 9 fields, field 9 = `c9fc1f53…ae87a7e`**. `revision.read` returned 8 fields.

**Files read (line ranges):**
- `docs/spec/SMP1.md` 1-853 (the whole file).
- `crates/sley-protocol/src/server.rs` 495-690, 1340-1363, 2160-2180, 2435-2460, 2887-2931, 3280-3316.
- `crates/sley-protocol/src/lib.rs` 30-38, 1055-1090.
- `crates/sley-txn/src/maintenance.rs` 1-220.
- `crates/sley-txn/src/repository.rs` 876-895, 4055-4061.
- `crates/sley-repo/src/index_cache.rs` (diff hunk).
- `docs/spec/SCB1.md` 112-127.
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 1-6, 345-374, 536-570.
- `docs/spec/SLEY_CLI_V1.md` 1-30, 340-362.
- `docs/spec/SMP1_JSON_BRIDGE_V1.md` 1-50.
- `docs/spec/SESSION_HANDLE_PROFILE_V1.md` 1-40, 160-215, 360-380.
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` (diff; 164-183).
- `scripts/fuzz_proof_record.py` 130-183.
- `scripts/check_smp1_contract.py` 50-57, 252-300.
- `conformance/smp1-json-bridge/v1/methods.json` (rows 201, 212, 600, 604).
- The machine-summary `protocol` section.
- The gate record §3, §5, §11.

Note on tree state: at the end, `git status` showed one untracked file I did not create: `evidence/review/verdicts/complete_root_index_snapshot/ariadne_contract_review_revision_4-f073811.md`. It appeared during this session, most likely from a concurrent review. I did not touch it.

## Evidence checked

- **Version 1 path.** At version 1 the response is exactly `revision_summary`, through `head_open_summary(&head, None)` and `revision_summary_fields` (`server.rs:2906-2912, 3282-3316`). The legacy-Harness test `server_tests.rs:615-640` checks this with a warm cache and matches `SMP1.md:711-712`.
- **Legacy server.** A legacy `Server::new` can never dispatch 201 under a selection above 1. `SMP1.md:222-227` says so, and the claim check at `server.rs:1102` enforces it.
- **Version 2 path.** The body is `open_summary`: fields 1-8 byte-identical, field 9 only when warm, count 9, tag 9, length 32 (`server_tests.rs:7393-7461`, plus my runtime probe).
  - Structural absence of field 9 follows SCB1 §5 (`SCB1.md:119-122`: "Optional fields are omitted when absent"), so the appendix A grammar at `SMP1.md:693-694` is well formed.
  - Bounds are 1 item with no omission or truncation, matching `SMP1.md:708-709`.
  - `revision.read` stays at 8 fields under every selection I exercised (`SMP1.md:710-711`).
- **The probe is read-only.**
  - `cached_complete_root_snapshot_id` only reads the record and applies `accept_cached`, with no fallthrough and no write-back. The test `index_cache.rs` `probe_reports_only_a_materialized_snapshot_and_never_builds` covers a corrupt record: it is reported absent and left unrewritten.
  - The guard is taken with `try_lock_shared` and nothing is created (`maintenance.rs:142-146, 148-193`). The initializing call `maintenance()` at `server.rs:2443-2447` is not used by the probe.
- **Non-empty request body.** It is refused before the head loads (`server.rs:2903-2905`), under v1 (test), v2 (test) and v3 (runtime).
- **Frozen vectors still verify.** The SMP1 frame/hello vectors, bridge vectors, entity-read, S20-300 snapshot, root-query and capsule vectors, and the fixture/table generators all PASS unchanged. The bridge's generated v1 metadata has no body columns (`methods.json` row 201: tag/name/owner/family/reserved), so correcting row 201's request column changes no generated byte.
- **Revision history.** It is present (`SMP1.md:53-72`). The status line, "Current composition (revision 13)", the checker revision (`CONTRACT_REVISION = 13`) and the machine summary (`contract_revision` 13, `current_delta_review` 13 PENDING×3, `contract_complete` false, status `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING`) all agree.
- **Callers.** Every in-repo caller of `workspace.open` sends an empty body (`bench/live/sley2_tool.py:338,830`, `crates/sley-cli/tests/cli.rs:136`, the tests).
- **Version 3 reachability.** `negotiate_versioned` admits 3 (`lib.rs:1065-1074`). The CLI offers it through `--protocol-profile v3-capable` (`sley-cli/src/lib.rs:213,511`). The gate at `server.rs:2907` is `>= PROTOCOL_VERSION_V2`, and its docstring at `server.rs:2892-2893` says "version 2 (or later)".

## Findings

[P1] [version-gate] crates/sley-protocol/src/server.rs:2892-2893,2907 vs docs/spec/SMP1.md:659,697-698,711-712; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:176-177; docs/spec/NATIVE_TEST_ADMISSION_V1.md:353,542-543 - The server sends `open_summary` for every selection ≥ 2, but no contract says so. SMP1 revision 13 restricts `open_summary` to "`workspace.open` under a version 2 selection only", and S20-300 revision 4 names its sole probe consumer as field 9 "under a version 2 selection". Version 3 is reachable through `negotiate_versioned` (lib.rs:1065-1074) and `sley --protocol-profile v3-capable`. Its owning contract defines v3 as the union of the SMP1 tables "unchanged at revision 12" and requires old method behavior to be preserved. That contract was not re-pinned. Runtime evidence: under selection 3, a warm `workspace.open` answers a 9-field body whose field 9 (`c9fc1f53…ae87a7e`) is identical to the v2 run. So a live, negotiable version has 201 response bytes that no contract states. - Closure evidence: either (a) gate on `== PROTOCOL_VERSION_V2`, with a v3 warm-cache test asserting an 8-field body; or (b) SMP1 and S20-300 §5 state that `open_summary` applies to version 2 and the NATIVE_TEST_ADMISSION version 3 union, NATIVE_TEST_ADMISSION is revised or re-pinned to SMP1 revision 13, and a v3 warm-cache field-9 test is added. In either case, correct the consumer re-pin sentences that say only the version 2 body changes.

[P3] [spec-precision] docs/spec/SMP1.md:63-64,70-71,316-318,844-846 (also ADR-0032 current pin; machine-summary protocol.status_note) - The text says revision 13 serves version 1 "exactly as before, byte for byte, including `workspace.open`". It also says "no version 1 byte changes", that revision 13 changes only "one version 2 response body and one request column", and that appendix A row 201 is amended "for the version 2 selection only". But revision 13 changes version 1 behavior. At ab42a3a9 a non-empty 201 body was ignored and answered with success (`Method::WorkspaceOpen => self.workspace_open()`). Now it answers `PROTOCOL_PAYLOAD_INVALID` under every version (server.rs:2903-2905; server_tests.rs:638-639 on the legacy v1 Harness; my runtime probe). Revision 13 also rewrites the frozen v1 table's row 201 response cell ("accepted head" → "accepted head summary (appendix A)"). The one-sentence history mention of the refusal does not reconcile these absolute claims. - Closure evidence: restate these passages as "every version 1 success response is byte-identical; a non-empty 201 body, previously ignored, is refused under every version; row 201's request and response cells are corrected", in SMP1, ADR-0032 and the status note.

[P3] [re-pin] docs/spec/SLEY_CLI_V1.md:26,356-359; docs/spec/SESSION_HANDLE_PROFILE_V1.md:16-17,22-24; docs/spec/SMP1_JSON_BRIDGE_V1.md:40,46-50; scripts/check_cli_contract.py:226 - **Ruling: the pin-only re-pins are not acceptable as made. Each owner needs a revision bump.** Every earlier pin move in these three contracts went through a revision, including explicit "no behavior change" re-pins: CLI revisions 4 and 5, bridge revisions 7 and 8, session profile revision 3. Their stage checkers assert the pin, so the pin is a checked normative clause of text that was already accepted (all three packages are COMPLETE). The in-place edits are also incomplete:
- The CLI's authority sentence (SLEY_CLI_V1.md:26) still names "`docs/spec/SMP1.md` revision 12". The CLI checker's substring test is satisfied by the new re-pin sentence, so it cannot see this.
- The session profile's opening sentence (:16-17) still says "SMP1 (S20-400, revision 12)".
- The shared rationale sentence ("changes only the version 2 `workspace.open` response body") is false under the current code (see the P1).

The substantive claim holds: I verified that no bridge, CLI or session clause changes. The bridge and CLI carry 201 bodies as opaque bytes, `methods.json` has no body columns, and 201 stays head-bound in the session profile. - Closure evidence: bridge revision 11, CLI revision 9 and session profile revision 5 re-pins, each with a new-delta review (or a recorded owner-lane acceptance of pin-only moves); stale lines :26 and :16-17 corrected; the CLI checker anchored to the authority sentence.

[P3] [evidence] scripts/check_smp1_persistent_fuzz_slice.py; scripts/check_smp1_json_bridge_persistent_fuzz_slice.py; scripts/fuzz_proof_record.py:170-175; docs/spec/SMP1.md:626 - Both SMP1-family persistent fuzz slice checkers exit 1 with `proof-record-predates-lane-change` at HEAD. Both proofs were recorded at source commit `7bcc5a89`. The only lane-path changes since then are the three revision-13 implementation files (`server.rs`, `server_tests.rs`, `index_cache.rs`), so this delta invalidated the S20-700 evidence that SMP1 §10 requires. The gate record §11.4 discloses this. - Closure evidence: a fresh local fuzz proof at a head containing `dda51a16`, with both checkers exiting 0.

[P3] [stale-spec, pre-existing] docs/spec/SMP1.md:62-63,122,210-213,243-245 - SMP1 still says the explicit entrypoints `negotiate_versioned` and `encode_frame_for_version`/`decode_frame_for_version`/`validate_for_version` "admit only selections 1 and 2", and that a greatest-common result "including 3, is refused". It also says ordinary frames carry "1 or 2". But `lib.rs:1065-1074` has admitted 3 since `116d031e` (2026-09-16), the day after the revision-12 PASS. Revision 13 re-issues the document without reconciling this, and the two-version framing is the root of the P1. This is not introduced by revision 13, but no review has seen it. - Closure evidence: SMP1 states that version 3 is defined by NATIVE_TEST_ADMISSION appendix D over the explicit entrypoints (or the entrypoints go back to 1-2 only), with the SMP1 checker anchoring that sentence.

[P4] [precision] docs/spec/SMP1.md:705-707 - "an absent or contended repository maintenance boundary … are all absence" describes the probe accurately, but the absent case cannot happen through `workspace.open`. `accepted_head()` needs `locks/` and first takes the shared lock, blocking (sley-txn repository.rs:881-884, 4055-4060). An absent boundary therefore fails the whole method, and an exclusive holder makes the opener wait. The server docstring (server.rs:2915-2921) says this; SMP1 does not. - Closure evidence: one sentence in SMP1 appendix A saying only the probe is non-waiting, and that the head load keeps the S20-390 blocking shared acquisition.

[P4] [owner-column] docs/spec/SMP1.md:304-306,328,659 - §4 says that where a body crosses an owner boundary "the row says so". Under version 2, row 201's response is S20-390 plus S20-300, but its owner cell still says "S20-390". That is probably unavoidable: the cell feeds the frozen bridge v1 metadata (`methods.json` owner "S20-390"). Appendix A states the split instead. - Closure evidence: a sentence in the v1-table preamble recording this exception, or an explicit note that appendix A governs row 201's owner split.

[P4] [test-coverage] scripts/check_smp1_contract.py:252-293 - The contract checker checks only which appendix tags are covered. None of the revision-13 normative text is anchored (row 201's cells, `open_summary`, the version-gate rule, the empty-body refusal), so reverting that text would still PASS. Only the Rust tests pin the behavior. - Closure evidence: anchors for the row 201 cells, the `open_summary` grammar and its version-scope sentence, each with a refusal case in `test_smp1_contract.py`.

## Assessment

The version 1 and version 2 wire behavior of `workspace.open` is correct and well tested:
- Version 1 success bytes are unchanged, including with a warm cache.
- Version 2 field 9 is present only when warm, structurally absent when cold, and counted as one item with no omission signal.
- The probe never builds, writes, deletes, initializes or waits.
- `revision.read` is unchanged.
- Every frozen SMP1-family vector still verifies.

The `open_summary` grammar fits the SCB1 §5 optional-field rule, and the revision history, status, checker and machine-summary bindings agree.

The version gate does not match the contract. The implementation sends `open_summary` to version 3 sessions, which I confirmed at runtime. SMP1 revision 13 and S20-300 revision 4 both say version 2 only, and the version 3 owner contract still pins SMP1 revision 12 with a preserve-old-behavior rule. That is wire behavior at a negotiable version that no contract states. It blocks acceptance, and closing it takes either a one-line code gate change with a v3 test, or coordinated text in SMP1, S20-300 and NATIVE_TEST_ADMISSION.

Beyond that:
- The "version 1 byte-for-byte" claims overstate what revision 13 does, because the v1 empty-body refusal is new.
- The consumer re-pins are incomplete (the CLI's line 26 and the session profile's lines 16-17 still name revision 12) and depart from each owner's re-pin-by-revision precedent. My ruling is that each owner needs a revision bump.
- The SMP1 fuzz slice proofs are stale because of this delta.
- SMP1 §2 predates version 3.

I make no GA, release-readiness or package-completion claim.

VERDICT: REVISE_0_P0_1_P1_0_P2_4_P3_3_P4
SECTION: protocol
FIELD: ariadne_contract_review_revision_13
SCOPE_SHA: f073811914297505803f5b731cd4691e1623de84
FINDINGS: [P1] [version-gate] crates/sley-protocol/src/server.rs:2892-2893,2907 vs docs/spec/SMP1.md:659,697-698,711-712; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:176-177; docs/spec/NATIVE_TEST_ADMISSION_V1.md:353,542-543 - the server answers open_summary for every selection >= 2 (runtime: v3 warm workspace.open returns 9 fields, field 9 c9fc1f53…ae87a7e identical to v2) while SMP1 r13 and S20-300 r4 say version 2 only and the v3 owner pins SMP1 "unchanged at revision 12" with preserve-old-behavior; no contract states the v3 201 bytes - gate == V2 with a v3 warm 8-field test, or SMP1/S20-300 text covering v3 plus a NATIVE_TEST_ADMISSION revision/re-pin and a v3 field-9 test, and corrected re-pin rationale | [P3] [spec-precision] docs/spec/SMP1.md:63-64,70-71,316-318,844-846 (ADR-0032, protocol.status_note) - "version 1 byte for byte, including workspace.open", "no version 1 byte changes", "version 2 selection only" overclaim: a non-empty v1 201 body was answered success at ab42a3a9 and is now PROTOCOL_PAYLOAD_INVALID (server.rs:2903-2905; server_tests.rs:638-639), and the frozen v1 row 201 response cell was rewritten - restate as success bytes identical, empty-body refusal new under every version, row 201 cells corrected | [P3] [re-pin] docs/spec/SLEY_CLI_V1.md:26,356-359; docs/spec/SESSION_HANDLE_PROFILE_V1.md:16-17,22-24; docs/spec/SMP1_JSON_BRIDGE_V1.md:40,46-50; scripts/check_cli_contract.py:226 - ruling: pin-only re-pins are not acceptable; each owner needs a revision bump per its own precedent (CLI r4/r5, bridge r7/r8, session r3); the in-place edits leave CLI :26 and session :16-17 naming SMP1 revision 12 (the CLI checker cannot see it) and the rationale "changes only the version 2 body" is false under the code; no substantive consumer clause changes (verified) - bridge r11, CLI r9, session r5 re-pins with new-delta review or recorded owner acceptance, stale lines fixed, CLI checker anchored | [P3] [evidence] scripts/check_smp1_persistent_fuzz_slice.py; scripts/check_smp1_json_bridge_persistent_fuzz_slice.py; scripts/fuzz_proof_record.py:170-175; docs/spec/SMP1.md:626 - both exit 1 proof-record-predates-lane-change; proofs at 7bcc5a89, and the only lane changes since are the three revision-13 files - fresh fuzz proof at a head containing dda51a16 with both checkers exiting 0 | [P3] [stale-spec, pre-existing] docs/spec/SMP1.md:62-63,122,210-213,243-245 - SMP1 says explicit entrypoints admit only 1 and 2 and refuse 3, but lib.rs:1065-1074 admits 3 since 116d031e (2026-09-16, after the r12 PASS); r13 re-issues without reconciling, which caused the P1 framing - SMP1 states that NATIVE_TEST_ADMISSION appendix D defines version 3 over the explicit entrypoints, with a checker anchor | [P4] [precision] docs/spec/SMP1.md:705-707 - the "absent boundary is absence" case cannot happen through workspace.open (the head load requires and blocks on the shared lock first, sley-txn repository.rs:881-884, 4055-4060) and SMP1 does not say the opener still waits - one sentence scoping non-waiting to the probe | [P4] [owner-column] docs/spec/SMP1.md:304-306,328,659 - row 201's owner cell stays S20-390 although the v2 body crosses into S20-300; the cell is frozen bridge v1 metadata, and appendix A carries the split - state this exception in the v1-table preamble | [P4] [test-coverage] scripts/check_smp1_contract.py:252-293 - no anchor pins any revision-13 normative text (row 201 cells, open_summary, version scope, empty-body refusal), so a revert still passes - add anchors with refusal cases in test_smp1_contract.py
SUMMARY: The version 1 and version 2 behavior of SMP1 revision 13's workspace.open is correct, tested, and confirmed at runtime; the probe is read-only and non-waiting, and every frozen SMP1-family vector still verifies. The implementation also sends the nine-field open_summary to version 3 sessions (observed at runtime), which SMP1 r13, S20-300 r4 and the version 3 owner contract all exclude or leave unstated; this P1 blocks acceptance. Also open: the version 1 "byte for byte" claims overlook the new empty-body refusal, the consumer re-pins are incomplete (ruling: each owner needs a revision bump), the SMP1 fuzz proofs are stale because of this delta, and SMP1 §2 predates version 3.
