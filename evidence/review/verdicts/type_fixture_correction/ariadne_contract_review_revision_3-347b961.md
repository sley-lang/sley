<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 347b9611efd6fe203282334d1b271c07f09553ae; role: ariadne; field: ariadne_contract_review_revision_3; dispatched: 2026-09-23T04:36:29Z; duration_s: 114; process_exit_code: 0 -->
# Ariadne Council review — type_fixture_correction

Harness: claude-code
Reviewed checkpoint: 347b9611efd6fe203282334d1b271c07f09553ae

**Scope.** `git rev-parse HEAD` returned `347b9611efd6fe203282334d1b271c07f09553ae`, which matches SCOPE_SHA. The worktree was clean.

**Git commands run:**
- `git diff --stat d3de686b..HEAD`: 28 files.
- `git diff d3de686b..HEAD` over:
  - `bench/fixtures/sley2_live_judge.py`
  - `bench/live/succ_witness_type_full.py`
  - `bench/live/tests/test_judge_type_variant.py`
  - `bench/live/TYPE-FIXTURE-REVIEW-PACKET.md`
  - `bench/live/SUCCESSION-COVERAGE.md`
- `git diff --stat d3de686b..9352e0fe`: the only code in this range is the judge, the witness and the unit tests, plus the filed r2 verdict and round index.
- `git diff --stat 9352e0fe..HEAD`: only the packet, the coverage file and the `r3_*` / `trial_type_r3_*` logs. No code changed after the log-generating commit.

**Commands run:**
- `sha256sum` over the judge, the manifest and `base.pack`:
  - judge: `0ffd61d4261ad2847b611c35762e128cf4e22996afb2369b8fe0503311fcddbc`
  - manifest: `d61216a9…308f`
  - `base.pack`: `875630cd…a45afd`
- `python3 -m unittest bench.live.tests.test_judge_type_variant` exited 0 with `Ran 53 tests in 0.005s` / `OK`.
- I did not re-run cargo. I relied on the committed provenance logs `r3_s3_g1_type.log` and `r3_rust_gates.log`, both at git_head 9352e0fe with a judge sha equal to HEAD. They show `cargo exit: 0`:
  - s3_g1_type: `3 passed; 1 ignored`
  - `succ_live_packs_frozen`: ok
  - succ_live_judge_cases: `12 passed`
- I did not run `make lint`, because `scripts/record_lint_report.py:107` writes a report file into the tree.

**Files read:**
- my r2 verdict `evidence/review/verdicts/type_fixture_correction/ariadne_contract_review_revision_2-d3de686.md`, lines 1-108
- `bench/fixtures/sley2_live_judge.py`, lines 770-1108
- `bench/live/TYPE-FIXTURE-REVIEW-PACKET.md`, lines 1-120 (R3) and 236-390 (rev2 §3 boundaries, §4, §5), plus a grep for D3/FAILED_CODE/CasePayload
- `bench/live/succ_witness_type_full.py`, lines 30-60, 101-124, 191-205, 281-311 and 418-424
- `bench/live/tests/test_judge_type_variant.py`: the full diff and the test index
- all 18 `bench/live/succ-trials-20260923/trial_type_r3_*.log` files (provenance, verdict and outcome lines)
- `r3_s3_g1_type.log` and `r3_rust_gates.log` in full
- the result lines of `r3_unittest_suites.log`: `Ran 251` OK; `Ran 23` OK; `Ran 251` OK (skipped=75)

## Evidence checked

**D3 removal (judge diff).** The only behavioural change in `sley2_live_judge.py` is the deletion of the loop that raised `ORACLE_FAILED_CODE "Failed arm drops the error code"` when the Failed case edge had no CasePayload argument. The other hunks are docstring edits (`:784`, `:810-816`). At HEAD, `_type_structure` ends the per-switch check at Member-key coverage and sort order (`:1063-1074`). It then returns the plan (`:1075-1076`) without reading edge arguments. No other predicate changed:
- A1 (Named status): `:956-964`
- A2 (4 members, 1 integer-coded): `:966-994`
- A3 (status value and explicit code): `:997-1014`
- B (Bool scan): `:1016-1017`, `:883-911`
- C1/C2 (single Named JobState param; VariantSwitch on it): `:1019-1028`, `:1053-1062`
- D1 (exhaustive sorted Member keys): `:1063-1074`
- D2 (reachable blocks Required, no Trap): `:1033-1051`
- E (two executions): `:1079-1107`

