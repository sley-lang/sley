<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f132c901af63dea5ab65a012bce383d52282ac61; role: ariadne; field: ariadne_contract_review; dispatched: 2026-09-23T04:53:37Z; duration_s: 329; process_exit_code: 0 -->
# Ariadne Council review — dead_task_unblock

Harness: claude-code
Reviewed checkpoint: f132c901af63dea5ab65a012bce383d52282ac61

What I verified myself:
- `git rev-parse HEAD`: f132c901af63dea5ab65a012bce383d52282ac61. Scope matches. Branch work/succ-dead-impl, tree clean.
- `git log --oneline 40e3b97c..HEAD`: 2fd93d25 and f132c901.
- `git diff --stat 40e3b97c..HEAD`: 16 files. `git diff 40e3b97c..2fd93d25 --stat` shows only the code: the judge, the witness and the test. `git diff --stat 2fd93d25..HEAD` shows only records and logs. So the code that produced `trial_dead_pos.log` (git_sha 2fd93d25, dirty_count 0) is byte-identical to HEAD.
- I read the full diff of `bench/fixtures/sley2_live_judge.py`, `DEAD-TOMBSTONE-PROPOSAL.md` and `SUCCESSION-COVERAGE.md`.
- `python3 -m unittest bench.live.tests.test_judge_dead_cascade -v`: exit 0, "Ran 18 tests … OK (skipped=2)". The two skipped tests are the binary-gated end-to-end class; the sandbox does not expose the sley or driver binaries.
- `cargo test --offline -q -p sley-repo --test succ_live_emit succ_live_packs_frozen`: exit 0, "1 passed". This ties the committed base.pack and task_manifest.json byte-for-byte to the `base_dead` emitter.
- `cargo test --offline -q -p sley-policy --lib function_deletion_tests`: exit 0, "7 passed".
- I decoded the two ORACLE_COMMIT_REJECTED detail payloads with python3.
- Files read:
  - `sley2_live_judge.py`: 440-590, 618-706, 1113-1159, 1290-1349, 1376-1415
  - `test_judge_dead_cascade.py`: 1-214
  - `succ_witness_dead.py`: 1-340
  - `crates/sley-repo/tests/s3_g1_dead.rs`: 1-400
  - `crates/sley-repo/tests/succ_live_emit.rs`: 215-244, 381-403, 1160-1258, 1497-1639
  - the S2B-DEAD-001 `task_manifest.json`, `expect.json`, `oracle.py`, `live_oracle.py` and `fixture.json`
  - `bench/corpus/v1/tasks.json`: 71-78
  - `bench/benchmark-plan.json`: 124
  - `DEAD-TOMBSTONE-PROPOSAL.md`: 1-91
  - all 10 trial logs, plus `cargo_test_sley_policy.log` (head and result lines)
  - REQ-11 verdict files (VERDICT lines)

## Evidence checked

**Frozen authority**
- `tasks.json:71-78`: "unreachable block removed", "unused private helper removed", observation unchanged, no effect expansion. strict_oracle: removed_blocks 1, removed_private_functions 1. Forbidden: public deletion, test deletion, reachable CFG change.
- Manifest judge: absent `[dead_block 90, dead_helper 8e]`, fixed_inputs `[[7,10]]`, note "helper parameter and block follow their function". Targets: 88, 89, 8d, 90, 8e.
- The emitter comment at `succ_live_emit.rs:1164-1167` says: "The fix drops the block from the function, then deletes the block and the helper with its parameter and block."
- `mutation_rule` (`benchmark-plan.json:124`): no fixture, corpus or manifest file changed in the diff. Only the judge implementation changed.

**Frozen pack structure (emitter, proven equal to the committed pack by `succ_live_packs_frozen`)**
- Namespace 88 has members {89, 8E, 92}.
- Live function 89 has blocks [8A, 90]. Block 90 (function 89, ExplicitlyUnreachable) contains op 93 (block 90, ConstantRef → constant 92).
- Helper 8E has parameter 91 (owner 8E) and block 8F (function 8E, no ops).
- The pack contains no TestCase entity.
- The unit-test OWNERS map (`test_judge_dead_cascade.py:43-45`) matches these edges exactly.

