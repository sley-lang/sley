<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9; role: vulcan; field: vulcan_surface_review_revision_16; dispatched: 2026-09-23T14:33:54Z; duration_s: 469; process_exit_code: 0 -->
# Vulcan Council review — protocol

Harness: claude-code
Reviewed checkpoint: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9

I first checked the scope. `git rev-parse HEAD` printed `03b25eb9b3d7e80576394109ca4b73cc21b2fab9`, the branch is `work/succ-context-impl`, and `git status --short` was empty. I did not write anything to the tree.

Git commands:
- `git log --oneline 26d050e6..HEAD` shows 8 commits. The one in scope is 6519f6e0, "SMP1 revision 16 (errata-only)…".
- `git diff --stat 26d050e6..HEAD -- docs/spec/SMP1.md crates/sley-protocol scripts/` shows 11 files changed.
- `git diff 26d050e6..HEAD -- crates/ bench/` shows one code change: the `workspace_open` docstring in server.rs (6 lines). The only other file is a gate record.
- I also ran `git diff 26d050e6..HEAD -- docs/` over ADR-0032, SLEY2_TRIAL_RUNNER_V1 and ENTITY_READ_PROFILE_V2 to check for normative text outside SMP1.md.

Checkers, run from the tree:

| Checker | Exit code | Result |
|---|---|---|
| check_smp1_contract | 0 | PASS, revision 16, `problems: []` |
| check_smp1_json_bridge_contract | 0 | PASS, revision 12 |
| check_cli_contract | 0 | PASS, revision 10 |
| check_session_handle_profile | 0 | PASS, revision 6, `smp1_revision: 15` |
| check_complete_root_index_snapshot_profile | 0 | PASS, revision 6 |
| check_sley2_trial_runner | 0 | PASS, revision 8 |
| check_native_test_vectors | 0 | "6 requests, 6 responses, 7 rejections OK" |
| check_context_capsule_profile | 0 | PASS |
| check_restricted_query_capsule_profile | 0 | PASS |
| check_smp1_persistent_fuzz_slice | 0 | PASS |
| check_smp1_json_bridge_persistent_fuzz_slice | 0 | PASS |

Three vector checkers exit 1 under plain `python3` with `ModuleNotFoundError: blake3`. I re-ran them with `uv run --no-project --with blake3 [--with unicodedata2]`:

| Checker | Exit code | Result |
|---|---|---|
| check_smp1_vector | 0 | PASS: 3 frames, 4 mutations |
| check_smp1_json_bridge_vector | 0 | PASS: 41 methods, 37 rejections, 5 vectors |
| check_entity_read_vectors | 0 | PASS: 23 cases, 91 rejections |

Unit suites:

| Suite | Exit code | Result |
|---|---|---|
| test_smp1_contract | 0 | 18 tests OK |
| test_smp1_json_bridge_contract | 0 | 4 tests OK |
| test_cli_contract | 0 | 10 tests OK |
| test_session_handle_profile | 0 | 12 tests OK |
| test_complete_root_index_snapshot_profile | 0 | 23 tests OK |
| test_sley2_trial_runner | 0 | 7 tests OK |
| test_smp1_json_bridge_table (run directly) | 0 | 17 cases PASS |

Cargo: `cargo test --offline -p sley-protocol --lib workspace_open` passed 5 tests with 0 failed and 123 filtered out.

Live probe:
- The probe is a scratch crate at `/tmp/vulcan-r16-probe`, outside the tree, with its own target directory. It depends by path on the tree's sley-protocol, sley-repo (test-support), sley-txn, sley-scb1 and sley-id.
- The only code change in this range is a docstring, so the probe reflects HEAD behaviour.
- Setup: a `test_support::genesis` repository, a `Server::new_versioned` server (negotiated version 2), and a completed `session.open`.
- Results:
  1. `entity.version` with the lock present: OK.
  2. After removing `locks/maintenance.lock`, `entity.version`: FAILED `TXN_IO` (numeric 0). The lock is still absent afterwards.
  3. `entity.signature`: FAILED `TXN_IO`. The lock is still absent.
  4. `workspace.open`: OK, 8-field body. The lock is re-created.
  5. `entity.version`: OK again.
  6. After removing the lock again, `session.budgets`: OK. The lock is re-created.
  7. With the lock path replaced by a directory, `workspace.open`: FAILED `TXN_IO` 39019.

Mutation probe: I patched `Path.read_text` for SMP1.md in memory and fed six Status-line variants to check_cli_contract, check_smp1_json_bridge_contract, check_session_handle_profile and check_smp1_contract. The variants were: baseline, declaration dropped, normative 14, negated ("not errata-only"), two prefix segments, and the declaration moved off the Status line. Baseline returned rc 0 in all four. Every other variant returned rc 1: the consumers reported `smp1-revision-pin`, and the SMP1 checker reported `spec-normative-revision:<n>!=15`.

