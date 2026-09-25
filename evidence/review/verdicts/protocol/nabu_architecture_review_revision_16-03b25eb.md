<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9; role: nabu; field: nabu_architecture_review_revision_16; dispatched: 2026-09-23T14:33:54Z; duration_s: 505; process_exit_code: 0 -->
# Nabu Council review — protocol

Harness: claude-code
Reviewed checkpoint: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9

What I verified:
- **Scope.** `git rev-parse HEAD` returned `03b25eb9b3d7e80576394109ca4b73cc21b2fab9`, which matches. The worktree is clean. `git log 26d050e6..HEAD` shows 8 commits; the errata commit is `6519f6e0`.
- **Diffs read.**
  - `git diff 26d050e6..HEAD -- docs/spec/SMP1.md crates/sley-protocol scripts/` (11 files).
  - The same range for `docs/adr/ADR-0032-smp1-transport-boundary.md`, `docs/WORK_PACKAGES.md`, `docs/spec/SLEY2_TRIAL_RUNNER_V1.md` and `docs/adr/ADR-0036-*`.
  - `git diff --stat 26d050e6..HEAD` (41 files).
- **Files read.**
  - `docs/spec/SMP1.md:1-104, 750-789`
  - `crates/sley-protocol/src/server.rs:1150-1310, 2200-2490, 2895-2973`
  - `crates/sley-txn/src/maintenance.rs:1-190`
  - `crates/sley-txn/src/repository.rs:870-1011, 4055-4061, 4445-4454`
  - `docs/adr/ADR-0032-smp1-transport-boundary.md:1-34`
  - `scripts/check_smp1_contract.py:50-96, 195-231, 380-466`
  - `scripts/check_sley2_trial_runner.py:140-200`
  - `scripts/test_sley2_trial_runner.py:25-50`
  - `scripts/test_smp1_contract.py:71-91`
  - `bench/live/sley2_tool.py:60-130, 285-445, 1015-1030`
  - `bench/live/taskpacks.py:177-202`
  - The prior Nabu transcript `evidence/review/verdicts/protocol/nabu_architecture_review_revision_15-26d050e.md:128-179`.
- **Checkers.** One per call, all exit 0:
  - `check_smp1_contract.py`: `"result": "PASS"`, revision 16
  - `check_cli_contract.py`: PASS, revision 10
  - `check_smp1_json_bridge_contract.py`: PASS, revision 12
  - `check_session_handle_profile.py`: PASS, `smp1_revision` 15
  - `check_sley2_trial_runner.py`: PASS, revision 8
  - `check_complete_root_index_snapshot_profile.py`: PASS, revision 6
  - `check_smp1_persistent_fuzz_slice.py`: PASS
  - `check_smp1_json_bridge_persistent_fuzz_slice.py`: PASS
  - `check_smp1_vector.py` and `check_smp1_json_bridge_vector.py` under `uv run --offline --frozen --project oracle/scb1`: exit 0, PASS each
- **Unit suites.**
  - `scripts.test_smp1_contract`, `test_cli_contract`, `test_smp1_json_bridge_contract` and `test_session_handle_profile`: `Ran 44 tests … OK`.
  - `scripts.test_sley2_trial_runner` and `test_complete_root_index_snapshot_profile`: `Ran 30 tests … OK`.
- **Cargo.**
  - `cargo test --locked --offline -p sley-cli --test cli handshake_identity_does_not_depend_on_the_transport_flag`: 1 passed. Nothing recompiled, so `debug/sley` matches HEAD.
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 5 passed.
- **Live `sley serve` probe** (`sley2_tool.Session` over a `stage_initial("sley_2_0","S2B-CONTEXT-001",…)` workspace in a `~/.cache` mkdtemp, removed afterwards). I unlinked `repo/locks/maintenance.lock` between requests.
  - `workspace.open` returned a byte-identical 212-byte body, and the lock was recreated.
  - `session.budgets` succeeded, and the lock was recreated.
  - `entity.version` (body `00`) failed with `TXN_IO` (26 bytes), and the lock stayed absent. With the lock present, the same request fails `PROTOCOL_PAYLOAD_INVALID`.
