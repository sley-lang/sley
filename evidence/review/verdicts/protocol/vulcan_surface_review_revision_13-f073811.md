<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f073811914297505803f5b731cd4691e1623de84; role: vulcan; field: vulcan_surface_review_revision_13; dispatched: 2026-09-23T09:14:19Z; duration_s: 801; process_exit_code: 0 -->
# Vulcan Council review — protocol

Harness: claude-code
Reviewed checkpoint: f073811914297505803f5b731cd4691e1623de84

What I verified myself:
- **Scope.** `git rev-parse HEAD` gives `f073811914297505803f5b731cd4691e1623de84`, which matches the scope SHA. Branch `work/succ-context-impl`. The worktree is clean apart from three untracked evidence paths.
- **Git commands run.**
  - `git diff --stat` and `git diff ab42a3a9..HEAD -- docs/spec/SMP1.md crates/sley-protocol docs/spec/SMP1_JSON_BRIDGE_V1.md docs/spec/SLEY_CLI_V1.md docs/spec/SESSION_HANDLE_PROFILE_V1.md` (6 files, +236/−26).
  - The same range over the checker scripts, ADR-0029, ADR-0032 and ADR-0033.
  - `git diff --stat ab42a3a9..HEAD -- conformance fuzz`: empty, so no vector or fuzz changes.
  - `git show ab42a3a9:crates/sley-protocol/src/server.rs`: at the base, line 1282 dispatches `self.workspace_open()` and line 2889 is `fn workspace_open(&self)`, so the base ignored the request body.
- **Checkers run** (python3, absolute paths):

  | Checker | Exit | Result |
  |---|---|---|
  | `check_smp1_contract.py` | 0 | PASS: revision 13, 41 methods, status `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING` |
  | `check_smp1_json_bridge_contract.py` | 0 | PASS: revision 10 |
  | `check_cli_contract.py` | 0 | PASS: revision 8 |
  | `check_session_handle_profile.py` | 0 | PASS: `smp1_revision` 13 |
  | `check_smp1_vector.py` (uv `--offline --frozen --project oracle/scb1`) | 0 | PASS: frames 3, mutations 4 |
  | `check_smp1_json_bridge_vector.py` (same uv setup) | 0 | PASS: methods 41, vectors 5, rejections 36 |

  - In memory, I verified the SHA256SUMS for `conformance/smp1/v1` and `conformance/smp1-json-bridge/v1`, `v2` and `v3`: all files ok.
- **Tests run.**

  | Command | Result |
  |---|---|
  | `python3 -m unittest scripts.test_smp1_contract` | Ran 14, OK |
  | `python3 -m unittest scripts.test_cli_contract scripts.test_current_contract_review` | Ran 13, OK |
  | `cargo test --locked --offline -p sley-protocol --lib workspace_open` | exit 0, 2 passed |
  | `cargo test --locked --offline -p sley-protocol --lib` | exit 0, 119 passed, 0 failed, 4 ignored |
  | `cargo test --locked --offline -p sley-repo --lib index_cache` | exit 0, 9 passed, including `probe_reports_only_a_materialized_snapshot_and_never_builds` |