Files read:
- `crates/sley-protocol/src/server.rs:490-560, 1130-1305, 2200-2500, 2900-3080`
- `crates/sley-protocol/src/server_tests.rs:1-80, 2712-2949, 3355-3435, 5223-5302, 7394-7653`
- `crates/sley-txn/src/maintenance.rs:1-190`
- `crates/sley-txn/src/repository.rs:870-1030`
- `crates/sley-repo/src/test_support.rs:90-130`
- `crates/sley-protocol/Cargo.toml`
- `scripts/check_smp1_contract.py:195-231, 380-470`
- `scripts/check_sley2_trial_runner.py:120-200`
- The full scripts diffs for the CLI, bridge, session, S20-300 and S20-620 checkers and their tests
- `docs/spec/SMP1.md:1-24, 72-104, 762-782`
- `docs/adr/ADR-0032-smp1-transport-boundary.md:20-32`
- `docs/spec/SLEY2_TRIAL_RUNNER_V1.md:20-40, 51, 329-334`
- My own prior transcript `evidence/review/verdicts/protocol/vulcan_surface_review_revision_15-26d050e.md`
- The `protocol` section of `machineresearch/sley-2.0/machine-summary.json`

## Evidence checked

**Diff scope.**
- The SMP1.md diff has five hunks:
  - the Status line, which adds "errata-only over normative revision 15";
  - the Current composition label, now "(revision 16)";
  - the revision-history entry for 16 (:75-81);
  - the review-status sentence (:101-103);
  - appendix A (:770-777).
- Only appendix A is normative text. It changes a statement about behaviour and adds nothing to the wire format, tables, codes or ordering.
- The server.rs change is docstring-only (:2931-2935).
- The claim that nothing else changed normatively holds.

**The erratum's code path for `workspace.open`.**
- Dispatch sends WorkspaceOpen to `session_check_mixed_retained` (server.rs:1184-1186).
- That calls `head_binding_mixed` (:2415), then `head_mixed` (:2391-2392), then `Server::maintenance()` (:2469-2473).
- `Server::maintenance()` calls `initialize_repository_maintenance`, which creates `locks/maintenance.lock` and fsyncs it (maintenance.rs:50-100), then takes the blocking shared lock.
- `workspace_open` (:2936-2955) then runs the non-waiting, non-initializing probe (:2965-2973).
- Probe step [4] confirms this: an absent lock is re-created and the method answers.
- The SMP1 sentence for `workspace.open` and the server docstring match the code.

**Consumer checkers.**
- CLI (:302-312), bridge (:342-352) and session handle (:468-478) each parse the errata form first and fall back to the raw revision. They compare the result with their own pin, 15.
- The SMP1 checker (:423-431) requires the declared normative revision to equal `NORMATIVE_REVISION = 15`. It still requires exactly one Status line (:432-440) and a composition revision equal to 16.
- The v3 owner pin in NATIVE_TEST_ADMISSION is checked against the normative revision (:93).
- All mutation variants fail closed.
- S20-620 is the exception. Its `composed_pin_problems` (check_sley2_trial_runner.py:155) reads the raw document revision, and its text pins 16 (finding 4).

**Other scripts in the diff.**
- The S20-300 and S20-620 completion gates now require the `_revision_<N>_note` to name a 40-hex commit whose copy of the contract shows revision N.
- Unscoped notes, unknown commits and older-revision commits are refused, and the new tests pass.
- See finding 5 for what the gate does not bind.

**Machine summary.** `current_delta_review` is `{contract_revision: 16, ariadne/nabu/vulcan: PENDING}`, and the r15 lane fields are all scoped "on 26d050e…".

## Findings

[P3] [spec-code] docs/spec/SMP1.md:770-774 - The erratum over-generalizes. It says the head load happens under the S20-390 acquisition "like every head-bound read", that "That acquisition first creates the maintenance boundary", and that `initialize_repository_maintenance` is "run by the session check that precedes every session-bound answer". The S20-390 acquisition does not initialize: `acquire_shared_repository_maintenance` requires an existing boundary (maintenance.rs:102-110, 153-157). Initialization happens only in the SMP1 server's `Server::maintenance()` wrapper (server.rs:2469-2473), reached through `head_mixed`. On a version-aware server, `entity.version` and `entity.signature` are head-bound but take another path: server.rs:1176-1177, then `session_check_retained` (:2210-2215), `head_binding` (:2360-2361), `head()` (:2479-2485), `TransactionRepository::accepted_head` (repository.rs:881-884). That path never initializes. The live probe confirms it: with the lock removed after `session.open`, both entity reads FAILED `TXN_IO` (numeric 0, where the `maintenance()` path reports 39019) and left the lock absent, while `workspace.open` and `session.budgets` re-created it. The `workspace.open` clause itself is correct. My revision-15 transcript made the same over-generalization ("every session-checked method"), and this text inherited it. - Closure evidence: scope the parenthetical to the server's `maintenance()` head load, used by `session.open`, `workspace.open` and the legacy session check. State that the version-aware entity-read check uses the non-initializing S20-390 acquisition and fails `TXN_IO` on an absent boundary; making that path initialize would be a behaviour change, not an erratum. Add a check_smp1_contract anchor with a revert case.

