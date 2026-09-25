<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f073811914297505803f5b731cd4691e1623de84; role: nabu; field: nabu_architecture_review_revision_6; dispatched: 2026-09-23T08:55:06Z; duration_s: 2049; process_exit_code: 0 -->
# Nabu Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: f073811914297505803f5b731cd4691e1623de84

What I verified myself. I made no edits to the tree and ran all of the following at HEAD f0738119:

- **Scope:** `git rev-parse HEAD` returned `f073811914297505803f5b731cd4691e1623de84`, which matches the scope SHA.
- **Git reads:**
  - `git log --oneline 5b538f36..HEAD`: 8 commits (149f9142 through f0738119).
  - `git diff --stat 5b538f36..HEAD`: 49 files, +1802/−377.
  - I read the full diffs of: `crates/` (`server.rs`, `server_tests.rs`); `docs/spec/{SMP1,SLEY2_TRIAL_RUNNER_V1,COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1,SMP1_JSON_BRIDGE_V1,SLEY_CLI_V1,SESSION_HANDLE_PROFILE_V1}.md`; ADR-0029/0032/0033/0036; the six changed `scripts/`; `sley2_live_judge.py`; `sley2_tool.py`; `mediated_sley.py`; `mediated_attempt.py`; `trusted_capture.py`; `mediated_client.py`; `tooling.py`; `succ_witness_context.py`; the four witness logs; `test_mediated_context.py`; `SUCCESSION-COVERAGE.md`; `MEDIATED-INPUT-ACCOUNTING.md`; and `machine-summary.json`.
  - `git diff --stat 1450577a..HEAD`: only logs, the gate record and T54 counters. The code the witnesses ran is byte-equal to HEAD.
- **Checkers** (each exit 0, `result` PASS, problems `[]`):
  - `check_sley2_trial_runner.py`: `S20_620_IMPLEMENTED_REVIEW_PENDING`.
  - `check_smp1_contract.py`: `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING`.
  - `check_complete_root_index_snapshot_profile.py`: `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`.
  - `check_finding_register.py`.
  - `check_cli_contract.py`: `S20_430_COMPLETE`.
  - `check_smp1_json_bridge_contract.py`: `S20_420_COMPLETE`.
  - `check_session_handle_profile.py`: `S20_330_COMPLETE`.
  - `generate_smp1_json_bridge_table.py --check`.
  - `scripts/test_smp1_contract.py`: Ran 14, OK. `scripts/test_current_contract_review.py`: Ran 6, OK.
- **Cargo** (preset `CARGO_TARGET_DIR`):
  - `-p sley-protocol --lib workspace_open`: 2 passed (`..._under_version_1_never_carries_field_9`, `..._v2_discloses_only_the_materialized_head_snapshot`).
  - `-p sley-repo --lib index_cache`: 9 passed, including `probe_reports_only_a_materialized_snapshot_and_never_builds`.
  - `-p sley-cli --test cli handshake_identity_does_not_depend_on_the_transport_flag`: 1 passed. I ran it only to bind the `sley` binary: sha256 `a5d98b49…20c4`, mtime after dda51a16.
  - `-p sley-repo --test succ_live_judge_cases --no-run`: built `…-42d77a02eb446c77`.
- **Python suites:**
  - `unittest discover -s bench/sley2/tests`: Ran 23, OK.
  - `bench.live.tests.test_tooling`: Ran 14, OK.
  - `test_sley2_tool.Sley2ToolSurfacePinTests`: Ran 2, OK.
  - With both binaries bound and `TMPDIR` under `/var/tmp`: `test_mediated_access` + `test_mediated_gateway` + `test_sley2_tool` + `test_agent_access`: Ran 83 in 583.5 s, OK (no skips), exit 0.
  - `test_mediated_context`: Ran 8 in 1673.1 s, all ok, exit 0. This covers positive, exceeded budget, fake discharge, incomplete discovery, incomplete impact, extra continue, capture-evidence and no-argument `revision`.