These are textually identical to d3de686b apart from the line shift from the removed block.

**A2/A3 value-level enforcement is intact.**
- `:987-988`: no coded member → ORACLE_FAILED_CODE (the "null error" outcome).
- `:993-994`: a code type outside `_TYPE_CODE_VARIANTS` → ORACLE_FAILED_CODE.
- `:1005-1007`: a Failed status with a non-`Some` payload (missing or null) → ORACLE_FAILED_CODE.
- `:1008-1012`: a code whose data variant is not an integer, or whose value is not an int or is a bool → ORACLE_FAILED_CODE.

The unit tests prove this on the original shape (`test_failed_status_null_payload_is_failed_code` :247, `test_no_coded_member_is_failed_code` :252, `test_non_integer_code_is_failed_code` :258). They also prove it inside the new discard shape (`:447-468`): a null Failed status, a codeless member, a Text code type and a Text status code, each asserting ORACLE_FAILED_CODE. All 53 tests pass in my run.

**Live check of the null-code path.** `trial_type_r3_neg_nullcode.log` shows production refusing first, with `failed_phase 6`, `TYPE_CONST_SHAPE` and `outcome production_refused … -> PASS`. The judge path stays a fail-closed backstop, as the witness docstring says (`succ_witness_type_full.py:51-55`).

**alt_failed_discard is the r2 neg_dropcode design, and it now accepts.**
- `VARIANTS["alt_failed_discard"] = {"drop_payload": True, "failed_const": True}` (`:115-116`). That is the exact flag set of the retired `neg_dropcode`.
- The construction leaves the Failed edge `arguments` empty (`:309-310`), gives the Failed block no bound parameter (`:286`), and allocates no `pC` slot (`:194-195`). This is the IR form of `Failed(_) => …`.
- `trial_type_r3_alt_failed_discard.log` records design `drop_payload true, failed_const true`, git_head 9352e0fe, a judge sha equal to HEAD, and no dirty paths. It shows `compose valid True`, `finished True`, and verdict `accepted` with `type exec 5151={"SInt":"0"},5252={"SInt":"1"},5353={"SInt":"2"},5454={"SInt":"3"}`.
- Those values are byte-identical to `trial_type_r3_alt_failed_fixed.log`.
- Determinism holds, because `_type_execute` rejects any disagreement between its two in-judge runs (`:1105-1106`), and the run accepted.

**The other rev3 live results are unchanged.** All match the r2 outcomes:
- Accepted:
  - `pos`, `alt_code8`, `alt_queued`, `alt_join`: SInt 0/1/2/7
  - `alt_shared`: 0/0/2/7
  - `alt_uint`: UInt 0/1/2/7
  - `alt_arith`: Ok 0/2/4/107
- ORACLE_BOOL_COMPAT_FIELD:
  - `neg_bool` ("status still Bool")
  - `neg_typedef_only` ("switch result … Bool-typed")
  - `neg_bool_const` ("constant … Bool-typed")
  - `neg_two_param` ("function parameter … Bool-typed")
  - `legacy_mig` and `legacy_neg`
- ORACLE_TRAP_ARM: `neg_trap`.
- Production refusal: `neg_droppayload`, phase 7, CFG_TARGET_ARGUMENTS.

This refusal fits the witness construction: without `failed_const`, the arm still binds `pC` while the edge feeds nothing. It is not a judge rule.

**Frozen fixture is unchanged.** Manifest sha `d61216a9…` and pack sha `875630cd…` match the rev2 values and every r3 provenance line. `succ_live_packs_frozen` passes in `r3_rust_gates.log`.

**Records.**
- Packet R3.1 (lines 20-70) accurately describes the change. Its line citations match HEAD:
  - judge `:1063-1074`, `:987-994`, `:1005-1012`, `:782-786`, `:810-816`
  - witness `:35-38`, `:115`, `:55-59`
  - tests `:441`, `:447-469`
- The §3 D3 row is marked RETIRED (line 200).
- The coverage row and the per-task note (`SUCCESSION-COVERAGE.md:22`, `:370-375`, `:630-631`) state that D3 is retired, that value-level enforcement is unchanged, and that there are 53 tests.