[P3] [test-coverage] crates/sley-protocol/src/server.rs:2931-2935 - No test pins the behaviour that revision 16 now states as normative. Searching `crates/sley-protocol` for `maintenance.lock` returns no matches, and none of the 5 `workspace_open` tests removes the boundary. My revision-15 closure evidence named this test "either way". Without it, the sentence can silently regress, which is how the wrong text survived revisions 14 and 15. The erratum is currently supported only by code reading and by my out-of-tree probe. - Closure evidence: a server test that deletes `locks/maintenance.lock` after `session.open` and asserts that `workspace.open` succeeds, the lock is re-created, and the answer is 8 fields cold. A companion assertion should cover whatever outcome the corrected text states for version-aware entity reads.

[P4] [spec-precision] docs/spec/SMP1.md:771-777, docs/adr/ADR-0032-smp1-transport-boundary.md:26-29, crates/sley-protocol/src/server.rs:2931-2935 - The corrected texts leave out the failure branch of initialization. Probe [7], with the lock path replaced by a directory, gives `workspace.open` FAILED `TXN_IO` 39019, and maintenance.rs:46-49 lists the create and persistence failures that do the same. ADR-0032's "(it never fails the method, as revision 15 said)" is absolute and reads as though revision 15 made the corrected claim. The docstring's "so the probe always finds it" overstates: the session-check guard is dropped when `head_mixed` returns (:2392-2406), and the probe acquires the lock again later (:2969). A removal in between is harmless absence, but it is not "always". - Closure evidence: state that an absent boundary is created, or the method fails `TXN_IO` 39019 when it cannot be. Reword the ADR parenthetical to "contrary to revision 15". Qualify "always".

[P4] [pin-policy] docs/spec/SMP1.md:79-81, docs/spec/SLEY2_TRIAL_RUNNER_V1.md:51,331-332, scripts/check_sley2_trial_runner.py:155-162 - SMP1 revision 16 and ADR-0032 say "consumers keep their revision 15 pins". S20-620 instead re-pinned document revision 16, and its checker reads the raw Status revision without the errata parse, so consumers now follow two pin policies (normative and document). The errata marker is also self-declared: the consumer checkers trust the Status line, and no gate compares revision 16's normative text against revision 15. I verified the diff by hand for this round. - Closure evidence: either S20-620 pins the normative revision like the CLI, bridge and session consumers, or SMP1 and ADR-0032 name S20-620 as a document-revision consumer. Optionally, add a digest or anchor check that an errata-only revision leaves the normative sections byte-identical.

[P4] [checker] scripts/check_complete_root_index_snapshot_profile.py:223-250, scripts/check_sley2_trial_runner.py:171-198 - `scope_bound_problems` is documented as binding a verdict "to the text it reviewed", but it binds only the Status-line revision number at the named commit. It takes the first `on <40-hex>` match in the note. It runs `git show` with no ancestry check against HEAD. It does not compare the reviewed bytes with HEAD. So an edit made after the review under the same revision number, or a verdict scoped to a side-branch commit that carries revision N, still binds. - Closure evidence: require `git merge-base --is-ancestor <sha> HEAD`, and require the contract bytes (or a normative-section digest) at `<sha>` to equal HEAD's, or document that the gate binds the revision number only. Add a negative case for each.

## Assessment

Revision 16 does what it says for the case it was written for. The `workspace.open` session check creates an absent maintenance boundary before its blocking shared acquisition, so the method does not fail and the probe never sees an absent boundary. I traced this through `session_check_mixed_retained` → `head_mixed` → `Server::maintenance` → `initialize_repository_maintenance` and confirmed it live. The diff changes no wire format, table, code or ordering; the only normative edit is the appendix A sentence. The CLI, bridge and session-handle checkers read SMP1's declared normative revision correctly, and every mutation of the declaration failed closed. All SMP1-family checkers, vector oracles, script suites and the bounded `workspace_open` cargo tests pass.

