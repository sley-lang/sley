<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: nabu; field: nabu_architecture_review_revision_7; dispatched: 2026-09-23T13:59:40Z; duration_s: 572; process_exit_code: 0 -->
# Nabu Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

I am an independent reviewer, not the author. I made no edits to the reviewed tree: nothing was created, formatted or committed there. My only write was to my own reviewer memory, which is outside the tree. At the start of the session, twelve untracked `evidence/review/verdicts/**-26d050e.md` files from other lanes were already present. I did not write them and did not read them. Apart from those files, the tracked tree is clean.

**Scope and dispatch note**
- `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, which matches the scope. I checked it again at the end of the session and it was unchanged.
- The dispatch names my lane's prior transcript as `evidence/review/verdicts/sley2_trial_runner/nabu_architecture_review_revision_6-2b0f1c9.md`. That file does not exist.
  - The round-7 index `evidence/review/rounds/context-r7-2b0f1c9.json` has no Nabu entry for S20-620. For this section it lists only `ariadne_contract_review_revision_6` and `vulcan_surface_review_revision_7`.
  - My lane's latest transcript is `nabu_architecture_review_revision_6-f073811.md`, verdict `PASS_0_P0_0_P1_0_P2_2_P3_3_P4`. I give a CLOSED/OPEN status for each of its five findings.
  - The round-7 repairs for those findings (gate record §12.3) had not been reviewed by this lane before. This review is their first verification.

**Git commands**
- `git log --oneline 2b0f1c9f..HEAD`: 12 commits, from `531805ae` to `26d050e6`.
- `git diff --stat 2b0f1c9f..HEAD`: 86 files, +4996/−537.
- I read the full diffs, restricted to this package, of:
  - `docs/spec/SLEY2_TRIAL_RUNNER_V1.md`, `scripts/check_sley2_trial_runner.py`, `scripts/test_sley2_trial_runner.py` (new)
  - ADR-0036, the S20-620 closeout, the WORK_PACKAGES row, `Makefile`
  - `bench/live/{sley2_tool.py, scratch.py, prove_merge_production.py, SUCCESSION-COVERAGE.md, succ_witness_context.py, succ_witness_create.py}` and `bench/live/tests/test_scratch_cleanup.py`
- The `sley2_trial_runner` machine-summary section, compared field by field against `git show 2b0f1c9f:…/machine-summary.json`.
- `git diff --stat 2b0f1c9f..HEAD -- bench/sley2/ bench/fixtures/sley2_live_judge.py bench/live/mediated_sley.py bench/live/mediated_attempt.py` is empty. The runner, judge and mediated route are unchanged, which matches "no runner behavior changes".
- `git show 2b0f1c9f:docs/spec/SLEY2_TRIAL_RUNNER_V1.md` line 3 reads "revision 6". At 2b0f1c9f the summary has `contract_revision` 6 and no `_revision_7` field. `git log -S vulcan_surface_review_revision_7 -- machine-summary.json` shows the field was added by `1d8086fc`.

**Checkers** (each run through a python3 subprocess wrapper, because the sandbox refused `$?`)
- `scripts/check_sley2_trial_runner.py`: exit 0, `"result": "PASS"`, revision 7, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, problems `[]`.
- `scripts/test_sley2_trial_runner.py -v`: exit 0, Ran 4, OK.
- `check_smp1_contract.py`: exit 0, PASS, revision 15.
- `check_complete_root_index_snapshot_profile.py`: exit 0, PASS, revision 6.
- `check_cli_contract.py`: exit 0, PASS, revision 10.
- `check_finding_register.py`: exit 0, PASS.

**Tests**
- `python3 -m unittest discover -s bench/sley2/tests -t .`: Ran 23, OK.
- `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 5 passed, no rebuild (0.03 s). This includes `workspace_open_v3_answers_the_version_2_open_summary` and `workspace_open_answers_from_the_single_checked_head_load`.
- `cargo test --locked --offline -p sley-cli --test cli handshake_identity_does_not_depend_on_the_transport_flag`: 1 passed, no rebuild, so `$CARGO_TARGET_DIR/debug/sley` is built from HEAD.
- `bench.live.tests.test_scratch_cleanup` with `SLEY2_SLEY_BINARY` bound: Ran 11, OK, 0 skipped.
- Base-code comparison: I loaded the 2b0f1c9f versions of `scratch.py`, `sley2_tool.py` and `prove_merge_production.py` via `git show` + exec into `sys.modules`, then ran the four new tests. All four fail on the base code:
  - stalled serve: "serve child reaped" fails, because the child is left running;
  - merge stages: leftovers are non-empty;
  - hard link: 384 != 256;
  - parent: `ScratchRemovalError` not raised.