**Prior finding status (r2 P1, D3 consumer-shape pin): CLOSED.** Each item in my closure-evidence list is met:
1. The rejection is removed, with no residual observation.
2. `neg_dropcode` is re-proved as the accepted `alt_failed_discard`, with a fresh provenance log that includes the executed values.
3. The unit test is flipped to `test_failed_edge_discarding_code_accepts`.
4. The docstring, packet §3 D3, boundary 2c and the coverage row are updated. The §4 and §5 items are superseded by the R3.1 enumeration; see the P4 below.

## Findings

[P4] [record-consistency] bench/live/TYPE-FIXTURE-REVIEW-PACKET.md:66-70,200,332-333,341-342 - R3.1's "Superseded in revision 2" list names §3 row D3, the §4 P2 test count, boundary 2c and the §5 `neg_dropcode` line. It omits two rev2 §4 statements that are no longer true at HEAD: §4 [P3] (lines 332-333) says "'Failed arm forwards CasePayload' sits in the CORPUS tier (D3) in both places", and §4 [P4] (lines 341-342) says the witness docstring "names the judge path that `neg_dropcode` reaches live". The retired D3 row (line 200) also still cites judge `:1071-1077`, which at HEAD points to the D1 case-coverage check. These are covered only by the general sentence "stands, except where this section supersedes it" (line 16). They are historical text, not a behavioural defect. - Closure evidence: add §4 [P3] and §4 [P4] to the R3.1 supersession list, and mark the D3 row's `:1071-1077` as a d3de686b line reference (a one-line packet edit, which can go with the adoption commit).

## Assessment

The r2 P1 is closed.

**D3 retired cleanly.** D3's consumer-shape rejection is deleted from `_type_structure` with no residual observation, and the diff touches no other predicate: A1-A3, B, C1-C2, D1-D2 and E are textually unchanged.

**Value-level enforcement intact.** Enforcement of "Failed carries an explicit error code" / "null error" still covers:
- a missing coded member
- a non-integer code type
- a null or absent Failed payload
- a non-integer or boolean code value

This is proved by unit tests on both the bind and discard shapes, with production's TYPE_CONST_SHAPE refusal ahead of the judge.

**Discard arm accepts deterministically.** The payload-free Failed arm, the IR form of the frozen S3 `JobState::Failed(_) => "failed"` arm, is accepted live. It returns the same four values as `alt_failed_fixed` over two in-judge executions. The log is sha-bound to the HEAD judge, and no code changed after it was generated.

Rulings 2a-2c are now all derivable from the frozen text, and packet §4's claim that the judge "narrows nothing beyond the frozen text" now holds. The one P4 is a residual record-consistency nit in historical rev2 prose and does not affect adoptability. On this scope, the fixture correction is adoptable to main as the sley_2_0 TYPE fixture. This verdict claims nothing about GA or release readiness.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_1_P4
SECTION: type_fixture_correction
FIELD: ariadne_contract_review_revision_3
SCOPE_SHA: 347b9611efd6fe203282334d1b271c07f09553ae
FINDINGS: [P4] [record-consistency] bench/live/TYPE-FIXTURE-REVIEW-PACKET.md:66-70,200,332-333,341-342 - R3.1's supersession list omits two rev2 §4 statements that are stale at HEAD (§4 [P3] "'Failed arm forwards CasePayload' sits in the CORPUS tier (D3)" and §4 [P4] "the judge path that `neg_dropcode` reaches live"), and the retired D3 row still cites judge `:1071-1077`, which now points to the D1 check; historical prose only, no behavioural effect - add §4 [P3] and §4 [P4] to the R3.1 supersession list and mark `:1071-1077` as a d3de686b reference
SUMMARY: The revision-2 P1 (D3 consumer-shape pin) is CLOSED: the judge no longer inspects Failed-edge arguments, and no other predicate changed. A2/A3 still reject missing, null, non-integer and boolean codes with ORACLE_FAILED_CODE, proven by 53 passing unit tests including four inside the discard shape. The former neg_dropcode design, now alt_failed_discard, validates in production and is accepted live, executing deterministically to SInt 0/1/2/3 exactly like alt_failed_fixed, with provenance sha-bound to the HEAD judge and an unchanged manifest and pack. One P4 record nit remains in historical rev2 packet prose; the fixture correction is adoptable to main as the sley_2_0 TYPE fixture.