- **Files read.**
  - `crates/sley-protocol/src/server.rs`: 411, 487–669, 770–830, 1100–1318, 2159–2210, 2435–2466, 2681–2790, 2887–2931, 3280–3316.
  - `crates/sley-protocol/src/lib.rs`: 560–589, 775–825, 960–1115.
  - `crates/sley-protocol/src/server_tests.rs`: 607–640, 2790–2830, 3205–3301, 7384–7461.
  - `crates/sley-txn/src/maintenance.rs`: 60–219.
  - `crates/sley-txn/src/repository.rs`: 881–883, 2730–2754.
  - `crates/sley-repo/src/index_cache.rs`: 1–410.
  - `crates/sley-cli/src/lib.rs`: 1085–1154.
  - `crates/sley-cli/tests/cli.rs`: 110–143.
  - `docs/spec/SMP1.md`: 1–72, 110–189, 300–392, 588–853.
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`: 140–197.
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: 500–571.
  - `docs/spec/ENTITY_READ_PROFILE_V2.md`: 1–75.
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md`: 1–56.
  - `docs/spec/SLEY_CLI_V1.md`: 1–45, 300–369.
  - `docs/spec/SESSION_HANDLE_PROFILE_V1.md`: pin and head-bound lines.
  - `scripts/check_smp1_contract.py`: 280–439.
  - `scripts/check_complete_root_index_snapshot_profile.py`: 150–219.
  - `scripts/check_cli_contract.py`: pin lines.
  - `machineresearch/sley-2.0/machine-summary.json`: the `protocol` section, loaded with Python.
- **Other lanes' transcripts.** A repo-wide grep surfaced some lines from the untracked Ariadne and Nabu revision-13 transcripts under `evidence/review/verdicts/protocol/`. I did not read or rely on them. Every finding below comes from my own trace of code, specs and checker output.

## Evidence checked

**Server behaviour at every version**
- **Version 1.** `workspace_open` now takes the body (`server.rs:2902`). A non-empty body is refused `PROTOCOL_PAYLOAD_INVALID` before the head loads (`:2903-2905`). The snapshot is probed only when `profile.protocol_version >= PROTOCOL_VERSION_V2` (`:2907`); otherwise the response is exactly `revision_summary`. Test `server_tests.rs:615-640` materializes the cache through `query.root`, then asserts that the v1 open bytes equal `revision.read` and that `[0]` is refused. Both parts pass.
- **Version 2.** The response is `head_open_summary`: the same eight fields from `revision_summary_fields`, plus field 9 as `(9, 32-byte id)` when present (`:3289-3299`). `revision_summary` still encodes the identical field list, so `revision.read` bytes are unchanged; the test asserts equality before and after. Test `server_tests.rs:7393-7461` covers:
  - cold: eight fields, `returned_entities` 1, omitted 0, not truncated, no cache file written;
  - a `query.root` refused `QUERY_SNAPSHOT_MISMATCH` still materializes the cache;
  - warm: count byte 9, tail `[9, 32]` plus the id, which equals the `query.root`-bound id, and a preimage built from that disclosure is answered;
  - a non-empty body is refused.
- **Version 3.** 201 is in `V3_ALL` (`lib.rs:568-575`), and `introduced_in` returns 1 for it (`:785-793`). `dispatch_admitted` intercepts only native test methods and commit (`server.rs:1221-1231`). So under `offered_hello_v3` with `new_versioned`, 201 reaches the same `>= V2` gate and answers `open_summary`. I derived this by static trace; I did not execute it. No test drives 201 under version 3. This is finding 1.
- **Legacy path.** The gate does not also check `self.version_aware`, unlike every other v2+ gate (`server.rs:911, 1114, 1176, 1360, 2204`). It is still unreachable there: a legacy `Server::new` whose selection is above 1 fails `check_claim` on every frame (`server.rs:1100-1105`), with DOWNGRADE for v1 frames and VERSION_UNSUPPORTED for v2 frames (`server_tests.rs:3242-3292`). So no session can bind.

**Probe claims**
- `acquire_shared_repository_maintenance_nonblocking` uses `acquire_repository_maintenance(root, false, false)`. It requires an existing real lock directory and a regular lock file, and never creates either (`maintenance.rs:148-161`). It uses `try_lock_shared`, which maps a held lock to WouldBlock (`:179-186`).
- `cached_complete_root_snapshot_id` only reads and accepts: `read_record` and `accept_cached`, with no fresh build, no write-back and no delete (`index_cache.rs:256-270`).
- The server turns every error into absence (`server.rs:2927-2930`).
- As the code comment says, the head load itself still takes the shared lock and blocks (`repository.rs:881-883`). The claim is about the probe only, and is accurate as written.