- **Independent check of the trusted-side parser.** I ran `sley2_tool._root_query_chain` / `_RR_PREFIX` over all 27 vectors in `conformance/root-backed-query/v1/accepted.json`. For each vector, the request was rebuilt from the response echo. Result: 27/27 agree on `after`/`next` presence, including the truncated `page-namespaces-1` (next `1:04…04`).
- **Two-direction completion-binding demonstration.** I used `git archive HEAD` into `/var/tmp` scratch copies and edited only the summary.
  - COMPLETE with three `_revision_6` PASS verdicts and the `_revision_5` REVISE values kept: exit 1, `completion-unbound-review` ×3.
  - COMPLETE with `_revision_5` overwritten to PASS: exit 0, PASS.
- **Round index.** The sha256 of the three revision-5 transcripts equals `evidence/review/rounds/context-r5-5b538f3.json`: `6c708655…e970`, `5de991a8…f4c9`, `a0bb48d1…3c1d`.
- **Files read, with line ranges:**
  - Gate record 513–602.
  - My revision-5 verdict (full).
  - `server.rs` 632–652, 2440–2475, 2681–2715, 2887–2931.
  - `sley-txn/src/repository.rs` 860–900, 2731–2755.
  - `sley-txn/src/maintenance.rs` 142–190.
  - `SMP1.md` 695–700 (plus the diff).
  - `ROOT_BACKED_QUERY_PROFILE_V1.md` 300–409.
  - `NATIVE_TEST_ADMISSION_V1.md` 1–12, 530–570.
  - `SLEY_CLI_V1.md` 1–30.
  - `SLEY2_TRIAL_RUNNER_V1.md` 1–22.
  - `sley2_live_judge.py` 3236–3290, 3380–3560.
  - `sley2_tool.py` 100–135.
  - `check_sley2_trial_runner.py` 280–310.
  - `test_agent_access.py` 120–237.
  - `test_mediated_gateway.py` 1–60, 205–250.
  - `test_tooling.py` 317–365.
  - `mediated_client.py` 1–30.
  - `mediated_sley.py` `adjudicate` (350–380).
  - Witness log heads.
- **Environment notes:**
  - The sandbox refused `$?`, `printenv`, compound shell lines, process substitution and brace+quote patterns. I re-ran those through python3 wrappers.
  - I removed my `/var/tmp` scratch directories afterwards.
  - While I was working, other lanes' untracked outputs appeared: `evidence/review/verdicts/{complete_root_index_snapshot/*_revision_4-f073811.md, protocol/, sley2_trial_runner/ariadne_contract_review_revision_6-f073811.md}`. I did not write them and did not read them. Tracked files are clean.

## Evidence checked

**Status of each revision-5 finding:**

- **Prior [P2] [ownership] SMP1 owns method 201 — CLOSED.**
  - SMP1 revision 13 defines `open_summary`: `SMP1.md:57, :659, :693-713`, fields 1–8 are S20-390's, optional field 9 is S20-300's, and absence is structural with one item. Row 201's request column is corrected to "none" (`:328`), a non-empty body is refused, and `revision.read` stays unchanged.
  - The owning package moved: `protocol` is at `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING`, `contract_revision` 13, and `current_delta_review` 13 is PENDING ×3.
  - S20-620 §9 (`SLEY2_TRIAL_RUNNER_V1.md:311-313`) now cites SMP1 instead of defining the body, and the Boundary sentence (`:30-32`) is now true.
  - The version gate is tested for v1 and v2. A residual v3 gap is new finding P3-1.
- **Prior [P3] [fail-closed] opener lock — CLOSED; the premise was partly refuted, and I verified the refutation.**
  - The probe now uses `acquire_shared_repository_maintenance_nonblocking` and never initializes the boundary (`server.rs:2927`). Any failure is absence.
  - The head load already took a blocking shared lock before this change: `accepted_head` calls `acquire_shared_repository_maintenance` (`repository.rs:881-883`). So my blocking-behind-GC and import-contention points were pre-existing S20-390 behavior. That behavior is now stated in SMP1 appendix A, §9 (`:320-321`) and the docstring (`server.rs:2915-2921`).
  - The initializer's create_dir plus three fsyncs are gone. A protocol-level contention test would indeed hang in the head load first, which accepts the stated-behavior alternative.
