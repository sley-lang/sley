<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f073811914297505803f5b731cd4691e1623de84; role: vulcan; field: vulcan_surface_review_revision_6; dispatched: 2026-09-23T09:10:20Z; duration_s: 1760; process_exit_code: 0 -->
# Vulcan Council review — sley2_trial_runner
Harness: claude-code
Reviewed checkpoint: f073811914297505803f5b731cd4691e1623de84

I checked the following myself. None of it rests on what the records say.

- **Scope.** `git rev-parse HEAD` returned `f073811914297505803f5b731cd4691e1623de84`, which matches the scope.
  - `git log --oneline 5b538f36..HEAD`: 9 commits (149f9142 … f0738119).
  - `git diff --stat 5b538f36..HEAD`: 49 files, +1802/−377.
  - `git show --stat` on f0738119, 1450577a and 5233dd9e: T54 counters only, and witness logs only.
- **Diffs read in full**, for `5b538f36..HEAD`:
  - Python: the judge, `sley2_tool.py`, `mediated_sley.py`, `trusted_capture.py`, `mediated_attempt.py`, `mediated_client.py`, `tooling.py`, `succ_witness_context.py`.
  - Tests: `test_mediated_access.py`, `test_agent_access.py`, `test_mediated_context.py`, `test_tooling.py`.
  - Rust: `server.rs`.
  - Specs and ADRs: `SMP1.md`, `SLEY2_TRIAL_RUNNER_V1.md`, ADR-0036, ADR-0033, `SMP1_JSON_BRIDGE_V1.md`, `SLEY_CLI_V1.md`, `SESSION_HANDLE_PROFILE_V1.md`.
  - Scripts: the four re-pinned checkers, `test_smp1_contract.py`.
  - Records: `SUCCESSION-COVERAGE.md`, `MEDIATED-INPUT-ACCOUNTING.md`, the CONTEXT/SIG/TYPE-FULL witness logs.
- **Files read, with line ranges.**
  - Gate record `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:513-602` (§11).
  - `bench/fixtures/sley2_live_judge.py:2960-3559`.
  - `bench/live/mediated_sley.py:150-340`.
  - `bench/live/trusted_capture.py:380-470`.
  - `bench/live/mediated_attempt.py:349-440`.
  - `bench/live/sley2_tool.py:380-460,830-850,1080-1180,1246-1317`.
  - `bench/live/succ_witness_context.py:50-75`.
  - `crates/sley-query/src/root_query.rs:690-900,1589-1617`, plus the wire constants at `:41-45` and `query.rs:611-620`.
  - `crates/sley-protocol/src/server.rs:2443-2460,2681-2714,2887-2932`.
  - `crates/sley-protocol/src/server_tests.rs:605-640,7385-7470`.
  - `crates/sley-txn/src/maintenance.rs:107-193`, `crates/sley-txn/src/repository.rs:872-885`, `crates/sley-repo/src/index_cache.rs:256-290`.
- **Checkers, all exit 0.**

  | Checker | Result |
  |---|---|
  | `check_sley2_trial_runner.py` | PASS, revision 5, `S20_620_IMPLEMENTED_REVIEW_PENDING`, problems [] |
  | `check_smp1_contract.py` | PASS rev 13 |
  | `check_complete_root_index_snapshot_profile.py` | PASS rev 4 |
  | `check_finding_register.py` | PASS |
  | `check_cli_contract.py` | PASS rev 8 |
  | `check_smp1_json_bridge_contract.py` | PASS rev 10 |
  | `check_session_handle_profile.py` | PASS rev 4 |
  | `generate_smp1_json_bridge_table.py --check` | PASS |
  | `test_smp1_contract.py` | Ran 14, OK |

- **Rust tests.**
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: exit 0, 2 passed.
  - `cargo test --locked --offline -p sley-repo --test succ_live_judge_cases --no-run` and `-p sley-cli --test cli --no-run`: both "Finished" with no rebuild. So the `sley` binary and the judge driver under `CARGO_TARGET_DIR` are current at HEAD.