**Error precedence.** The body check runs after the session binding check (201 is head-bound, `server.rs:2167-2184`), after the budget debit and after method negotiation. It runs before the owner engine (the head load). That matches `SMP1.md:593-606`.

**Bounds.**
- `counted(summary, 1)` holds either way. The warm answer is 34 bytes longer and is charged to the budget as returned bytes.
- The probe reads at most `MAX_SNAPSHOT_RECORD_BYTES` (67,108,864) per call. That is the same cost class as a `query.root` cache hit, so it adds no new amplification path.

**Grammar and revision history.**
- Appendix A row 201 and the `open_summary` grammar (`SMP1.md:659, 693-712`) match the encoding.
- The v1 table's row 201 request column moved from "repository path digest" to "none" (`:328`).
- Appendix C is untouched. Appendix D gains one sentence (`:845-846`).

**Stage checker.** `check_complete_root_index_snapshot_profile.py:178-187` allowlists `server.rs` as the only probe caller.

**Machine summary.** `protocol.contract_revision` is 13. `current_delta_review` is `{contract_revision: 13, ariadne/nabu/vulcan: PENDING}`. `contract_complete` is false. The base review fields keep their revision 11/12 values as history.

**Consumer re-pins**
- **Bridge.** The composition sentence (`SMP1_JSON_BRIDGE_V1.md:40`) plus the stated sentence (`:46-50`). `crates/sley-json-bridge` never touches 201 or `revision_summary`. The generated `methods.json` tables carry no body text, and they are unchanged with valid checksums. A stated sentence is acceptable here.
- **Session handle.** `SESSION_HANDLE_PROFILE_V1.md:22-24`. 201 stays in the closed head-bound set (`:170`) and precedence is unchanged. A stated sentence is acceptable here.
- **CLI.** `crates/sley-cli` never touches 201's bodies, so a stated sentence is acceptable in principle. But the normative pin was not moved (finding 4).
- **Version-defining contracts.** NATIVE_TEST_ADMISSION_V1 (which defines v3) and ENTITY_READ_PROFILE_V2 (which defines v2) are neither re-pinned nor amended, and their normative text is contradicted (finding 1). A stated sentence cannot fix that. Those contracts need amending, or SMP1 or the code must change.

## Findings

