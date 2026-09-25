<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: vulcan; field: vulcan_surface_review_revision_15; dispatched: 2026-09-23T13:52:15Z; duration_s: 588; process_exit_code: 0 -->
# Vulcan Council review — protocol

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

What I verified myself:

**Scope**
- `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, which matches the scope SHA.
- Branch is `work/succ-context-impl`. The worktree is clean except for untracked verdict transcripts from other lanes, which I did not read.
- The only prior transcript I read is my lane's `evidence/review/verdicts/protocol/vulcan_surface_review_revision_14-2b0f1c9.md`.
- `/tmp` inode use was 10%.

**Git commands run**
- `git log --oneline 2b0f1c9f..HEAD`: 12 commits.
- `git diff --stat 2b0f1c9f..HEAD`: 86 files.
- Full diffs over the same range for:
  - `docs/spec/SMP1.md` and `docs/spec/NATIVE_TEST_ADMISSION_V1.md`
  - `scripts/check_smp1_contract.py` and `scripts/test_smp1_contract.py`
  - `crates/sley-protocol/` (server.rs, server_tests.rs)
  - `scripts/check_complete_root_index_snapshot_profile.py` and its test
  - `docs/spec/ENTITY_READ_PROFILE_V2.md`, `docs/adr/ADR-0032-smp1-transport-boundary.md` and `docs/WORK_PACKAGES.md`
  - the machine-summary note lines
- `git blame -L1046,1055 crates/sley-protocol/src/lib.rs`
- `git log -1 116d031e`

**Checkers run** (python3 from the worktree; vector oracles through `uv run --offline --frozen --project oracle/scb1`)

| Checker | Exit | Result |
|---|---|---|
| `check_smp1_contract.py` | 0 | PASS: revision 15, 41 methods, status `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING` |
| `check_complete_root_index_snapshot_profile.py` | 0 | PASS: revision 6 |
| `check_native_test_vectors.py` | 0 | "6 requests, 6 responses, 7 rejections OK" |
| `check_smp1_json_bridge_contract.py` | 0 | PASS: revision 12 |
| `check_session_handle_profile.py` | 0 | PASS: revision 6, `smp1_revision` 15 |
| `check_cli_contract.py` | 0 | PASS: revision 10 |
| `check_smp1_vector.py` | 0 | PASS: frames 3, mutations 4 |
| `check_smp1_json_bridge_vector.py` | 0 | PASS: methods 41, vectors 5, rejections 37 |
| `check_entity_read_vectors.py` | 0 | PASS: cases 23, rejections 91 |
| `generate_smp1_json_bridge_table.py --check --protocol-version 1` / `2` / `3` | 0 each | PASS |

**Tests run**

| Command | Result |
|---|---|
| `python3 -m unittest scripts.test_smp1_contract scripts.test_complete_root_index_snapshot_profile scripts.test_session_handle_profile scripts.test_cli_contract scripts.test_current_contract_review` | Ran 65, OK |
| `cargo test --locked --offline -p sley-protocol --lib` | 124 passed, 0 failed, 4 ignored |

- I also ran two in-memory probes of `probe_gate_problems`. Each used the suite's own `scratch_tree()` fixture with a mutated lib.rs or consumer.

**Files read**
- `docs/spec/SMP1.md`: 60–96, 225–264, 720–781.
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: 1–20, 352–373, 552–560.
- `docs/spec/ENTITY_READ_PROFILE_V2.md`: 1–90.
- `docs/spec/SLEY2_TRIAL_RUNNER_V1.md`: 320–339.
- `crates/sley-protocol/src/lib.rs`: 1–30, 540–619, 728–817, 855–914, 990–1139, 3500–3589.
- `crates/sley-protocol/src/server.rs`: 1130–1240, 2370–2499, 2912–2986.
- `crates/sley-protocol/src/server_tests.rs`: diff hunks at 607–614, 7461–7467 and 7584–7686.
- `crates/sley-txn/src/maintenance.rs`: 40–189.
- `crates/sley-txn/src/repository.rs`: 875–914, 995–1024, 4050–4069.
- `scripts/check_complete_root_index_snapshot_profile.py`: 88–219.
- `scripts/check_smp1_contract.py`: 50–91, 333–339.
- `scripts/test_smp1_contract.py`: 280–329.
- `scripts/test_complete_root_index_snapshot_profile.py`: 45–57, 102–118, plus the diff.
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: §13, the headings and rows at 784–957, especially 891–931.
- `machineresearch/sley-2.0/machine-summary.json`: the `protocol` section loaded with Python, and lines 2007–2113.

## Evidence checked

**My lane's revision-14 findings: status lines**

- Revision-14 finding 1, [P3] [spec-code] SMP1.md:227-230 (the per-selection negotiation filter): **CLOSED.**
  - SMP1 §2 (:240-251) now states the filter exactly:
    - version 1 removes 306, 307, 605, 606 and 607;
    - version 2 removes 605, 606 and 607;
    - version 3 without the native-tests bit removes 601, 602, 605, 606 and 607.
  - Every other opaque unknown tag stays. A re-deriving peer must apply the same filter.
  - I traced this against `negotiate_versioned` (lib.rs:1075-1097) and the tag constants (lib.rs:40-52). They match exactly.
  - The unit test at lib.rs:3518-3563 pins all three branches and the retention of 999/100.
  - The version 1 compatibility statement (:76-86) counts the new drop as the second observable change: the methods, the handshake identity and the `session.capabilities` bytes change on the explicit path only, while the legacy entrypoints keep the tags. The drop is in the code since 116d031e (2026-09-16), which is after revision 12.
  - Anchors `per-selection-filter` and `version-1-native-filter-compat` were added (check_smp1_contract.py:71-78), with three non-vacuous revert cases (test_smp1_contract.py:301-306, 312).
  - NATIVE revision 7 re-pins appendix D to "`docs/spec/SMP1.md` at revision 15". It is the single pin, and `v3-owner-pin` requires exactly one. Appendix C :370 supports the citation.
  - My closure criterion is met. A residual in ENTITY_READ is filed separately as new finding 2.
- Revision-14 finding 2, [P4] [stale-doc] server_tests.rs:610-613: **CLOSED.** The docstring (:610-614) now says field 9 "applies under version 2 and every later selection whose table carries version 2's row 201".
- Revision-14 finding 3, [P4] [surface] server.rs:2925 (`materialized_head_snapshot` visibility): **CLOSED**, by the gate option I named.
  - `probe_gate_problems` now requires exactly two wrapper references in server.rs, the definition plus one call inside `fn workspace_open(` (:210-216).
  - Any other file that names the wrapper is refused. The only exemptions are crate integration tests and `server_tests.rs` while it is `#[cfg(test)]` (:152-159, :193-194).
  - Negative tests exist and pass: `test_a_second_wrapper_caller_is_refused`, `test_the_wrapper_named_in_another_file_is_refused` and `test_the_server_test_module_may_name_the_wrapper_only_while_cfg_test`.
  - At HEAD the only references are server.rs:2947 (the call) and :2963 (the definition), plus server_tests.rs:7565 and :7570, with lib.rs:17-18 `#[cfg(test)] mod server_tests;`.
  - Residual token bypasses are filed as new finding 3.

**Code delta: `workspace.open` answers from the retained head (004cf5be)**
- For `WorkspaceOpen` the session check now returns the `HeadRevision` it loaded (server.rs:1183-1189, 2410-2420), and `workspace_open` uses it for V1 heads (:2942-2945).
- The binding and the answer come from the same `VerifiedRevision`: the `head_mixed` V1 branch at :2398 returns `self.head()`, and both the binding and the retained value derive from it.
- A native head still falls to `self.head()` and fails through the v1 loader, as before.
- Precedence is unchanged: the session check and head load, then the budget, then the body check.
- `retained` is set only on success. One fewer head verification is performed per answer.
- There is no new fail-open path and no new bound exposure.

**Absent-boundary sentence (new finding 1)**
- `head_mixed` (server.rs:2391-2392) calls `Server::maintenance()`, and that calls `initialize_repository_maintenance` (:2469-2473; maintenance.rs:40-99: creates `locks/maintenance.lock`, then fsync).
- It then calls `accepted_head_any_with_maintenance` and `self.head()`, which find the boundary present.
- So SMP1's "an absent boundary fails the method" does not hold on the server path.

**Machine summary**
- `protocol.contract_revision` is 15, and `current_delta_review` is bound to 15 with all lanes PENDING.
- `status_note` matches the delta.
- The revision-14 lane fields are kept as history.

## Findings

[P3] [spec-code] docs/spec/SMP1.md:763-766 - The sentence "so an absent boundary fails the method" is false on the server path. Before any head load, every session-checked method, `workspace.open` included, calls `head_mixed` (crates/sley-protocol/src/server.rs:2391-2392), which calls `Server::maintenance()` (:2469-2473), which calls `initialize_repository_maintenance`. That call creates a missing `locks/maintenance.lock` and syncs it (crates/sley-txn/src/maintenance.rs:40-99). The accepted-head reads that follow (crates/sley-txn/src/repository.rs:999-1011, 881-884) find the lock present, the method succeeds, and the probe sees a present boundary. Only `TransactionRepository::accepted_head` on its own refuses an absent lock (maintenance.rs:153-157). So the opener performs a durable create where the contract promises a fail-closed refusal. This round copied the claim into the new server.rs:2931-2933 docstring ("takes the boundary shared, blocking, and fails the method"). docs/spec/SLEY2_TRIAL_RUNNER_V1.md:330-332 also relies on it, citing SMP1 appendix A. The S20-620 Ariadne P4 closure (gate record :931) rests on it. My revision-14 transcript accepted this sentence without tracing `Server::maintenance()`. This is a static trace, not executed: no test in the tree removes the boundary on the server path. - Closure evidence: SMP1 :763-766, the server docstring and S20-620 §9 state that the server's head load initializes an absent boundary before its blocking shared acquisition, and that only an invalid boundary or an absent layout fails the method. Alternatively, the read path takes a non-initializing acquisition. Either way, add a server test that deletes `locks/maintenance.lock` after `session.open` and asserts the stated `workspace.open` outcome.
[P3] [cross-contract] docs/spec/ENTITY_READ_PROFILE_V2.md:43-44,59-62,78-80 - This round amended ENTITY_READ: :3-5 now says the section 2 amendment is "covered only by the SMP1 revision 14 and 15 review rounds", and :43-44 cites "SMP1 revisions 13 to 15". Section 2 still says three things SMP1 revision 15 contradicts: that the version-aware entrypoint "filters 306 and 307 from the intersection when the selected version is 1" and "does not filter other opaque unknown numeric tags"; that there is "one later exception"; and that the non-empty 201 refusal is "the recorded exception" to v1 negotiation byte stability. SMP1 revision 15 §2 (:240-251) and its history (:79-86) instead state the 605-607 drop at versions 1 and 2, and a second version 1 observable change to the handshake identity. `negotiate_versioned` names ENTITY_READ section 2 as its contract (lib.rs:1043-1044). So two normative texts disagree on the handshake preimage. - Closure evidence: ENTITY_READ section 2 states the per-selection filter (or defers to SMP1 §2 for it) and lists the version 1 native-tag drop beside the 201 exception, preferably with a check_smp1_contract anchor over ENTITY_READ and a revert case.
[P4] [checker] scripts/check_complete_root_index_snapshot_profile.py:101,103-106,156-158,208 - The gate that closes my revision-14 visibility P4 has two token-level bypasses, contrary to its stated aim that literals and comments never fake a token. (a) The `server_tests.rs` exemption searches the raw lib.rs text for `#[cfg(test)] mod server_tests;` (:156-158), not the `strip_rust` output. In memory, a lib.rs holding that text in a comment or a string literal, plus an unconditional `mod server_tests;` or `pub mod server_tests;`, returned `[]` while `server_tests.rs` called the wrapper; the unconditional declaration alone is refused. (b) `BLOCKING` (:103-106) misses the UFCS form `Self::maintenance(self)` and transitive helpers. Adding `let _waited = self.head_mixed().ok()?;` to `materialized_head_snapshot` returned `[]`, although `head_mixed` initializes and blocks. The real tree is clean. - Closure evidence: match the declaration on `strip_rust(lib)`. Forbid `maintenance(`, `head_mixed(`, `head(` and `accepted_head` in the wrapper body, or allowlist its calls. Add a negative case for each.
[P4] [stale-doc] crates/sley-protocol/src/lib.rs:1046-1055 - The `negotiate_versioned` doc comment is the code-side statement of the filter that SMP1 revision 15 §2 now makes normative, and it disagrees with that code. It says "only the known higher-version tags are filtered … when the selected version is lower", but version 3 without the bit also drops 601/602, which are v1-table reserved tags. It says "with the bit 601/602/605 dispatch while 606-607 still refuse as reserved", but `is_native_test` (:755-767) and `Hello::validate` (:876-883) have admitted and dispatched all five since N7d-2. The comment predates this round (blame: dbd1f7fb, 2026-09-16). - Closure evidence: the doc comment lists the per-selection filter as SMP1 §2 does and says all five native methods dispatch at version 3 with the bit.
[P4] [evidence-integrity] machineresearch/sley-2.0/machine-summary.json:2007,2009,2012,2109,2111,2113 - The six protocol base-field notes still say the retained PASS "predates and does not review revision 13 or 14", while `contract_revision` is 15. In the same register-chain commit (7b1e060c), the CLI and session notes were updated to "revision 9 or 10" and "revision 5 or 6". A reader could take the base PASS values from the revision 12 era as covering revision 15. - Closure evidence: the notes say "revision 13, 14 or 15" (or "any later revision"), ideally derived from `contract_revision` by the register tooling.

## Assessment

**Prior findings.** Revision 15 closes all three findings of my revision-14 transcript:
- SMP1 §2 now states the explicit negotiation's per-selection filter exactly as `negotiate_versioned` implements it. It is anchored, with revert cases, and the version 1 compatibility statement counts the handshake-identity change honestly.
- NATIVE revision 7 re-pins appendix D to SMP1 15, with a single-pin gate.
- The stale test docstring is corrected.
- The wrapper's single production caller is now gated, with negative tests.

**Code delta.** The retained-head repair is sound. The binding and the answer come from one verified revision, error precedence and budgets are unchanged, and the native-head path fails as before.

**Verification.** Every SMP1-family and native checker exits 0, as do the vector oracles, the drift checks, 65 script tests and the sley-protocol lib suite (124 passed).

**What remains.**
- **Absent boundary (P3).** SMP1's fail-closed claim for an absent maintenance boundary does not match the server, which initializes the boundary during the session-check head load. The error dates from revision 14, where I missed it. This round propagated it into a new server docstring and into S20-620's closure.
- **ENTITY_READ (P3).** Section 2 still states the old 306/307-only filter and a single v1 exception, although this round amended that contract under the SMP1 rounds.
- **Three notes (P4).** Two token-level gate bypasses, a stale `negotiate_versioned` doc comment and stale base-field scope notes.

None of these is a fail-open path or a P0/P1/P2 defect, so my lane accepts this package at contract revision 15. This verdict makes no claim about GA or release readiness.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_3_P4_PRIOR_P3_P4_CLOSED
SECTION: protocol
FIELD: vulcan_surface_review_revision_15
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P3] [spec-code] docs/spec/SMP1.md:763-766 - The sentence "so an absent boundary fails the method" is false on the server path. Before any head load, every session-checked method, `workspace.open` included, calls `head_mixed` (crates/sley-protocol/src/server.rs:2391-2392), which calls `Server::maintenance()` (:2469-2473), which calls `initialize_repository_maintenance`. That call creates a missing `locks/maintenance.lock` and syncs it (crates/sley-txn/src/maintenance.rs:40-99). The accepted-head reads that follow (crates/sley-txn/src/repository.rs:999-1011, 881-884) find the lock present, the method succeeds, and the probe sees a present boundary. Only `TransactionRepository::accepted_head` on its own refuses an absent lock (maintenance.rs:153-157). So the opener performs a durable create where the contract promises a fail-closed refusal. This round copied the claim into the new server.rs:2931-2933 docstring ("takes the boundary shared, blocking, and fails the method"). docs/spec/SLEY2_TRIAL_RUNNER_V1.md:330-332 also relies on it, citing SMP1 appendix A. The S20-620 Ariadne P4 closure (gate record :931) rests on it. My revision-14 transcript accepted this sentence without tracing `Server::maintenance()`. This is a static trace, not executed: no test in the tree removes the boundary on the server path. - Closure evidence: SMP1 :763-766, the server docstring and S20-620 §9 state that the server's head load initializes an absent boundary before its blocking shared acquisition, and that only an invalid boundary or an absent layout fails the method. Alternatively, the read path takes a non-initializing acquisition. Either way, add a server test that deletes `locks/maintenance.lock` after `session.open` and asserts the stated `workspace.open` outcome. | [P3] [cross-contract] docs/spec/ENTITY_READ_PROFILE_V2.md:43-44,59-62,78-80 - This round amended ENTITY_READ: :3-5 now says the section 2 amendment is "covered only by the SMP1 revision 14 and 15 review rounds", and :43-44 cites "SMP1 revisions 13 to 15". Section 2 still says three things SMP1 revision 15 contradicts: that the version-aware entrypoint "filters 306 and 307 from the intersection when the selected version is 1" and "does not filter other opaque unknown numeric tags"; that there is "one later exception"; and that the non-empty 201 refusal is "the recorded exception" to v1 negotiation byte stability. SMP1 revision 15 §2 (:240-251) and its history (:79-86) instead state the 605-607 drop at versions 1 and 2, and a second version 1 observable change to the handshake identity. `negotiate_versioned` names ENTITY_READ section 2 as its contract (lib.rs:1043-1044). So two normative texts disagree on the handshake preimage. - Closure evidence: ENTITY_READ section 2 states the per-selection filter (or defers to SMP1 §2 for it) and lists the version 1 native-tag drop beside the 201 exception, preferably with a check_smp1_contract anchor over ENTITY_READ and a revert case. | [P4] [checker] scripts/check_complete_root_index_snapshot_profile.py:101,103-106,156-158,208 - The gate that closes my revision-14 visibility P4 has two token-level bypasses, contrary to its stated aim that literals and comments never fake a token. (a) The `server_tests.rs` exemption searches the raw lib.rs text for `#[cfg(test)] mod server_tests;` (:156-158), not the `strip_rust` output. In memory, a lib.rs holding that text in a comment or a string literal, plus an unconditional `mod server_tests;` or `pub mod server_tests;`, returned `[]` while `server_tests.rs` called the wrapper; the unconditional declaration alone is refused. (b) `BLOCKING` (:103-106) misses the UFCS form `Self::maintenance(self)` and transitive helpers. Adding `let _waited = self.head_mixed().ok()?;` to `materialized_head_snapshot` returned `[]`, although `head_mixed` initializes and blocks. The real tree is clean. - Closure evidence: match the declaration on `strip_rust(lib)`. Forbid `maintenance(`, `head_mixed(`, `head(` and `accepted_head` in the wrapper body, or allowlist its calls. Add a negative case for each. | [P4] [stale-doc] crates/sley-protocol/src/lib.rs:1046-1055 - The `negotiate_versioned` doc comment is the code-side statement of the filter that SMP1 revision 15 §2 now makes normative, and it disagrees with that code. It says "only the known higher-version tags are filtered … when the selected version is lower", but version 3 without the bit also drops 601/602, which are v1-table reserved tags. It says "with the bit 601/602/605 dispatch while 606-607 still refuse as reserved", but `is_native_test` (:755-767) and `Hello::validate` (:876-883) have admitted and dispatched all five since N7d-2. The comment predates this round (blame: dbd1f7fb, 2026-09-16). - Closure evidence: the doc comment lists the per-selection filter as SMP1 §2 does and says all five native methods dispatch at version 3 with the bit. | [P4] [evidence-integrity] machineresearch/sley-2.0/machine-summary.json:2007,2009,2012,2109,2111,2113 - The six protocol base-field notes still say the retained PASS "predates and does not review revision 13 or 14", while `contract_revision` is 15. In the same register-chain commit (7b1e060c), the CLI and session notes were updated to "revision 9 or 10" and "revision 5 or 6". A reader could take the base PASS values from the revision 12 era as covering revision 15. - Closure evidence: the notes say "revision 13, 14 or 15" (or "any later revision"), ideally derived from `contract_revision` by the register tooling.
SUMMARY: SMP1 revision 15 and NATIVE_TEST_ADMISSION revision 7 close all three findings of my lane's revision-14 transcript (1 P3, 2 P4): the negotiation filter now matches `negotiate_versioned` exactly, with anchors and revert cases; the test docstring is fixed; and the probe wrapper's single production caller is gated. Every SMP1-family and native checker, vector oracle, drift check, script suite (65 OK) and the sley-protocol lib suite (124 passed) pass, and the retained-head code change keeps precedence and bounds. Two P3 findings remain: SMP1's "an absent boundary fails the method" is false because the server's session-check head load creates the boundary (a claim this round copied into a new server docstring and into S20-620), and ENTITY_READ section 2 still states the old 306/307-only filter. There are also three P4 notes (gate token bypasses, a stale `negotiate_versioned` doc comment, stale base-field notes); PASS.