- I checked afterwards that no `sley2-*`, `judge-stall-test-*` or `merge-stage-test-*` directory remained. The orphaned `sleep` left by the base-code run exited by itself.

**In-memory completion-binding probe** (the checker's `read` patched for the summary path only)
- **A.** Status set to COMPLETE, no new fields: exit 1. Problems: `completion-unbound-review:ariadne_contract_review` and `completion-unbound-review:nabu_architecture_review`. Vulcan is not named.
- **B.** Status set to COMPLETE, plus `ariadne_contract_review_revision_7` and `nabu_architecture_review_revision_7` PASS: exit 0, problems `[]`.

**Files read, with line ranges**
- My f073811 transcript, 1–150 (full); Vulcan r7 transcript, 1–120 (full).
- Gate record, 600–969.
- `check_sley2_trial_runner.py`, 1–348 (full).
- `check_cli_contract.py`, 140–172.
- `build_finding_register.py`: 1–40, 430–469, 1070–1129.
- `SLEY2_TRIAL_RUNNER_V1.md`: 1–50 and 300–369.
- `SMP1.md`: 756–769 (plus lines 63, 703 and 744 via grep).
- `SLEY_CLI_V1.md`: 20–32 (plus pin lines via grep).
- `NATIVE_TEST_ADMISSION_V1.md`: 9–13 and 555–557 via grep.
- `COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`: 14 and 208–209 via grep.
- `server.rs`: 2455–2484 and 2900–2984.
- `sley-txn/src/maintenance.rs`: 100–174. `repository.rs` lines 829, 883 and 920 via grep.
- `bench/sley2/runner.py`: 340–469.
- `bench/live/sley2_tool.py`: 266–345.
- `prove_merge_production.py`: 120–214.
- The first three lines of the four `succ-trials-20260923/*.log` files.

## Evidence checked

**Status of each finding in this lane's latest transcript (f073811)**

- **Prior [P3] [ownership] field-9 gate fires under version 3 — CLOSED.**
  - The specs now follow the code:
    - SMP1 `:63`, `:703` and `:744`: "version 2 and every later selection whose table carries row 201 (version 3)";
    - S20-300 `:208-209`;
    - NATIVE_TEST_ADMISSION `:555-557`, which re-pins SMP1 revision 15.
  - The server docstring (`server.rs:2916-2933`) matches, and the gate at `:2946` is unchanged.
  - A v3 test exists and passes (`workspace_open_v3_answers_the_version_2_open_summary`).
- **Prior [P3] [identity] S20-620 amended in place under revision 5 — CLOSED as specified.**
  - The contract took revision 6, with a history sentence (`SLEY2_TRIAL_RUNNER_V1.md:15-20`).
  - The `_revision_5` REVISE ×3 values are kept byte-exact.
  - All three `_revision_6` fields are PASS and bind by the checker's own rule. The scenario I demonstrated in round 6 is the HEAD state for revision 6.
  - The same identity defect has come back at revision 7 in the Vulcan lane. That is new finding P2-1 below.
- **Prior [P4] [record] CLI :26 stale; consumer confirmations untracked — CLOSED.**
  - `SLEY_CLI_V1.md:20-25` and `:36` pin SMP1 revision 15 through CLI revision 10.
  - `cli`, `json_bridge` and `session_handle_profile` are at `*_IMPLEMENTED_REVIEW_PENDING`, each with `current_delta_review` `{contract_revision 10/12/6, PENDING ×3}`.
  - Their checkers enforce these records (`check_cli_contract.py:167-171`), so no consumer is COMPLETE on an SMP1 revision that is still under review.
- **Prior [P4] [evidence] witness dirty marker; SIG/TYPE-FULL lack source lines — CLOSED.** All four logs open with `source c21ff9457ea7… (clean apart from witness outputs)`, including `trial_sig.log:1` and `trial_type_full.log:1`.
- **Prior [P4] [record] corpus task-input amendment ratification — OPEN.** The gate record still lists it as open (`:938`, `:960`). It is carried forward as P4-2.

**Revision-7 delta (package paths)**
- **Composed pins.**
  - `composed_pin_problems` (`check_sley2_trial_runner.py:146-168`) reads the SMP1 and S20-300 Status lines. The 4 revert tests pass.
  - I enumerated the spec's revision pins: only `:42-43` and `:323-324` (SMP1 15) and `:327` (S20-300 6) exist, and all three are anchored.
  - An authority moving turns the contract red, so the check fails closed. The coupling direction is right: the consumer pins its composing owner, and the owner never reads the consumer.
- **Absent-boundary sentence (`:330-332`).** The code bears it out:
  - `acquire_repository_maintenance` requires the lock directory and file (`maintenance.rs:153-161`).
  - The head load takes that lock shared and blocking (`repository.rs:883`).
  - `workspace_open` answers from the head the session check loaded, or else from `self.head()` (`server.rs:2942-2945`), before the non-blocking probe (`:2967`).
- **Round-7 Vulcan bench closures.** Each of the four new tests passes at HEAD and fails on the 2b0f1c9f code, as the gate record §13.3 claims. The merge prover registers each stage at `mkdtemp` (`prove_merge_production.py:129-130`), attempts every tree, and runs under `scratch_root`.
- **Completion binding at revision 7.** The dispatch asks how the implementation keeps the existing `vulcan_surface_review_revision_7` field from satisfying the revision-7 binding. It does not keep it out; see P2-1.
  - The only safeguard is prose: the field's `_note` ("reviews the contract revision 6 text") and the `status_note` ("completion needs PASS _revision_7 fields"). The checker reads neither.

## Findings

[P2] [evidence-binding] scripts/check_sley2_trial_runner.py:324-331 (with machineresearch/sley-2.0/machine-summary.json sley2_trial_runner.vulcan_surface_review_revision_7) - The COMPLETE gate binds a lane verdict by its field name alone: `<lane>_revision_<spec_revision>` must start with PASS. The existing `vulcan_surface_review_revision_7` = `PASS_0_P0_0_P1_0_P2_2_P3_3_P4` is a verdict on 2b0f1c9f, where the contract Status read revision 6 (`git show 2b0f1c9f:docs/spec/SLEY2_TRIAL_RUNNER_V1.md:3`), and its own note says it reviews the revision 6 text. So revision 7 starts with Vulcan's binding already satisfied. In-memory probe: status COMPLETE with only the Ariadne and Nabu `_revision_7` PASS fields added gives exit 0 and PASS; with neither added, only Ariadne and Nabu are reported unbound. As a result, the Vulcan lane never has to review the revision 7 text or `236b7640` (the fixes to its own P3s). A new Vulcan revision-7 verdict could bind only by overwriting the historical field. Nothing prevents this: no spec sentence says so (compare revision 6's "bind nothing here"), no checker rule enforces it, and `test_sley2_trial_runner.py` has no binding test. The sibling mechanism (`check_cli_contract.py:140-171`: "Historical review fields … never satisfy the current delta") is not used here. This is the same identity defect as my round-6 P3, and now it fails open. - Closure evidence: bind completion to the revision the verdict actually reviewed, not to the field name. Either adopt `current_delta_review` `{contract_revision: 7, …PENDING}` gated as in the CLI, bridge and session checkers, or have the checker resolve each `<lane>_revision_<N>` through its round-index `scope_sha` and require the contract Status at that commit to read revision N. Keep the historical Vulcan field byte-exact. Add a two-direction test: the existing Vulcan field is refused for revision 7, and a properly bound revision-7 PASS is admitted.