[P2] [version-gate] crates/sley-protocol/src/server.rs:2907 - The field-9 gate is `protocol_version >= PROTOCOL_VERSION_V2`, so a version 3 selection (`offered_hello_v3` with `new_versioned`; 201 in `V3_ALL`, lib.rs:568-575; no v3 intercept at server.rs:1221-1231) answers `open_summary`. The documents disagree. SMP1.md:697 says "`workspace.open` under a version 2 selection only" (also :659 and :846). COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:177 names the consumer "under a version 2 selection". NATIVE_TEST_ADMISSION_V1.md:542-543 defines version 3 as the union of the SMP1 tables "unchanged at revision 12". ENTITY_READ_PROFILE_V2.md:38-40 says every existing tag's owner payload keeps its version-1 definition under version 2. Neither version-defining contract was amended or re-pinned. The bridge, CLI and session re-pin sentences repeat "changes only the version 2 `workspace.open` response body". No test drives 201 under version 3 (static trace, not executed). - Closure evidence: a decided version 3 behaviour, then either SMP1 wording covering it (e.g. "every selection above 1") plus amended or re-pinned NATIVE_TEST_ADMISSION appendix D and ENTITY_READ section 2 and corrected re-pin sentences, or a gate `== PROTOCOL_VERSION_V2`; and a version 3 server test pinning the cold and warm bytes of 201.
[P3] [compat-claim] docs/spec/SMP1.md:62-64,315-318,845-846 - SMP1 says revision 13 serves version 1 "exactly as before, byte for byte, including `workspace.open`", that "no version 1 byte changes", and that appendix A row 201 is amended "for the version 2 selection only". The same claim appears in ADR-0032:9-13 and machine-summary `protocol.status_note` ("version 1 bytes are unchanged"). But server.rs:2903-2905 now refuses a non-empty body under every version, while the base (ab42a3a9 server.rs:1282, 2889) ignored the body and answered success, and the base v1 table advertised a "repository path digest" request. ENTITY_READ_PROFILE_V2.md:69-70 requires v1 methods to keep their existing rejection behaviour. The refusal itself is correct: it is fail-closed and follows SMP1 section 4:389-392. The records misstate it. - Closure evidence: SMP1 history, ADR-0032 and the machine summary name the one v1-observable change (a non-empty 201 body, previously ignored, is now `PROTOCOL_PAYLOAD_INVALID` under every version, per section 4); appendix D's "version 2 selection only" sentence is corrected; ENTITY_READ section 2:69-70 is reconciled.
[P3] [evidence-integrity] docs/spec/SMP1.md:700-704 - SMP1 says field 9 is "the identity `query.root`, `query.continue`, and `capsule` bind for that head". Field 9 is the id of a cached record accepted without re-deriving edges (index_cache.rs:106-126, 256-270), while `capsule` always builds fresh (server.rs:2758-2771). A digest-valid forged record with the right inventory is the residual S20-300 accepts (COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:144-147). With such a record, field 9 names an id that `query.root` answers with forged edges and that `capsule` refuses with `QUERY_SNAPSHOT_MISMATCH`. SMP1 drops the owner's "pointer, not evidence" caveat (:180-182). A client that adopts field 9 instead of a freshly derived id loses the mismatch cross-check that caught such records under revision 12. - Closure evidence: SMP1 row 201 prose carries the pointer-not-evidence sentence and states that `capsule` binds the fresh identity, which equals field 9 only when the cache is honest.
[P3] [pin] docs/spec/SLEY_CLI_V1.md:26 - The CLI's normative composition sentence still reads "`docs/spec/SMP1.md` revision 12", while the appended re-pin note at :357-359 says revision 13. scripts/check_cli_contract.py:226 only looks for the substring "SMP1 revision 13 and bridge revision 10", which the appended note satisfies, so the stale pin passes the gate. - Closure evidence: line 26 moved to revision 13, and a checker assertion anchored on the composition sentence's SMP1 pin (with a negative test).
[P4] [fail-closed] crates/sley-protocol/src/server.rs:1242-1257,1265-1266,1275 - The other appendix A "empty" rows (102, 103, 104, 210, 214, 504) still ignore a non-empty body, contrary to SMP1 section 4:389-392. Revision 13 adds an explicit refusal clause only to row 201 (SMP1.md:659), so "empty" now reads two ways. This predates revision 13 and is not agent-reachable beyond the two session reads. - Closure evidence: refuse non-empty bodies on every "empty" row (with a stated v1 note), or state in appendix A that those rows ignore the body.
[P4] [determinism] docs/spec/SMP1.md:693-694,746-747,624-625 - `open_summary` marks field 9 optional by absence ("-- field 9 optional"). Appendix A's declared conventions (:641-645) list only the SSMC1 option union, although the prose at :708-709 settles the encoding. Separately, appendix B's "equal frame lists over equal repository state produce equal answer sequences" and section 10's "byte-identical responses across runs" are not scoped for a body that now depends on the disposable derived cache and on maintenance contention. - Closure evidence: add absent-trailing-field optionality to the appendix A conventions, and scope the determinism statements for 201 under version 2 and above.

## Assessment