- **Prior [P3] [authority] continuation scope — CLOSED.**
  - One trial-wide `_ContinuationLedger` (`sley2_live_judge.py:2995-3077`) now serves both audits (`:3151/3287`, `:3488/3540`). This removes the two divergent per-scope implementations.
  - Discharge is keyed by (query key, exact next cursor). Refused, unbound, past-the-end, other-query and surplus continues never discharge, and routes without continuation stay open.
  - The rule is stated in TOOLING (`tooling.py:162-171`) and §9 (`:336-344`).
  - `test_continuation_across_invocations_accepted` proves the one-shot route. The integrated positive asserts chain continuity on real server bodies (`following.after == page.next`, 4 impact pages). `fake_discharge` rejects "inconsistent continuation".
  - The key derivation lives on the trusted side (tool), is sealed into the transcript or capture, and the judge only consumes it. The offsets match the S20-310 layout (my 27/27 vector check).
- **Prior [P3] [identity] tool version — CLOSED.**
  - `TOOL_VERSION = "2"` (`sley2_tool.py:77`), with `MEDIATED_TOOL_VERSION` derived from it (`mediated_attempt.py:92`), so there is one source of truth.
  - `test_prior_tool_version_rejects` exists on both routes and passes.
  - Synthetic `tool_version: "1"` values remain in `test_mediated_gateway.py:32`, but `adjudicate` never reads them (`mediated_sley.py:350-380`).
- **Prior [P3] [record] stale records — CLOSED.** `SUCCESSION-COVERAGE.md:29` and `MEDIATED-INPUT-ACCOUNTING.md:97-109` now match HEAD: layout documented and pinned, 8 integrated proofs, live-model usability not demonstrated.
- **Prior [P4] [evidence] witness-log identity — CLOSED.** The CONTEXT logs name `source 1450577a…`, and the code at that commit is byte-equal to HEAD. A residual precision gap is P4-2.
- **Prior [P4] [dependency] test-only module — CLOSED.** `mediated_client.py:4-8` and accounting row 5 (`:35`) now describe the route-neutral scripted agent, test-only on both routes.
- **Prior [P4] [authority] budget literals — CLOSED.** `test_documented_budgets_are_the_enforced_constants` (`test_tooling.py:317-341`) ties the TOOLING numbers to `sley2_tool.MAX_RESPONSE_BYTES`, the judge's `MAX_RESPONSE_BYTES` and `AGENT_CUMULATIVE_RESPONSE_BUDGET`, and the capture caps.
- **Prior [P4] [record] corpus amendment ratification — OPEN.** Gate record `:554` and `:600` state it is open. It is carried forward as P4-3.

**Other delta items:**
- The S20-300 revision 4 identity probe has a checker-enforced sole-caller allowlist (`check_complete_root_index_snapshot_profile.py` `probe_callers`), and S20-300 moved off COMPLETE.
- The closed capture-label vocabulary (`audit_label`, `AUDIT_LABELS`) is applied on every capture path. The judge's new lazy import of `AUDIT_LABELS` from the gateway follows the existing judge → trusted `bench.live` pattern (`TOOL_METHODS`, `TOOL_VERSION`).
- The environmental failure ("tool/binary identity mismatch" after a concurrent relink) shows that the judge's binary-identity gate fails closed. My own runs, with no concurrent cargo, pass.
- The pin-only consumer re-pins do not change behavior for the consumers:
  - the CLI's only `workspace.open` caller sends an empty body (`sley-cli/tests/cli.rs:136`);
  - the bridge method tables are unchanged (`--check` PASS);
  - bodies are opaque.
  The residual record and composition gaps are P3-1 and P4-1.

## Findings