[P4] [ownership] bench/live/sley2_tool.py:269-286,306-314 (with bench/sley2/runner.py:346-422) - The kill-and-reap abort that closes Vulcan's r7 P3 is written in a consumer, and it writes into the runner-owned `Endpoint`'s private state (`getattr(endpoint, "_process", None)`, `endpoint._closed = True`). `Endpoint` owns the process lifecycle through `close()`, and this is the only place under `bench/` that reaches into those attributes from outside. If runner.py renames the attribute, `_abandon` silently does nothing, so the reap guarantee fails open. The only regression test, `StalledServeIsReaped`, is skipped unless `SLEY2_SLEY_BINARY` is bound, and it sits in `bench/live/tests`, which no Makefile target runs. - Closure evidence: move the logic into an owner-side `Endpoint.abort()` (kill, reap, mark closed) called from `Session.__init__`, with an offline test in `bench/sley2/tests`, which the stage checker runs. At minimum, make `_abandon` fail loudly when the attribute is missing.

[P4] [record] bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:938,960 - Carried forward from revisions 5 and 6: the settled task-input amendment (REQ-10-rev2:49-51) is still neither implemented nor ratified. The stand-in relies on there being a single Record typedef and fails closed otherwise. - Closure evidence: the corpus owner ratifies dropping or deferring the amendment in the REQ-10 design chain.