The version 1 and version 2 wire behaviour of `workspace.open` is correct, fail-closed and well tested:
- Field 9 is present only on a warm, accepted cache.
- Absence is structural: eight fields, no omission or truncation signal.
- The probe never builds, writes, deletes, initializes or waits on the maintenance boundary, and every probe failure becomes absence.
- The body refusal sits at the right point in the error precedence.
- `revision.read` stays byte-identical.
- Frozen conformance vectors and checksums still verify, and all SMP1-family checkers and suites pass.

The contract does not yet describe what the code does across versions:
- **Version 3** (the P2). The code serves field 9 under version 3, which SMP1, S20-300 and the three re-pin sentences all exclude. The two contracts that define versions 2 and 3 still say legacy tags are unchanged.
- **Version 1** (a P3). The version 1 body refusal is a real observable change, yet it is described as none.

**Re-pin ruling:**
- A stated sentence without a revision bump is acceptable for the JSON bridge and the session-handle profile, since no clause either owns changes.
- It is also acceptable for the CLI once line 26 is actually moved.
- The shared rationale sentence must be corrected along with the version gate.
- NATIVE_TEST_ADMISSION_V1 and ENTITY_READ_PROFILE_V2 need real amendments, not a sentence, unless the gate is narrowed to version 2 and the v1 compatibility wording is fixed.

**Out of scope, not counted (for the S20-300 owner):**
- The doc comment at `index_cache.rs:246` still says "metadata-only" cost class, while S20-300 section 5 now states the bounded decode.
- The `read_record` comment at `:272-276` says the open handle defeats a planted-symlink swap. In fact `File::open` follows symlinks, and the post-open check only requires the target to be a regular file.

This verdict makes no claim about GA or release readiness.

