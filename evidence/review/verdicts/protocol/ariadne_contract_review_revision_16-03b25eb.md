<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9; role: ariadne; field: ariadne_contract_review_revision_16; dispatched: 2026-09-23T14:33:54Z; duration_s: 450; process_exit_code: 0 -->
# Ariadne Council review — protocol

Harness: claude-code
Reviewed checkpoint: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `03b25eb9b3d7e80576394109ca4b73cc21b2fab9`, which matches the scope, and the tree was clean.
- **Git commands.** I ran `git log --oneline 26d050e6..HEAD` (8 commits). I ran `git diff --stat` and `git diff 26d050e6..HEAD` on `docs/spec/SMP1.md crates/sley-protocol scripts/`, and also on ADR-0032, ADR-0036, `SLEY2_TRIAL_RUNNER_V1.md` and `WORK_PACKAGES.md`. I ran `git show --stat 33864b96`, and `git show 26d050e6:machineresearch/sley-2.0/machine-summary.json` to diff the `protocol` section.
- **Checkers.** Every one below exited with the result shown:

| Command | Exit | Result |
|---|---|---|
| `check_smp1_contract.py` | 0 | PASS, revision 16, `problems: []` |
| `check_smp1_json_bridge_contract.py` | 0 | PASS, revision 12 |
| `check_cli_contract.py` | 0 | PASS, revision 10 |
| `check_session_handle_profile.py` | 0 | PASS, `smp1_revision: 15` |
| `check_complete_root_index_snapshot_profile.py` | 0 | PASS, revision 6 |
| `check_sley2_trial_runner.py` | 0 | PASS, revision 8 |
| `uv run --project oracle/scb1 python scripts/check_smp1_vector.py` | 0 | PASS, 3 frames, 4 mutations |
| `uv run … check_smp1_json_bridge_vector.py` | 0 | PASS, 41 methods, 5 vectors, 37 rejections |
| `check_smp1_persistent_fuzz_slice.py` | 0 | PASS |
| `check_smp1_json_bridge_persistent_fuzz_slice.py` | 0 | PASS |

  The bare `python3 scripts/check_smp1_vector.py` exits 1 on a missing `blake3` module. That is an environment issue: the same checker passes under uv.
- **Unit tests.** All exited 0:
  - `test_smp1_contract`: 18 OK
  - `test_cli_contract`: 10 OK
  - `test_smp1_json_bridge_contract`: 4 OK
  - `test_session_handle_profile`: 12 OK
  - `test_sley2_trial_runner`: 7 OK
  - `test_complete_root_index_snapshot_profile`: 23 OK
  - `test_smp1_json_bridge_table.py`, run directly: 17 cases PASS
- **Cargo.**
  - `cargo test -p sley-cli --test cli handshake_identity_does_not_depend_on_the_transport_flag`: 1 passed. The build was a fingerprint no-op, so `debug/sley` is current for this tree.
  - `cargo test -p sley-protocol workspace_open`: 5 passed.