## Assessment

**Prior findings.** Four of the five findings from my f073811 transcript are closed, each checked against code, tests, specs or checker output:
- the v3 field-9 composition;
- the revision 6 bump;
- the consumer re-pins and their tracking;
- witness identity.

The corpus ratification stays open because only its owner can act on it.

**What else I verified.** The revision-7 contract delta is sound. The composed pins are now mechanical and fail closed. The absent-boundary sentence matches the code. The runner, judge and mediated route are unchanged. Vulcan's four bench repairs each have a test that fails on the old code and passes at HEAD.

**Why REVISE.** The binding this package relies on for completion does not bind. The existing `vulcan_surface_review_revision_7`, a verdict on the revision 6 text, already satisfies the revision-7 COMPLETE gate. So the package can reach COMPLETE without a Vulcan review of revision 7, including a review of the repairs to Vulcan's own findings. The only other route is overwriting a historical verdict.
- Evidence binding and identity discipline are this lane's remit.
- The coordinator named this exact hazard.
- The same class of defect was closed once already (S20-300, and S20-620 at revision 6).

So I rate it P2 rather than P3. No false completion exists today, because the status is `S20_620_IMPLEMENTED_REVIEW_PENDING`. The fix is local to the checker and summary and needs no redesign. The P4 ownership note does not affect any judgment.

I make no GA or release-readiness claim. I do not review SMP1 15, S20-300 6, CLI 10, bridge 12 or session 6 as packages of their own.

The Google Calendar and Google Drive connectors need authorizing in claude.ai connector settings, and Vercel via `claude mcp`, before they can be used. This review did not need them.

VERDICT: REVISE_0_P0_0_P1_1_P2_0_P3_2_P4
SECTION: sley2_trial_runner
FIELD: nabu_architecture_review_revision_7
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P2] [evidence-binding] scripts/check_sley2_trial_runner.py:324-331 (with machine-summary.json sley2_trial_runner.vulcan_surface_review_revision_7) - COMPLETE binds by field name only; the existing vulcan _revision_7 PASS reviewed contract revision 6 at 2b0f1c9f, so Vulcan's revision-7 binding is already satisfied (probe: COMPLETE plus only Ariadne/Nabu _revision_7 PASS gives exit 0); a fresh Vulcan r7 verdict could bind only by overwriting history; no spec, checker or test guard, unlike current_delta_review (check_cli_contract.py:140-171) - bind completion to the reviewed revision (current_delta_review, or round-index scope_sha whose contract Status is N), keep the historical field, and add a two-direction test | [P4] [ownership] bench/live/sley2_tool.py:269-286,306-314 (with bench/sley2/runner.py:346-422) - _abandon writes into runner-owned Endpoint._process/_closed; a rename silently disables the reap; the only test is binary-gated in bench/live/tests, which no Makefile target runs - owner-side Endpoint.abort() with an offline bench/sley2 test, or fail loudly on a missing attribute | [P4] [record] bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:938,960 - carried: task-input amendment (REQ-10-rev2:49-51) still unratified; stand-in fails closed - corpus-owner ratification in the REQ-10 design chain
SUMMARY: Four of this lane's five f073811 findings are verified closed (the v3 field-9 composition, the revision 6 bump, consumer re-pin tracking, witness identity); the corpus ratification stays open. The revision-7 delta itself is sound: composed pins are mechanical and fail closed, the absent-boundary sentence matches maintenance.rs and repository.rs, the runner and judge are unchanged, and all four new bench tests fail on the 2b0f1c9f code and pass at HEAD. REVISE because the completion gate binds verdicts by field name only, so the existing vulcan_surface_review_revision_7 (a review of the revision 6 text) already satisfies the revision-7 binding, which an in-memory probe demonstrated. A P4 notes that the new serve-child reap reaches into private Endpoint state.