VERDICT: REVISE_0_P0_0_P1_1_P2_3_P3_2_P4
SECTION: protocol
FIELD: vulcan_surface_review_revision_13
SCOPE_SHA: f073811914297505803f5b731cd4691e1623de84
FINDINGS:
[P2] [version-gate] crates/sley-protocol/src/server.rs:2907 - The field-9 gate is `protocol_version >= PROTOCOL_VERSION_V2`, so a version 3 selection (`offered_hello_v3` with `new_versioned`; 201 in `V3_ALL`, lib.rs:568-575; no v3 intercept at server.rs:1221-1231) answers `open_summary`. The documents disagree. SMP1.md:697 says "`workspace.open` under a version 2 selection only" (also :659 and :846). COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:177 names the consumer "under a version 2 selection". NATIVE_TEST_ADMISSION_V1.md:542-543 defines version 3 as the union of the SMP1 tables "unchanged at revision 12". ENTITY_READ_PROFILE_V2.md:38-40 says every existing tag's owner payload keeps its version-1 definition under version 2. Neither version-defining contract was amended or re-pinned. The bridge, CLI and session re-pin sentences repeat "changes only the version 2 `workspace.open` response body". No test drives 201 under version 3 (static trace, not executed). - Closure evidence: a decided version 3 behaviour, then either SMP1 wording covering it (e.g. "every selection above 1") plus amended or re-pinned NATIVE_TEST_ADMISSION appendix D and ENTITY_READ section 2 and corrected re-pin sentences, or a gate `== PROTOCOL_VERSION_V2`; and a version 3 server test pinning the cold and warm bytes of 201.
[P3] [compat-claim] docs/spec/SMP1.md:62-64,315-318,845-846 - SMP1 says revision 13 serves version 1 "exactly as before, byte for byte, including `workspace.open`", that "no version 1 byte changes", and that appendix A row 201 is amended "for the version 2 selection only". The same claim appears in ADR-0032:9-13 and machine-summary `protocol.status_note` ("version 1 bytes are unchanged"). But server.rs:2903-2905 now refuses a non-empty body under every version, while the base (ab42a3a9 server.rs:1282, 2889) ignored the body and answered success, and the base v1 table advertised a "repository path digest" request. ENTITY_READ_PROFILE_V2.md:69-70 requires v1 methods to keep their existing rejection behaviour. The refusal itself is correct: it is fail-closed and follows SMP1 section 4:389-392. The records misstate it. - Closure evidence: SMP1 history, ADR-0032 and the machine summary name the one v1-observable change (a non-empty 201 body, previously ignored, is now `PROTOCOL_PAYLOAD_INVALID` under every version, per section 4); appendix D's "version 2 selection only" sentence is corrected; ENTITY_READ section 2:69-70 is reconciled.
[P3] [evidence-integrity] docs/spec/SMP1.md:700-704 - SMP1 says field 9 is "the identity `query.root`, `query.continue`, and `capsule` bind for that head". Field 9 is the id of a cached record accepted without re-deriving edges (index_cache.rs:106-126, 256-270), while `capsule` always builds fresh (server.rs:2758-2771). A digest-valid forged record with the right inventory is the residual S20-300 accepts (COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:144-147). With such a record, field 9 names an id that `query.root` answers with forged edges and that `capsule` refuses with `QUERY_SNAPSHOT_MISMATCH`. SMP1 drops the owner's "pointer, not evidence" caveat (:180-182). A client that adopts field 9 instead of a freshly derived id loses the mismatch cross-check that caught such records under revision 12. - Closure evidence: SMP1 row 201 prose carries the pointer-not-evidence sentence and states that `capsule` binds the fresh identity, which equals field 9 only when the cache is honest.
[P3] [pin] docs/spec/SLEY_CLI_V1.md:26 - The CLI's normative composition sentence still reads "`docs/spec/SMP1.md` revision 12", while the appended re-pin note at :357-359 says revision 13. scripts/check_cli_contract.py:226 only looks for the substring "SMP1 revision 13 and bridge revision 10", which the appended note satisfies, so the stale pin passes the gate. - Closure evidence: line 26 moved to revision 13, and a checker assertion anchored on the composition sentence's SMP1 pin (with a negative test).
[P4] [fail-closed] crates/sley-protocol/src/server.rs:1242-1257,1265-1266,1275 - The other appendix A "empty" rows (102, 103, 104, 210, 214, 504) still ignore a non-empty body, contrary to SMP1 section 4:389-392. Revision 13 adds an explicit refusal clause only to row 201 (SMP1.md:659), so "empty" now reads two ways. This predates revision 13 and is not agent-reachable beyond the two session reads. - Closure evidence: refuse non-empty bodies on every "empty" row (with a stated v1 note), or state in appendix A that those rows ignore the body.
[P4] [determinism] docs/spec/SMP1.md:693-694,746-747,624-625 - `open_summary` marks field 9 optional by absence ("-- field 9 optional"). Appendix A's declared conventions (:641-645) list only the SSMC1 option union, although the prose at :708-709 settles the encoding. Separately, appendix B's "equal frame lists over equal repository state produce equal answer sequences" and section 10's "byte-identical responses across runs" are not scoped for a body that now depends on the disposable derived cache and on maintenance contention. - Closure evidence: add absent-trailing-field optionality to the appendix A conventions, and scope the determinism statements for 201 under version 2 and above.
SUMMARY: SMP1 revision 13's version 1 and version 2 `workspace.open` behaviour is correct and tested: field 9 only on a warm accepted cache, structural absence otherwise, and a probe that never builds, writes, creates or waits on the lock. All SMP1-family checkers, frozen vectors, checksums and suites pass. The code also serves field 9 under version 3, which SMP1, S20-300 and the version-defining contracts (NATIVE_TEST_ADMISSION appendix D, ENTITY_READ section 2) exclude or leave unamended; the version 1 non-empty-body refusal is misreported as no change; SMP1 overstates field 9 as the identity `capsule` binds; and the CLI's normative SMP1 pin still reads revision 12. Bridge and session-handle pin-only re-pins are acceptable as stated sentences, and the CLI's will be once line 26 moves, but the version-defining contracts need real amendments; REVISE.