- **Python suites.**
  - `python3 -m unittest bench.live.tests.test_tooling`: Ran 14, OK.
  - `python3 -m unittest discover -s bench/sley2/tests -t .`: Ran 23, OK.
  - `test_mediated_access` + `test_agent_access` (binary bound): exit 0, Ran 48, OK.
  - `test_mediated_gateway` + `test_sley2_tool` (binary bound): exit 0, Ran 35, OK.
  - **Integrated CONTEXT proofs** `bench.live.tests.test_mediated_context` (both binaries bound, bwrap): exit 0, Ran 8, OK in one run (1086 s). This includes `test_context_fake_discharge_rejects`. The binary-gated suites did not run in the revision-5 review; this time they did.
- **Pre-repair reproduction.** I ran the same 48 audit tests against the `2563d577^` judge, loaded in memory from `git show`: 10 FAIL and 2 ERROR.
  - Mediated audit: 9 FAIL + 1 ERROR. That is the "10" the record claims.
  - Direct audit: 1 FAIL + 1 ERROR.
- **In-memory probes.** I sent 13 adversarial page sequences directly into `_ContinuationLedger` (results under Evidence item 1).

## Evidence checked

**Status of each revision-5 finding:**

Prior finding [P2] [judge-continuation] sley2_live_judge.py (unconditional continuation discharge): **CLOSED**

Prior finding [P3] [capture-labels] mediated_sley.py (agent-supplied capture labels): **CLOSED**

Prior finding [P3] [surface-contract] tooling.py (per-invocation scope vs staged transport): **CLOSED**

Prior finding [P3] [budget-contract] tooling.py (per-reply bound units, 8 MiB vs 4 MiB): **CLOSED**

Prior finding [P4] [strict-input] server.rs (workspace.open ignores a body): **CLOSED**

Prior finding [P4] [resource-claims] server.rs (probe lock/init/cost claims): **CLOSED**

1. **P2 closure: continuation is now bound to the page it continues. Verified.**
   - **Where the binding comes from.**
     - `_root_query_chain` (`sley2_tool.py:228-253`) derives it from the exact request and response bodies. `_RQ_PREFIX`/`_RR_PREFIX` and the cursor sizes match the Rust encoders (`root_query.rs:744-765,849-866,1589-1617`; limits = 36 bytes, cursor tags 1/2/3 = 32/68/32).
     - The server answers only a canonical body: `server.rs:2698` requires `preimage() == body`. So on a successful page, the parsed `query`/`after` are exactly what the server executed.
     - The binding is attached only to non-failed root-query responses (`sley2_tool.py:398`). The gateway carries it only for a single-call `raw` frame (`mediated_sley.py:232`). The capture seals it inside the hash-chained response record (`trusted_capture.py:418-423,455`).
   - **What the ledger does** (`sley2_live_judge.py:2995-3076`), in both audits and trial-wide:
     - Failed pages are ignored.
     - A continue must match `(query, after)` against an open `(query, next)` count.
     - Truncation mismatch between the flag and the binding rejects.
     - Cursor-bearing `query.root`, `query.restricted` and `refs.list` truncation can never be discharged.
     - `finish()` rejects anything still open.
   - **My probes, all fail-closed:**
     - truncated continue without next → "continuation binding"
     - the same root twice with only one chain → reject
     - the same root twice with two chains → accept
     - `open` with omit>0 and failed → hidden truncation
     - restricted truncation → reject
     - unparsed root chain → reject
     - untruncated root with omit>0 → reject
     - cursor-bearing root → reject
     - flag mismatch → reject
     - duplicate continue after close → inconsistent
     - continue after a refused root → inconsistent
     - junk-typed chain → reject
     - chain ending on a cursor-bearing root → reject
   - **Regressions exist and discriminate:**
     - refused, forged-label and `ff…ff` cases in the mediated audit;
     - refused and `ff…ff` cases in the direct audit (the direct route has no command labels);
     - the integrated `fake_discharge` mode.
   - **Detail count is honest now.** The accepted detail's `continuations=` counts only discharging continues.
   - **One limit of the integrated mode.** `fake_discharge` rejects on its final past-the-end continue. On its own it would not tell whether the earlier refused or forged frames discharged. The unit cases do make that distinction, and I reproduced them failing on the pre-repair judge.
   - **Trust model on the direct route is unchanged.** Its transcript is an unkeyed hash chain (`sley2_tool.py:1246-1275`), so `chain` there is exactly as trustworthy as the direct route's other fields.