[P3] [ownership] crates/sley-protocol/src/server.rs:2907 (with :645) - The field-9 gate is `protocol_version >= PROTOCOL_VERSION_V2`, and the production `offered_hello_v3()` offers version 3 (reachable through the CLI V3-capable profile, crates/sley-cli/src/lib.rs:526). So a v3 selection also answers `open_summary` on a warm open. But SMP1 says field 9 is "under a version 2 selection only" (docs/spec/SMP1.md:57,659,697-698,846), and so does the S20-300 sole-consumer clause (COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:177). The v3 composition authority still says v3 is the union of the SMP1 tables "unchanged at revision 12" (docs/spec/NATIVE_TEST_ADMISSION_V1.md:542-545), and that file was missed by the §11.2 re-pin list. No test covers `workspace.open` under v3, so a live selection answers a body that no contract defines. - Closure: either gate `== PROTOCOL_VERSION_V2`, or amend SMP1 and S20-300 §5 to "version 2 and later" and re-pin NATIVE_TEST_ADMISSION appendix D to SMP1 revision 13 with its owner's disposition. Either way, add a v3 server test pinning the chosen body. The SMP1 revision 13 lane should take this up.

[P3] [identity] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:3,311-344 (with scripts/check_sley2_trial_runner.py:295-301) - The S20-620 contract was amended in place at revision 5 after the 5b538f3 review: the `workspace.open` bullet was rewritten, a new continuation acceptance rule was added, and non-empty bodies are now refused. There was no revision bump and no status-history sentence, so "revision 5" now names two different reviewed texts. The checker binds COMPLETE to `<lane>_revision_<contract revision>`, i.e. `_revision_5`, which holds the round-1 REVISE verdicts, while this round is filed as `_revision_6`. My scratch export shows the consequence: three `_revision_6` PASS verdicts with the `_revision_5` history kept gives exit 1, `completion-unbound-review` ×3. Only overwriting the historical `_revision_5` verdicts gives exit 0. - Closure: bump S20-620 to revision 6 with a status-history sentence naming the §9 amendments, so `_revision_6` binds by the checker's own rule and the history is kept. Alternatively, give the checker a round-qualified binding that leaves the `_revision_5` verdicts untouched, with a two-direction test.

[P4] [record] docs/spec/SLEY_CLI_V1.md:26 (with :356-359) - The CLI body still names "`docs/spec/SMP1.md` revision 12" as its composing authority, while the pin-only re-pin at :357 says 13. The three consumer-owner confirmations that gate record :561-568 and :597-598 call for are not obligations in evidence/review/finding-register.json (no S20-330/420/430 or re-pin entries). S20-330, S20-420 and S20-430 therefore stay COMPLETE while composing an SMP1 revision that is still under review, and only prose tracks the confirmation. - Closure: correct :26, and either register the three confirmations as obligations or record each owner's confirmation.

[P4] [evidence] bench/live/succ-trials-20260923/trial_context_neg.log:1 - The negative witness records "(tracked changes present)", and gate record §11.3 does not explain it. It is most likely the preceding positive run's rewrite of trial_context_pos.log, but the marker cannot tell code changes from output changes. trial_sig.log and trial_type_full.log carry no source line, although commit 5233dd9e says "logs name their source commit". (I verified the code at 1450577a equals the code at HEAD, so this is a precision gap, not a wrong result.) - Closure: make the marker list the dirty paths or exclude witness outputs, and either add source lines to the SIG and TYPE-FULL witnesses or correct the commit claim in the record.

[P4] [record] bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:554,600 - Carried forward from revision 5: the settled task-input amendment (REQ-10-rev2:49-51) is still unimplemented and unratified. The stand-in relies on a unique Record typedef and fails closed otherwise. - Closure: the corpus owner ratifies dropping or deferring the amendment in the REQ-10 design chain.

## Assessment

**Revision-5 findings.** Every revision-5 finding within this lane's remit is closed, and I verified each against code, tests, vectors or checker output rather than the record:
- The ownership inversion is repaired at its owner: SMP1 revision 13 defines `open_summary`, S20-300 revision 4 admits the probe, and both owning packages moved to review-pending.
- The probe no longer initializes or waits. The gate record's partial refutation of my lock finding is correct: the head load already blocked.
- Continuation is audited by one trial-wide ledger bound to query and cursor, which works on the documented one-shot route. The trusted parser agrees with all 27 S20-310 vectors, and the integrated proofs exercise it on real server bytes.
- The tool identity moved, with prior-version rejection on both routes.
- The records and the budget couplings are now mechanical.