The erratum over-reaches in one clause: version-aware entity reads do not initialize the boundary and fail `TXN_IO` without it. The regression test my revision-15 closure asked for was not added. Three P4 notes cover the omitted failure branch and wording, the split pin policy with S20-620, and the scope binding in the completion gate. None of these opens a fail-open path, and none is P0, P1 or P2, so my lane accepts revision 16. This verdict makes no GA or release-readiness claim.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_3_P4
SECTION: protocol
FIELD: vulcan_surface_review_revision_16
SCOPE_SHA: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9
FINDINGS: [P3] [spec-code] docs/spec/SMP1.md:770-774 - The erratum over-generalizes: it says the head load happens under the S20-390 acquisition "like every head-bound read", that "That acquisition first creates the maintenance boundary", and that `initialize_repository_maintenance` is "run by the session check that precedes every session-bound answer". The S20-390 acquisition does not initialize (maintenance.rs:102-110,153-157); only the server's `Server::maintenance()` wrapper (server.rs:2469-2473), reached through `head_mixed`, does. On a version-aware server, `entity.version` and `entity.signature` go server.rs:1176-1177 → `session_check_retained` (:2210-2215) → `head()` (:2479-2485) → `accepted_head` (repository.rs:881-884) and never initialize. Live probe: with the lock removed after `session.open`, both FAILED `TXN_IO` (numeric 0) and left the lock absent, while `workspace.open` and `session.budgets` re-created it. The `workspace.open` clause is correct; my revision-15 transcript made the same over-generalization. - Closure evidence: scope the parenthetical to the `maintenance()` head load (`session.open`, `workspace.open`, legacy session check), state that version-aware entity reads fail `TXN_IO` on an absent boundary, and add an anchor with a revert case. | [P3] [test-coverage] crates/sley-protocol/src/server.rs:2931-2935 - No test pins the behaviour revision 16 now states as normative: searching `crates/sley-protocol` for `maintenance.lock` returns no matches, and the server test named in my revision-15 closure evidence was not added, so the sentence can silently regress. - Closure evidence: a server test that deletes `locks/maintenance.lock` after `session.open` and asserts that `workspace.open` succeeds, re-creates the lock and answers 8 fields cold, plus an assertion for the stated entity-read outcome. | [P4] [spec-precision] docs/spec/SMP1.md:771-777, docs/adr/ADR-0032-smp1-transport-boundary.md:26-29, crates/sley-protocol/src/server.rs:2931-2935 - The corrected texts omit the failure branch of initialization (probe: a directory at the lock path gives `workspace.open` FAILED `TXN_IO` 39019). ADR-0032's "(it never fails the method, as revision 15 said)" is absolute and misattributes the claim to revision 15. The docstring's "always finds it" ignores that the guard is dropped before the probe acquires the lock again. - Closure evidence: state "created, or the method fails `TXN_IO` 39019 when it cannot be", reword the ADR to "contrary to revision 15", and qualify "always". | [P4] [pin-policy] docs/spec/SMP1.md:79-81, docs/spec/SLEY2_TRIAL_RUNNER_V1.md:51,331-332, scripts/check_sley2_trial_runner.py:155-162 - SMP1 and ADR-0032 say consumers keep their revision 15 pins, but S20-620 re-pinned document revision 16 and its checker reads the raw Status revision, so consumers follow two pin policies; the errata marker is self-declared and no gate checks that the normative text is unchanged. - Closure evidence: S20-620 pins the normative revision, or SMP1 and ADR-0032 name it as an exception; optionally add a normative-section digest check for errata revisions. | [P4] [checker] scripts/check_complete_root_index_snapshot_profile.py:223-250, scripts/check_sley2_trial_runner.py:171-198 - `scope_bound_problems` binds only the Status-line revision number at the first `on <sha>` in the note, with no ancestry check and no byte comparison, so a same-revision edit after review, or a side-branch commit carrying revision N, still binds. - Closure evidence: require `git merge-base --is-ancestor` and a contract digest equal to HEAD's, or document that the gate binds the revision number only, with a negative case for each.
SUMMARY: SMP1 revision 16's correction matches the code for `workspace.open`: the session check's `Server::maintenance()` creates an absent boundary before the blocking shared acquisition, confirmed by static trace and an out-of-tree live probe. Nothing else changed normatively, the CLI, bridge and session-handle checkers read the declared normative revision 15 and fail closed under every mutation I tried, and all SMP1-family checkers, vector oracles, script suites and the bounded `workspace_open` cargo tests pass. The erratum's parenthetical wrongly extends initialization to "every session-bound answer" (version-aware entity reads fail `TXN_IO` instead), and the requested regression test is missing (two P3); three P4 notes cover the omitted failure branch and wording, S20-620's divergent pin policy, and the completion gate's revision-number-only binding. PASS.