2. **P3 capture labels: closed.**
   - `audit_label` (`mediated_sley.py:68-85`) is used on every production capture path: `mediated_sley.py:259,335,459` and `mediated_attempt.py:416`.
   - The judge rejects any label outside `AUDIT_LABELS` (`sley2_live_judge.py:3500-3505`).
   - `test_denied_commands_record_under_the_fixed_label` shows `raw:query.continue` and `commit` recorded as `denied`.
   - Harmless residue, not a finding: `AUDIT_LABELS` also admits a bare `raw`, which `audit_label` never emits and which is non-bounded.
3. **P3 surface contract: closed by removing the scope dependency.**
   - Binding is by query and cursor across the whole trial. TOOLING (`tooling.py:162-171`) says so, and the text is pinned in `test_documented_layout_is_pinned`.
   - `test_cross_scope_bound_continuation_passes` / `test_cross_scope_wrong_cursor_rejects` pin the cross-session behaviour; `test_continuation_across_invocations_accepted` pins the cross-invocation case on the direct route.
   - `session_id` now only has to be non-empty, so the undocumented staged transport no longer affects evidence.
4. **P3 budget units: closed.**
   - TOOLING (`tooling.py:172-177`) states the bound in reply bytes, the hex doubling, the safe value 524000 and the 8388608 capture backstop.
   - The server refuses a response over `max_response_bytes` (`root_query.rs:1562`).
   - `test_a_maximal_documented_page_fits_the_per_reply_bound` builds the exact raw report shape (`sley2_tool.py:839-841`) and the printed envelope (`:1292-1295`).
   - The stand-in now requests 524000.
5. **P4 strict input: closed.**
   - `server.rs:2903-2905` refuses a non-empty body before the head load.
   - This is tested under version 1 (`server_tests.rs:638`) and version 2 (`:7463-7469`).
   - Every in-repo caller sends an empty body (`sley2_tool.py:338,830`; `crates/sley-cli/tests/cli.rs:136`).
6. **P4 resource claims: closed, and the partial refutation holds.**
   - The probe now uses `acquire_shared_repository_maintenance_nonblocking` (`maintenance.rs:142-146`). It requires the boundary to exist already (`:153-157`), so it never initializes it.
   - The refutation is correct: `head()` (`server.rs:2453-2459`) → `accepted_head()` (`repository.rs:881-884`) already takes a blocking shared lock. Blocking behind an exclusive owner therefore predates this change, and §9 now states it.
7. **Pin-only re-pins.**
   - The consumer contracts carry SMP1 bodies as opaque bytes, and all re-pinned checkers pass.
   - From this lane I see no surface change: the only behavioural delta is the empty-body refusal, and no caller sends a body. Owner confirmation remains the owners' job, as the record says.
8. **Environmental run failure.** I ran all 8 integrated proofs in one pass against unchanged, current binaries, and all passed. That is consistent with the record's explanation (a concurrent relink).
9. **Records.**
   - My revision-5 transcript is filed byte-exact: sha256 `a0bb48d1…c1d` matches `rounds/context-r5-5b538f3.json`.
   - `machine-summary.json` records it as `REVISE_0_P0_0_P1_1_P2_3_P3_2_P4`.

## Findings

[P4] [doc-drift] bench/fixtures/sley2_live_judge.py:3094-3099,3107-3113,3353-3357 (and bench/live/SUCCESSION-COVERAGE.md:520-533) - Both audit docstrings still describe the replaced rule: continuation "in the same session scope (the capture's own session_id…)", and "Limitation retained: transcripts record … not … continuation tokens, so the judge verifies … not … after==prev-next_after token equality". The ledger now binds by query and cursor across the whole trial and verifies exactly that equality. The coverage paragraph repeats the old limitation. The text understates what is verified and describes a scope rule that no longer applies - closure evidence needed: rewrite both docstrings (and update the coverage paragraph or mark it historical) to describe `_ContinuationLedger`.