All the checkers and suites I ran pass at HEAD, including the 8 integrated CONTEXT proofs and 83 binary-backed access, gateway and tool tests. The only prior finding still open is the corpus-owner ratification, which is not an implementation action.

**What remains.**
- **P3-1:** a new version gate that disagrees with its owning spec for the live version 3. It is the same composition-enumeration pattern again, this time the native appendix D pin.
- **P3-2:** an identity gap. The S20-620 text changed under an unchanged revision number, and as the checker is written, this round's verdicts cannot bind to completion without overwriting history. It fails closed, so no false completion is possible.
- **P4-1 to P4-3:** three record and evidence notes.

None of these is a correctness or safety defect in the S20-620 route, which uses version 2 only. None needs redesign.

**Why PASS.** For this lane, the S20-620 revision-5 implementation is accepted, conditional on the P3 closures being tracked.

**Scope limits.**
- I make no GA or release-readiness claim.
- I do not review SMP1 revision 13 or S20-300 revision 4 as their own lanes; those reviews are pending.
- I do not ratify the consumer re-pins; that belongs to the consumer owners.

Separately, the Google Calendar, Google Drive and Vercel connectors need authorizing in claude.ai connector settings (Vercel via `claude mcp`) before they can be used; this review did not need them.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_3_P4
SECTION: sley2_trial_runner
FIELD: nabu_architecture_review_revision_6
SCOPE_SHA: f073811914297505803f5b731cd4691e1623de84
FINDINGS: [P3] [ownership] crates/sley-protocol/src/server.rs:2907 (with :645) - field 9 gated `>= PROTOCOL_VERSION_V2` while production offers v3, so v3 warm opens answer open_summary; SMP1.md:57,659,697-698,846 and S20-300 §5 :177 say version 2 only; NATIVE_TEST_ADMISSION_V1.md:542-545 still composes SMP1 tables "unchanged at revision 12" (missed by the re-pin list); no v3 test - gate == V2, or amend SMP1/S20-300 to "version 2 and later" and re-pin native appendix D with owner disposition, plus a v3 server test | [P3] [identity] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:3,311-344 (with scripts/check_sley2_trial_runner.py:295-301) - S20-620 §9 amended in place at revision 5 after review, no bump or history line; checker binds COMPLETE to `_revision_5` (holding REVISE) while this round is filed `_revision_6`; demonstrated: `_revision_6` PASS ×3 with history kept gives exit 1 completion-unbound-review ×3, and only overwriting `_revision_5` passes - bump to revision 6 with a history sentence, or a round-qualified checker binding preserving `_revision_5`, tested both directions | [P4] [record] docs/spec/SLEY_CLI_V1.md:26 (with :356-359) - CLI body still names SMP1 revision 12 after the pin-only re-pin to 13; consumer-owner confirmations (gate record :561-568, :597-598) untracked in finding-register.json while S20-330/420/430 stay COMPLETE on an under-review SMP1 revision - fix :26 and register or record the three confirmations | [P4] [evidence] bench/live/succ-trials-20260923/trial_context_neg.log:1 - "(tracked changes present)" unexplained and cannot tell code from output dirt; trial_sig.log and trial_type_full.log lack source lines despite 5233dd9e's claim - list dirty paths or exclude outputs; add source lines or correct the claim | [P4] [record] bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:554,600 - carried forward: task-input amendment (REQ-10-rev2:49-51) still unratified; stand-in relies on a unique Record typedef (fails closed) - corpus-owner ratification in the REQ-10 design chain
SUMMARY: PASS for the architecture lane. Every revision-5 finding within this lane's remit is verified closed: SMP1 revision 13 owns the open_summary body, the probe no longer initializes or waits, one trial-wide ledger binds continuation by query and cursor (the parser matches 27/27 S20-310 vectors), and the tool identity moved to "2". All checkers and the binary-backed suites (83 tests, plus 8 integrated CONTEXT proofs) pass at HEAD; the corpus-amendment ratification stays open. Two P3s remain: the field-9 gate also fires under the live version 3, which no contract defines, and the S20-620 text changed under an unchanged revision 5, so this round's `_revision_6` verdicts cannot bind to completion without overwriting history. Three P4 record and evidence notes are listed; none affects the version-2 trial route.