**Q1: does the cascade implement the frozen manifest?**
- In `sley2_live_judge.py:519-535`, the cascade is consulted only for non-target, non-absent entities whose version changed. It is computed only when absent roles exist. The absent check (512-514) runs first, so a cascade can only apply once the owner is tombstoned.
- Owned entities are accepted only if deleted (`entity not in versions_post`). Kept or modified owned entities reject ORACLE_UNEXPECTED_ENTITY (533). Everything else still rejects ORACLE_COLLATERAL_TOUCHED (535).
- Ownership comes from decoded pre-state bodies (543-565): kinds 6→owner, 7→function, 8→block, matching the witness KINDS table. A missing pre object fails the harness. An undecodable or odd body is simply not owned, which fails closed toward COLLATERAL.
- TestCase (kind 14) and Constant (kind 9) never ride the cascade, so test deletion and deletion of 92 remain rejected. That matches the forbidden outcomes and the strict removed counts.
- Only DEAD declares `absent`. MODULE is the only other graph-flow manifest and never computes a cascade (unit test `test_manifest_without_absent_roles_never_derives_ownership`). No historical verdict changes.
- The positive log itself proves that 8f, 91 and 93 were all in the live cascade. Each changed, and the accepted verdict implies none reached line 535. This contrasts with `trial_dead_pos_prejudgefix.log` at 40e3b97c: `ORACLE_COLLATERAL_TOUCHED` on 8f8f….
- Nothing new is accepted except the removal of structure that cannot exist without its tombstoned owner. The orphan of 93 is refused by validation (phase 5), and by the commit when finish is bypassed. Without the 93 arm the frozen task would be unsatisfiable. Acceptance is not widened, and no candidate allowed by the frozen text is newly rejected.

**Q2: is the positive a genuine end-to-end acceptance?**
- The witness drives `sley2_tool.main` read → propose (candidate.create + candidate.validate) → finish. It then calls `judge.main("S2B-DEAD-001")`, which is exactly what `live_oracle.py` calls, and the judge does its own production commit.
- `trial_dead_pos.log` (2fd93d25, dirty 0):
  - ops: Replace 88, Replace 89, Delete 90, 93, 8e, 8f, 91
  - validation: decision tag 1, no diagnostics, required_capabilities [], selected_tests []
  - affected_closure still names 8e, 8f, 90, 91, 93
  - `finished True`; verdict `{"status": "accepted", "code": null, "detail": "all flows held"}`; judge exit 0
- The binary sha256 values in the log match the record (60e77a54…0314 and d8e495f8…1cf8). I could not re-hash them (outside the allowed directory) or re-run the witness (no binaries in the sandbox).

**Q3: are the negatives' symbols as logged?** I checked each log against SUCCESSION-COVERAGE.md:93-163 and the DEAD row:

| Negative | Logged result | Matches record |
|---|---|---|
| orphan93 | tag 8, phase 5, GRAPH_UNRESOLVED_REFERENCE (22004/36006), finish refused | ✓ |
| orphan93_bypass | ORACLE_COMMIT_REJECTED; decoded detail contains `CANDIDATE_VALIDATION_UNRESOLVED_REFERENCE`, varint 36006 | ✓ |
| test_on_tombstone | phase 5 GRAPH_UNRESOLVED_REFERENCE, finish refused | ✓ |
| helper_kept | ORACLE_UNEXPECTED_ENTITY with detail `dead_helper` (pre-existing absent check at 512-514) | ✓ |
| reachable_changed | op 8d 98→100 (witness asserts 98), ORACLE_REACHABLE_CHANGED; observation runs before `_judge_graph` (1337-1339) | ✓ |
| public_deleted | ORACLE_PUBLIC_DELETED, detail 8989… | ✓ |
| extra_delete | ORACLE_COLLATERAL_TOUCHED, detail 9292… | ✓ |

**Q4: is the "survivor test selection non-empty" disposition accurate?**
- The frozen pack has no TestCase (emitter plus pack gate), so an empty selection is forced for the frozen candidate. The record explicitly does not claim this clause for the frozen candidate.
- `trial_dead_probe_survivor_test.log`: validation tag 1 with selected_tests = [441ef4f3…], the created test targeting survivor 89, and helper tombstones in the closure. The commit is then refused with ORACLE_COMMIT_REJECTED; the decoded detail is `TXN_TEST_EVIDENCE_UNSUPPORTED`, varint 39008, which matches `docs/spec/ERROR_CODES_V1.md:431` and `crates/sley-txn/src/codec.rs:333`.
- This gate was recorded earlier for CREATE (`GATE-RECORD-20260921.md:116`, `SUCCESSION-COVERAGE.md:360`), so "pre-existing" is accurate.
- Required-test and survivor-test deletion are pinned by REQ-11's `native_test_plan::function_deletion_tests` (7 passed, re-run by me), including `deleting_the_helper_with_its_protected_required_test_still_refuses`.

**Records**
- REQ-11 commits 883361e3 and f7f9af90 are ancestors of HEAD.
- Verdicts on file: Ariadne PASS (883361e), Nabu REVISE (883361e), then Nabu rev2 PASS (f7f9af9). The docs say "Ariadne PASS, Nabu PASS rev2", which is accurate.
- The proposal's status block preserves the historical text unchanged.
- The DEAD row does not claim a live-model result, and `ga_claimed=false` is retained.