[P4] [evidence-provenance] bench/live/succ-trials-20260923/trial_context_neg.log:1 (with bench/live/succ_witness_context.py:63-71; gate record line 583) - The neg witness log records "source 1450577a… (tracked changes present)", but §11.3 reports the witnesses "at 1450577a" and does not mention the flag. The dirty check runs before each witness and counts every tracked file, and the witness writes its log only at the end. The likely cause is therefore the pos run's own rewrite of the tracked `trial_context_pos.log`, which is benign. But the flag cannot tell evidence output apart from code drift - closure evidence needed: exclude the witness log outputs from the check (or record the dirty paths), then re-run or disclose the flag in the record.

[P4] [spec-precision] docs/spec/SMP1.md:63-64,316-318 - "revision 13 still serves version 1 exactly as before, byte for byte, including `workspace.open`" and "no version 1 byte changes" overstate the delta. `server.rs:2903-2905` now refuses a non-empty body under every selection, and the version-1 test pins this at `server_tests.rs:638`, so version-1 behaviour did change for that out-of-contract input. Row 659 and line 61 do state the refusal, and appendix A always named an empty body, so there is no compatibility risk - closure evidence needed: qualify both sentences to in-contract (empty-body) requests.

## Assessment

All six revision-5 findings are closed, and I checked each against code, the Rust wire encoders, tests I ran, and pre-repair reproductions:
- **The P2 is closed.** Continuation discharge is now tied to the exact server-accepted request and response bytes, keyed by query and cursor across the whole trial. Refused, forged-label, past-the-end, skipping, foreign-query and surplus continues cannot close a page.
- **The incomplete-discovery negative class holds again.** I ran the 8 integrated proofs, including the new fake-discharge mode: all 8 pass, which the previous round could not reproduce.
- **The surface changes are fail-closed.** Every edge I probed rejects rather than accepts. These are the fixed capture labels, the documented budget units, the empty-body refusal and the non-waiting probe.

The three remaining findings are P4 documentation and provenance precision; none affects what a hostile input can slip past.

Outside this lane and not blocking it: the SMP1 revision 13 and S20-300 revision 4 reviews, the consumer-owner confirmation of the re-pins, the fuzz refresh, corpus ratification, and a live-model trial. No GA, release-readiness or model-trial claim is made or implied.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_3_P4_PRIOR_P3_P4_CLOSED
SECTION: sley2_trial_runner
FIELD: vulcan_surface_review_revision_6
SCOPE_SHA: f073811914297505803f5b731cd4691e1623de84
FINDINGS: [P4] [doc-drift] bench/fixtures/sley2_live_judge.py:3094-3099,3107-3113,3353-3357 (and bench/live/SUCCESSION-COVERAGE.md:520-533) - audit docstrings still state same-session-scope continuation and "token equality unverified", while the ledger binds by query and cursor across the whole trial and verifies after == next - closure evidence needed: rewrite the docstrings and update or mark historical the coverage paragraph; [P4] [evidence-provenance] bench/live/succ-trials-20260923/trial_context_neg.log:1 (with succ_witness_context.py:63-71; gate record line 583) - neg witness log says "(tracked changes present)" and §11.3 does not disclose it; the dirty check counts the tracked witness logs themselves, so the flag cannot tell evidence output from code drift - closure evidence needed: exclude log outputs or record dirty paths, re-run or disclose; [P4] [spec-precision] docs/spec/SMP1.md:63-64,316-318 - "serves version 1 exactly as before, byte for byte, including workspace.open" / "no version 1 byte changes" overstate: a non-empty body is now refused under version 1 too (server.rs:2903-2905, server_tests.rs:638) - closure evidence needed: qualify to in-contract (empty-body) requests
SUMMARY: All six revision-5 findings are closed. Continuation discharge is now bound to the exact server-accepted page by query and cursor across the whole trial, capture labels are a closed set, and TOOLING states the budget units. `workspace.open` refuses a body and its probe no longer waits or initializes the lock boundary. I verified this by code reading against the Rust encoders, 48 audit tests (12 of which break on the pre-repair judge), 13 ledger probes, and all 8 integrated CONTEXT proofs run at HEAD. Three P4 precision notes remain (stale judge docstrings, an undisclosed witness dirty flag, and an SMP1 version-1 byte-compatibility overstatement), so this lane accepts the S20-620 revision-5 implementation.