- **In-memory probes.**
  - I ran the three consumer checkers against five mutated SMP1 Status lines.
  - I ran `composed_pin_problems` with S20-620 pinned to 15.
  - I ran `check_smp1_contract` with appendix A spliced back to the revision 15 wording.

## Evidence checked

1. **The code path the erratum describes is real for `workspace.open`.**
   - `server.rs:1184-1186` sends `WorkspaceOpen` to `session_check_mixed_retained` (`:2410-2420`).
   - That calls `head_binding_mixed` (`:2375-2384`), then `head_mixed` (`:2391-2392`), then `Server::maintenance()`.
   - `Server::maintenance()` (`:2469-2473`) calls `initialize_repository_maintenance(&self.repository)` and then `acquire_shared_repository_maintenance` (blocking).
   - `initialize_repository_maintenance` (`maintenance.rs:50-100`) creates `locks/maintenance.lock` with `create_new` and treats `AlreadyExists` as success.
   - The probe `materialized_head_snapshot` (`server.rs:2965-2973`) uses `acquire_shared_repository_maintenance_nonblocking`, which never initializes, and `.ok()?` turns any failure into absence.
   - The live probe confirms this: an absent boundary does not fail `workspace.open`, and the answer is byte-identical.
   - The new docstring (`server.rs:2931-2935`) states this path exactly.
   - No production code deletes `maintenance.lock` (grep over `crates/`, tests excluded), so "never reaches the probe" holds. An external deletion inside the window would still produce absence, so the result fails closed.
2. **Nothing normative changed.**
   - `server.rs` changed only in 4 `///` doc lines.
   - The `SMP1.md` diff has five hunks: the Status line (revision number plus the errata declaration), the composition label, the history entry for revision 16, the review-state sentence, and the appendix A clause.
   - No method table, grammar, limit, error code or wire statement changed. The `open-summary-scope` anchor still reads "revisions 13 to 15".
   - `check_smp1_vector.py` and `check_smp1_json_bridge_vector.py` still PASS.