## Findings
[P4] [record-precision] bench/fixtures/sley2_live_judge.py:501-504, bench/live/SUCCESSION-COVERAGE.md:116-119 - The docstring and record attribute the whole ownership cascade, including Operation.block (op 93 following dead block 90), to the manifest note "helper parameter and block follow their function". That note literally covers only the helper's parameter and block. The block→operation arm actually rests on the corpus requirement "unreachable block removed" (`tasks.json:75`), the frozen emitter comment (`succ_live_emit.rs:1166-1167`), and validation refusing the orphan (`trial_dead_neg_orphan93.log`). The behavior is correct; only the attribution is overstated - closure evidence: amend the docstring or record to cite the corpus requirement and emitter for the block→operation arm.
[P4] [oracle-coverage-note] bench/fixtures/sley2_live_judge.py:1333-1340 - This predates the diff and does not affect this positive. The graph flow has no explicit predicate for the frozen required outcome "effect closure does not expand" (`tasks.json:75`); effect checks exist only in the PERF flow (2984-3008). Target 89 may change in any way that keeps the (7,10) observation. This positive satisfies the outcome on its own evidence: required_capabilities [], and the witness replaces only 89's `blocks` (`succ_witness_dead.py:195-196`). Future live-model DEAD trials would depend on validation and commit alone - closure evidence: a judge predicate comparing the pre and post effect sets of the live function (or of the whole closure), or a recorded statement of which production phase enforces non-expansion, with a negative vector.

## Assessment
1. **Cascade vs. frozen manifest:** the cascade implements the manifest without widening or narrowing acceptance.
   - Only structure owned in the pre-state by a tombstoned absent role (8f, 91, 93) may disappear, and only by deletion. Keeping or modifying it rejects ORACLE_UNEXPECTED_ENTITY; this is unit-pinned, since validation already refuses the live forms.
   - Unowned collateral, including constant 92 and any TestCase, still rejects ORACLE_COLLATERAL_TOUCHED.
   - The absent, observation and public checks are unchanged. MODULE's graph flow is unaffected.
   - No frozen corpus, fixture or manifest artifact changed, so `mutation_rule` is not engaged. The change is a conformance fix that makes the judge honor frozen text it previously contradicted.
2. **Positive:** a genuine scripted end-to-end acceptance through the real tool surface and the judge's production commit, on a clean tree whose code is identical to HEAD.
3. **Negatives:** every symbol matches its log verbatim.
4. **Survivor-selection disposition:** accurate. The frozen pack has no TestCase. The probe shows non-empty survivor selection at validation, and its commit is refused by the pre-existing TXN_TEST_EVIDENCE_UNSUPPORTED gate (39008).

The two findings are notes. The DEAD positive may be recorded as accepted on the succession arm, as scripted evidence only; the live-model trial and REQ-11's landing condition remain open, as the records state.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_2_P4
SECTION: dead_task_unblock
FIELD: ariadne_contract_review
SCOPE_SHA: f132c901af63dea5ab65a012bce383d52282ac61
FINDINGS: [P4] [record-precision] bench/fixtures/sley2_live_judge.py:501-504, bench/live/SUCCESSION-COVERAGE.md:116-119 - The docstring and record attribute the whole ownership cascade, including Operation.block (op 93 following dead block 90), to the manifest note "helper parameter and block follow their function", which literally covers only the helper's parameter and block; the block→operation arm rests on corpus "unreachable block removed" (tasks.json:75), the frozen emitter comment (succ_live_emit.rs:1166-1167) and validation refusing the orphan; behavior correct, attribution overstated - closure evidence: amend the docstring or record to cite the corpus requirement and emitter for that arm. | [P4] [oracle-coverage-note] bench/fixtures/sley2_live_judge.py:1333-1340 - Predates the diff: the graph flow has no explicit predicate for the frozen outcome "effect closure does not expand" (tasks.json:75); effect checks exist only in the PERF flow (2984-3008), and target 89 may change in any way that keeps the (7,10) observation; this positive satisfies the outcome on its own evidence (required_capabilities [], witness replaces only 89's blocks) - closure evidence: a judge effect-set predicate or a recorded statement of the enforcing production phase, with a negative vector.
SUMMARY: The graph-judge ownership cascade matches the frozen S2B-DEAD-001 manifest and corpus text: only structure owned in the pre-state by the tombstoned dead block or helper (8f, 91, 93) may disappear, and only by deletion, while unowned collateral, tests and constant 92 still reject; no frozen artifact changed. The positive, run on a clean tree identical to HEAD, is a genuine end-to-end acceptance through the real tool, finish and the judge's production commit, and every negative's symbol matches its log verbatim. The survivor-selection disposition is accurate: the frozen pack has no TestCase, and the probe's non-empty selection is refused at commit by the pre-existing TXN_TEST_EVIDENCE_UNSUPPORTED (39008) gate. Two P4 notes only; the DEAD positive may be recorded as accepted on the succession arm as scripted evidence.