- **Files read:**
  - `docs/spec/SMP1.md:1-104, 740-799`
  - `crates/sley-protocol/src/server.rs:1140-1310, 2200-2489, 2890-2973`
  - `crates/sley-txn/src/maintenance.rs:1-190`
  - `crates/sley-txn/src/repository.rs:870-1029, 4055-4061, 4445-4454`
  - `scripts/check_smp1_contract.py:380-465`
  - `scripts/check_sley2_trial_runner.py:146-168`
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:200-229`
  - `bench/sley2/runner.py:296-510, 1285-1310, 1426-1502`
  - `evidence/review/verdicts/protocol/vulcan_surface_review_revision_15-26d050e.md:95-114`
- **Probes.** None of them changed the tree:
  - In-memory checker probes: `Path.read_text` was monkeypatched on SMP1 for the CLI, bridge, session-profile and SMP1 checkers.
  - A live endpoint probe against a temporary repository outside the tree, deleting `locks/maintenance.lock` between requests.

## Evidence checked

**1. Diff scope.**
- Inside `crates/`, the only change since 26d050e6 is a docstring at `server.rs:2929-2935`. No executable Rust line changed, which supports "no behaviour change".
- In SMP1 the changes are:
  - the Status line (:3)
  - the composition label (:21; the bridge 12 and CLI 10 pins are unchanged)
  - the revision-16 history entry (:75-81)
  - the review-status sentence (:101-103)
  - the appendix A clause (:769-777)
- No table, wire record, code, limit or version rule changed. The script changes are the errata-aware pin readers, the SMP1 `NORMATIVE_REVISION` constant with its test, and the S20-300/S20-620 scope-bound completion gates. Those gates belong to other packages and are not SMP1 normative text.

**2. Code path for the corrected clause.** Each step, in order:
- Dispatch sends `Method::WorkspaceOpen` to `session_check_mixed_retained` (`server.rs:1184-1186`).
- That goes to `head_binding_mixed` (:2415, :2375-2376), then to `head_mixed` (:2391-2392).
- `head_mixed` calls `Server::maintenance()` (:2469-2473). That runs `initialize_repository_maintenance`, which creates `locks/maintenance.lock` and fsyncs it (`maintenance.rs:50-100`), and then takes the blocking `acquire_shared_repository_maintenance` (:107-111, 177-178).
- `accepted_head_any_with_maintenance` follows (`repository.rs:999-1011`). For the V1 branch, `self.head()` runs (`server.rs:2398`).
- The answer is built from the retained head (:2944-2947). The probe then takes the non-blocking, non-initializing guard (:2969).
- The clause's other two claims also hold: an exclusive owner makes the opener wait (a blocking shared lock), and the probe adds no wait and never initializes the boundary.

**3. Live confirmation** (HEAD-current `debug/sley`, exchange-fixture repository, lock unlinked after `session.open`):
- `workspace.open` under both v2 and v1 answered `failed False`, with a 211-byte body byte-identical to the lock-present answer and `returned_entities 1`. The lock was recreated.
- The revision-16 text is therefore correct for method 201, and revision 15's "fails the method" was indeed false.

**4. Over-generalization probe.** The new text generalizes the behaviour to "every head-bound read" and "every session-bound answer". To test that, I unlinked the lock and sent v2 `entity.version` and `entity.signature`:
- Both answered failed with `TXN_IO`; the body was `0601010002070654584e5f494f03010004010105020000060100`. The lock stayed absent.
- `session.budgets` then recreated the lock.
- With the lock present, `entity.version` reached the body layer and answered 40008.
- The cause: these methods are checked by `session_check_retained`, which goes `head_binding` → `Server::head` → `TransactionRepository::accepted_head` (`server.rs:2210-2215, 2360-2361, 2479-2485`; `repository.rs:881-884`). `accepted_head` requires an existing boundary (`maintenance.rs:102-106, 153-157`).

**5. Errata classification.**
- The only consumer that relied on the false sentence is S20-620 §9, which re-pinned and corrected in place. S20-300 at :210-213 is consistent with the new text.
- The CLI, bridge and session-profile contracts carry no absent-boundary text.
- Classifying revision 16 as errata-only, with consumers keeping the normative pin of 15, is therefore sound in substance.

**6. Consumer checkers.** `check_cli_contract.py:303-312`, `check_smp1_json_bridge_contract.py:343-352` and `check_session_handle_profile.py:469-478` read the declared `errata-only over normative revision N`, and otherwise fall back to the Status line's revision. The in-memory probes gave:

| Variant | CLI, bridge and session checkers | SMP1 checker |
|---|---|---|
| Current text | exit 0 | exit 0 |
| Declaration dropped | exit 1, `smp1-revision-pin` | exit 1, `spec-normative-revision:16!=15` |
| Declaration names revision 14 | exit 1, `smp1-revision-pin` | exit 1, `spec-normative-revision:14!=15` |
| Date prefix removed | exit 0 | exit 0 |

  The NATIVE appendix D owner pin is read against `NORMATIVE_REVISION` 15 (`check_smp1_contract.py:93`). All consumers fail closed and none needs a re-pin.

**7. Records.**
- In the machine summary, `protocol.contract_revision` is 16, `current_delta_review` is bound to revision 16 with all lanes PENDING, and the revision-15 lane fields are PASS x3, each note naming `on 26d050e6…`.
- `p3_open` holds the three ENTITY_READ §2 items. Vulcan's revision-15 absent-boundary P3 has correctly left the open list.
- ADR-0032:24-32 and `WORK_PACKAGES.md` S20-400 match SMP1.

## Findings
[P3] [spec-code] docs/spec/SMP1.md:770-775 - The erratum over-generalizes. "like every head-bound read. That acquisition first creates the maintenance boundary … run by the session check that precedes every session-bound answer" is false for the version-aware `entity.version` (306) and `entity.signature` (307). Both are head-bound (`server.rs:2202-2205`), and their session check `session_check_retained` (:2210-2215) loads through `Server::head` → `accepted_head` (:2360-2361, :2479-2485; `repository.rs:881-884`), which never initializes the boundary. The live probe at HEAD confirms it: with the lock removed, both methods fail `TXN_IO` and leave the lock absent. The text also attributes creation to "the S20-390 blocking shared maintenance acquisition", which by its own contract fails on an absent boundary (`maintenance.rs:102-106`). Creation is the S20-410 `Server::maintenance()` composition (`server.rs:2469-2473`). The clause is correct for `workspace.open` itself. - Closure evidence: scope the sentence to the `workspace.open` session check (`session_check_mixed_retained` → `head_mixed` → `Server::maintenance`), and either drop "every head-bound read" and "every session-bound answer" or state the entity-read exception; ideally add a test pinning the 306/307 absent-boundary outcome.
[P4] [test-coverage] crates/sley-protocol/src/server.rs:2929-2935 - No test in `crates/sley-protocol` removes `locks/maintenance.lock` (grep for `maintenance.lock`, `"locks"`, `remove_file` and `remove_dir_all` returns nothing), so the corrected clause (SMP1:769-777) and this docstring are pinned only by my live probe. Vulcan's revision-15 closure evidence asked for such a test ("either way"). A regression that swaps `Server::maintenance()` for a non-initializing acquire would silently make the clause false again. - Closure evidence: add a `server_tests` case that deletes the lock after `session.open` and asserts that `workspace.open` succeeds, returns a byte-identical body and recreates the lock.
[P4] [pin-discipline] docs/spec/SMP1.md:79-81 - "consumers keep their revision 15 pins (the normative revision)" is stated for all consumers; ADR-0032:29 and `protocol.status_note` say the same. However, S20-620 pins the physical revision 16 (`SLEY2_TRIAL_RUNNER_V1.md:51, 332`), and its checker requires the Status-line revision (`check_sley2_trial_runner.py:155-163`), while the CLI, bridge and session-profile checkers read the declared normative revision. Two pinning rules now coexist, so the next errata-only revision will turn only S20-620 red. - Closure evidence: SMP1 names which consumers keep the normative pin and records that S20-620 cites the corrected text at revision 16, or the S20-620 checker adopts the declared-normative reader.
[P4] [test-coverage] scripts/check_cli_contract.py:303-312 - The errata-aware Status regex is copied into three consumer checkers (here, `check_smp1_json_bridge_contract.py:343-352` and `check_session_handle_profile.py:469-478`). None of their own suites (`test_cli_contract.py`, `test_smp1_json_bridge_contract.py`, `test_session_handle_profile.py`) exercises the new branch; only `test_smp1_contract.py:330-343` covers the SMP1 side. My in-memory probes show all three behave correctly and fail closed; the gap is regression coverage of logic that exists in three copies. - Closure evidence: a shared helper, or per-consumer negative cases (declaration dropped, and a declaration naming a different normative revision, both give `smp1-revision-pin`).

## Assessment
- **Accepted.** Revision 16 does what it claims for its subject. Revision 15's appendix A sentence that an absent boundary fails `workspace.open` was false. The new text matches the code path traced statically and confirmed live under v1 and v2: the session check → `head_mixed` → `Server::maintenance` → `initialize_repository_maintenance` sequence produces a successful, byte-identical answer with the boundary recreated. No executable code, wire record, table or version rule changed, so the errata-only classification and the retained normative pin of 15 are justified.
- **Consumers.** The CLI, bridge and session-profile checkers read the declared normative revision and fail closed on a missing or mismatched declaration. The SMP1 checker guards the declaration against `NORMATIVE_REVISION`. All SMP1-family checkers, vectors, fuzz-slice gates and unittest suites pass.
- **Remaining issue.** The corrected sentence generalizes "initializes the absent boundary" to every head-bound read and every session-bound answer, which the live probe disproves for version-aware 306/307. That is the same kind of untraced boundary claim this erratum was meant to remove, but it does not affect method 201's contract, so it is P3.
- **Minor note (no finding).** "the probe always finds it" in the docstring holds for the single-threaded server, but an external deletion between the session check and the probe would still be structural absence, which SMP1 already covers.

This lane accepts revision 16 with the findings below. It makes no GA or release claim.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_3_P4
SECTION: protocol
FIELD: ariadne_contract_review_revision_16
SCOPE_SHA: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9
FINDINGS: [P3] [spec-code] docs/spec/SMP1.md:770-775 - erratum over-generalizes: "like every head-bound read … run by the session check that precedes every session-bound answer" is false for version-aware entity.version/entity.signature, whose session_check_retained → Server::head → accepted_head path (server.rs:2210-2215,2360-2361,2479-2485; repository.rs:881-884) never initializes the boundary (live probe at HEAD: TXN_IO, lock left absent), and creation is misattributed to the S20-390 blocking acquisition (maintenance.rs:102-106) rather than Server::maintenance (server.rs:2469-2473); correct for workspace.open - Closure evidence: scope the sentence to the workspace.open session check (session_check_mixed_retained → head_mixed → Server::maintenance) or state the 306/307 exception, ideally with a pinning test | [P4] [test-coverage] crates/sley-protocol/src/server.rs:2929-2935 - no sley-protocol test deletes locks/maintenance.lock, so the corrected clause (SMP1:769-777) is pinned only by a reviewer probe; Vulcan r15's closure evidence asked for such a test - Closure evidence: server_tests case deleting the lock after session.open and asserting that workspace.open succeeds, returns an identical body and recreates the lock | [P4] [pin-discipline] docs/spec/SMP1.md:79-81 - "consumers keep their revision 15 pins" (also ADR-0032:29, protocol.status_note) is stated for all consumers, but S20-620 pins physical revision 16 (SLEY2_TRIAL_RUNNER_V1.md:51,332) via a Status-line reader (check_sley2_trial_runner.py:155-163) while CLI/bridge/session read the declared normative revision - Closure evidence: SMP1 names the normative-pin consumers and the S20-620 exception, or S20-620 adopts the declared-normative reader | [P4] [test-coverage] scripts/check_cli_contract.py:303-312 - the errata-aware regex is copied into three consumers (also check_smp1_json_bridge_contract.py:343-352, check_session_handle_profile.py:469-478) with no negative case in their own suites; behaviour verified correct by in-memory probes - Closure evidence: shared helper or per-consumer drop/mismatch cases asserting smp1-revision-pin
SUMMARY: SMP1 revision 16 correctly replaces revision 15's false claim that an absent maintenance boundary fails workspace.open. The static trace and a live endpoint probe show that the session check initializes the boundary and the opener answers byte-identically, with no executable, wire or table change, so errata-only with a normative pin of 15 is justified. The CLI, bridge and session checkers read the declared normative revision and fail closed, and all SMP1-family checkers, vectors, fuzz-slice gates and suites pass. One P3 remains: the new sentence claims every head-bound read and session-bound answer initializes the boundary, which is false for version-aware entity.version/entity.signature. Three P4 notes cover test coverage and pin discipline.