3. **Consumer checkers.**
   - CLI (`check_cli_contract.py:302-312`), bridge (`check_smp1_json_bridge_contract.py:342-352`) and session (`check_session_handle_profile.py:468-478`) read the declared normative revision (`errata-only over normative revision (\d+)`) and fall back to the raw revision.
   - Mutation probe results, identical for all three:

     | Mutated Status line | Result |
     |---|---|
     | as-is | PASS |
     | declaration dropped | FAIL `smp1-revision-pin` |
     | declares normative 14 | FAIL |
     | declaration not in first segment | FAIL (fails closed) |
     | revision 17 errata over 15 | PASS (by design; the SMP1 checker's `CONTRACT_REVISION` owns that) |

   - The owner side (`check_smp1_contract.py:53-57, 423-431`) requires the declared normative revision to equal `NORMATIVE_REVISION` = 15. The v3 owner pin in NATIVE (`:92-94`) is checked against it. `test_smp1_contract.py:329-344` fails when the declaration is dropped.
4. **Machine summary protocol section.** `contract_revision` 16, `current_delta_review` {16, PENDING×3}. The revision 15 verdicts are kept with `on 26d050e…` notes. The historical verdicts are preserved.

## Findings

[P3] [ownership/text-accuracy] docs/spec/SMP1.md:770-777 - The erratum makes two claims the code does not support. First, it credits creation of the boundary to "the S20-390 blocking shared maintenance acquisition like every head-bound read". Second, its parenthetical says `initialize_repository_maintenance` is "run by the session check that precedes every session-bound answer". In fact the S20-390 acquire requires an existing boundary (`crates/sley-txn/src/maintenance.rs:102-111,153-157`; `accepted_head` at `repository.rs:881-885`). Only SMP1's own `Server::maintenance()` (`server.rs:2469-2473`) creates it. On a version-aware server, the session check for `entity.version`/`entity.signature` is `session_check_retained` (`server.rs:2210-2225,2243`), which goes `head_binding` → `head()` (`:2360-2369,2479-2486`) and never initializes. Live probe at 03b25eb9: with the lock unlinked, `entity.version` answers `TXN_IO` and the lock stays absent, while `workspace.open` and `session.budgets` recreate it. - Closure evidence needed: scope the sentence to row 201's path (the `workspace.open` session check through the server's `maintenance()`), attribute the creation to the SMP1 server rather than the S20-390 acquisition, and drop or qualify "every session-bound answer". Alternatively, make `session_check_retained` initialize and add a test for it.

[P3] [identity-discipline/pin-semantics] docs/spec/SMP1.md:79-81; docs/adr/ADR-0032-smp1-transport-boundary.md:25-30 - SMP1 and its ADR both say "consumers keep their revision 15 pins (the normative revision)". In the same commit, S20-620 revision 8 re-pins SMP1 revision 16 (`docs/spec/SLEY2_TRIAL_RUNNER_V1.md:31,51,331-332,342`). Its checker reads the raw Status revision, not the declared normative one (`scripts/check_sley2_trial_runner.py:155-163`), and its test asserts that any SMP1 bump turns the contract red (`scripts/test_sley2_trial_runner.py:47-50`). In-memory probe: pinning S20-620 to the normative 15 yields `smp1-pin:['15']!=16`. So one authority now has two pin semantics. Three consumers track the normative revision and one tracks the document revision. The SMP1 claim is false in-tree, and a future errata revision would force a re-pin on S20-620 only. - Closure evidence needed: either state in SMP1 and ADR-0032 that S20-620 deliberately pins the erratum text at revision 16 (and why), or have S20-620 pin normative 15, cite the revision 16 erratum separately, and make `composed_pin_problems` read the declared normative revision like the other three checkers. Add a revert case either way.

[P4] [evidence-binding] scripts/check_smp1_contract.py:58-83; scripts/test_smp1_contract.py:329-344 - Nothing binds the revision 16 correction, which is the whole content of the revision. `WORKSPACE_OPEN_ANCHORS` has no anchor for the new appendix A sentence. `ErrataRevisionCases` checks only the Status declaration. No `sley-protocol` or `bench` test exercises `workspace.open` with an absent boundary: `maintenance.lock` is referenced nowhere under `crates/sley-protocol` or `bench`. In-memory probe: splicing the revision 15 wording ("so an absent boundary fails the method") back into HEAD's appendix A passes `check_smp1_contract` (exit 0, PASS, no problems). - Closure evidence needed: an anchor for the corrected sentence, with a revert case. Optionally, a server test that deletes the lock and asserts that `workspace.open` succeeds and recreates it.

[P4] [duplicated-authority/test-coverage] scripts/check_smp1_contract.py:423-431; scripts/check_cli_contract.py:302-312; scripts/check_smp1_json_bridge_contract.py:342-352; scripts/check_session_handle_profile.py:468-478 - The errata Status-line grammar is copied verbatim into four checkers with no shared helper. None of `test_cli_contract.py`, `test_smp1_json_bridge_contract.py` or `test_session_handle_profile.py` changed in 26d050e6..HEAD, so the new consumer branch has no regression test. Today the four copies agree (all five mutation probes matched), but a drift in any one copy would go unnoticed. - Closure evidence needed: one helper that all four import, or a test asserting that the four parse a shared set of Status lines identically. Add a drop-declaration case and a wrong-normative case per consumer.

## Assessment

The central claim of revision 16 is correct. An absent maintenance boundary does not fail `workspace.open`, because the row 201 session check creates it before the blocking shared acquire, so the non-waiting probe never sees it absent. I confirmed this in the code (`session_check_mixed_retained` → `head_binding_mixed` → `head_mixed` → `Server::maintenance` → `initialize_repository_maintenance`) and by a live `sley serve` probe at HEAD. The server docstring states the same path exactly.

Nothing else changed normatively. The Rust change is doc-only, and the SMP1 hunks are status, history and appendix A text. All ten SMP1-family checkers and oracles pass, and the 74 script tests and 6 bounded cargo tests pass.

The CLI, bridge and session-handle checkers read SMP1's declared normative revision correctly and fail closed when the declaration is dropped or wrong.

The erratum itself goes too far in one parenthetical. Its "every session-bound answer" and its credit to the S20-390 acquisition are false for version-aware 306/307, which fail `TXN_IO` without creating the boundary (P3).

Its statement that consumers keep their revision 15 pins is contradicted by S20-620 revision 8, whose checker forbids the normative pin. That leaves two pin semantics for one authority (P3).

The correction has no anchor or test, and the errata grammar is duplicated across four checkers (P4 each).

Two open revision 15 items are neither closed nor re-filed here:
- **ENTITY_READ §2 filter text (P3).** Unchanged, since that file is not in the diff.
- **ADR-0032 "Current pin" statements (P4).** Revision 16 adds a fifth such statement (`ADR-0032:25`), which compounds the existing finding.

My lane accepts revision 16 as an errata-only revision over normative revision 15. I make no claim about the S20-620, CLI, bridge or session lanes beyond the pins read above, and none about GA or release readiness.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_2_P4
SECTION: protocol
FIELD: nabu_architecture_review_revision_16
SCOPE_SHA: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9
FINDINGS: [P3] [ownership/text-accuracy] docs/spec/SMP1.md:770-777 - the erratum credits boundary creation to "the S20-390 … acquisition like every head-bound read" and says `initialize_repository_maintenance` is "run by the session check that precedes every session-bound answer"; the S20-390 acquire requires an existing boundary (maintenance.rs:102-111,153-157; repository.rs:881-885), only SMP1's Server::maintenance (server.rs:2469-2473) creates it, and version-aware entity.version/signature use session_check_retained → head() (server.rs:2210-2225,2243,2479-2486), which never initializes; live probe: with the lock unlinked, entity.version answers TXN_IO and the lock stays absent - scope the sentence to row 201's path, attribute creation to the server, drop or qualify "every session-bound answer" (or initialize in session_check_retained, with a test) | [P3] [identity-discipline/pin-semantics] docs/spec/SMP1.md:79-81; docs/adr/ADR-0032-smp1-transport-boundary.md:25-30 - "consumers keep their revision 15 pins" is contradicted by S20-620 rev 8 pinning SMP1 16 (SLEY2_TRIAL_RUNNER_V1.md:31,51,331-332,342), whose composed_pin_problems (check_sley2_trial_runner.py:155-163) reads the raw Status revision; probe: pinning 15 yields smp1-pin:['15']!=16, so there are two pin semantics for one authority - declare S20-620's deliberate rev-16 pin in SMP1/ADR-0032, or pin normative 15, cite the erratum, and read the declared normative revision like the CLI/bridge/session checkers | [P4] [evidence-binding] scripts/check_smp1_contract.py:58-83; scripts/test_smp1_contract.py:329-344 - no anchor or server test binds the revision 16 correction; probe: splicing the rev-15 "fails the method" wording back into appendix A passes check_smp1_contract (exit 0) - add an anchor with a revert case, optionally a lock-deleted workspace.open server test | [P4] [duplicated-authority/test-coverage] scripts/check_smp1_contract.py:423-431; scripts/check_cli_contract.py:302-312; scripts/check_smp1_json_bridge_contract.py:342-352; scripts/check_session_handle_profile.py:468-478 - the errata Status regex is copied into four checkers with no shared helper and no consumer-side unit tests (the consumer test files are unchanged); the copies agree today (5 mutation probes) - share one helper or test identical parsing, with drop and wrong-normative cases per consumer
SUMMARY: SMP1 revision 16 correctly states that the workspace.open session check (session_check_mixed_retained → head_mixed → Server::maintenance → initialize_repository_maintenance) creates an absent boundary, so the method never fails and never reaches the probe; code tracing and a live sley serve probe (byte-identical 212-byte answer, lock recreated) confirm it, nothing normative changed, and all SMP1-family checkers, oracles and suites pass. The CLI, bridge and session checkers correctly read the declared normative revision 15 and fail closed on a dropped or wrong declaration. Two P3s remain: the erratum's "every session-bound answer" and its credit to the S20-390 acquisition are false for version-aware entity reads (live TXN_IO), and S20-620 re-pinned revision 16 through a raw-revision checker, contradicting "consumers keep their revision 15 pins". Two P4s note that the correction has no anchor or test (a revert passes) and that the errata grammar is duplicated across four checkers without consumer tests.
