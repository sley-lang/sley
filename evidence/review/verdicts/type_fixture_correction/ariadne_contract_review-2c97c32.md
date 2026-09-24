<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2c97c32f4dd5d87040cffee2af4b7519ed0f5bdf; role: ariadne; field: ariadne_contract_review; dispatched: 2026-09-23T03:14:38Z; duration_s: 319; process_exit_code: 0 -->
# Ariadne Council review — type_fixture_correction

Harness: claude-code
Reviewed checkpoint: 2c97c32f4dd5d87040cffee2af4b7519ed0f5bdf

I verified the scope first. `git rev-parse HEAD` returned `2c97c32f4dd5d87040cffee2af4b7519ed0f5bdf`, which matches SCOPE_SHA, and the worktree was clean. The git commands I ran were:
- `git log --oneline e8aef7c0..HEAD`
- `git diff --stat e8aef7c0 HEAD` over the fixture, emitter, judge and witness paths
- `git diff e8aef7c0 HEAD -- bench/fixtures/sley2/S2B-TYPE-001/`
- `git diff e8aef7c0 0b26c39c -- crates/sley-repo/tests/succ_live_emit.rs`
- `git show e8aef7c0:bench/fixtures/sley2_live_judge.py` (the base TYPE judge, lines 741-767)
- `git log` on `base.pack` and on the `trial_type_*` logs
- `sha256sum` of the fixture files, plus the same hash of `git show e8aef7c0:…/task_manifest.json`

I ran two test commands:
- `python3 -m unittest discover -s bench/live/tests -t .` exited 0: `Ran 198 tests … OK (skipped=75)`. The 75 skips are 46 × "sley binary unavailable", 13 × "needs SLEY2_SLEY_BINARY", 11 × "+ bwrap" and 5 × "+ SUCC_JUDGE_TEST_BINARY + bwrap". None of the 198 tests exercises `_judge_type_variant`.
- `cargo test -p sley-repo --test succ_live_emit` exited 0: `succ_live_packs_frozen ... ok`, and `emit_succ_live_packs` was ignored.

I read these files in full or over the stated ranges:
- the packet `bench/live/TYPE-FIXTURE-REVIEW-PACKET.md`, lines 1-179, including §6
- the frozen task `bench/corpus/v1/tasks.json`, lines 44-50
- `bench/benchmark-plan.json`, line 124 (`mutation_rule`)
- the emitter `succ_live_emit.rs`, lines 680-770
- the judge `sley2_live_judge.py`, lines 495-531, 740-1046 and 1170-1296
- `bench/live/tooling.py`, lines 95-160
- `bench/live/taskpacks.py`, lines 1-140
- `succ_witness_type_full.py`, lines 1-382, and `succ_witness_type.py`, lines 1-40
- all 11 `succ-trials-20260921/trial_type_*.log` files and `capture_type_pos.log`
- `expect.json` and `live_oracle.py` for S2B-TYPE-001

## Evidence checked

- **Manifest correction is exactly as described.** The diff of `task_manifest.json` against e8aef7c0 adds only the `switch_entry`/`switch_leaf` entities (0x6d, 0x6e) and the two matching targets. `pack_digest_blake3` `7579dee1…` is unchanged. `base.pack` was last touched at 3badd822. `task_manifest.v1-frozen.json` has sha256 `4b9e2eac4ec1…`, identical to the e8aef7c0 manifest, so the original is preserved. The emitter diff at 0b26c39c (`succ_live_emit.rs:754-755, 758-769`) touches only entities, targets and a comment. `succ_live_packs_frozen` passes.
- **The §2 defect is real at the judge/closure level.** In the base, entry 0x6d is a `CondBranch` on `Parameter(0x6c)` with a true→6d self-loop and a false→6e edge (`succ_live_emit.rs:718-737`). Leaf 0x6e returns 0x6c (`:738-749`). `_collateral_files` rejects any modified base entity outside `targets` (`sley2_live_judge.py:1242-1247`). So under the v1 targets `[65,6b,6c]`, a migration that rewrites the switch's own blocks is `ORACLE_COLLATERAL_TOUCHED`. The widening to 5 targets is the minimal closure for that route.
- **The judge only narrows acceptance against the frozen-base judge.** Every e8aef7c0 predicate is retained: status not Bool (`:834-835`), at least 4 blocks (`:922-923`), all blocks Required (`:934-935`). Every other check is new rejection. The only new acceptance path is the closure widening, which is disclosed and confined to the switch's own blocks. The §6 "retirements" relax 0b26c39c-only pins that never reached main, so no path accepted here was rejected by the e8aef7c0 judge.
- **Production-refusal claims check out.** `trial_type_neg_droppayload.log` shows `compose valid False … failed_phase 7, tag 10`, and its hex detail decodes to `CFG_TARGET_ARGUMENTS`. `trial_type_neg_nullcode.log` shows `failed_phase 6, tag 9`, decoding to `TYPE_CONST_SHAPE`. Both support §6 lines 132-140.
- **What the agent is told.** The prompt carries only the frozen corpus task JSON (`tooling.py:152-160`). No manifest target list and no fixture-tier predicate reaches the agent.

## Findings

[P1] [contract-semantics] bench/fixtures/sley2_live_judge.py:780-789,919-920,987-991,1003-1043 - The judge's "FIXTURE tier" adds restrictions that come from how the positive witness happened to be built. That is the same kind of unsupported pin the packet itself retired in §6 (literal 7, status-is-Failed, distinct leaves). The frozen text does not require them (tasks.json:44-50) and the agent never sees them (tooling.py:152-160). The restrictions are: switch result must be exactly SInt; every non-Failed leaf must directly `Return` a `ConstantRef` (opcode 1) of an SInt Constant entity; the Failed leaf must return its own Block param unchanged; and every block must be the entry or a direct case target, so a shared exit or intermediate block rejects as "unreachable block present". As a result, correct exhaustive migrations are scored as ORACLE_TYPE_NOT_MIGRATED, ORACLE_MISSING_CASE or ORACLE_FAILED_CODE. Examples: a non-SInt result type, leaves computing the value with arithmetic ops, a Failed arm mapping the code to a fixed value, or a join block. Under `mutation_rule` (benchmark-plan.json:124), narrowing the oracle beyond the frozen task is a semantic oracle change. - Closure evidence: either retire these pins to what the corpus requires (non-Bool, exhaustive Required non-Trap Member arms, payload carried, deterministic results) and add accepted witness runs for at least a non-SInt result and a join-block design, or record them as a new corpus version per mutation_rule. Also reconcile the tier placement (see the P3 finding).

[P2] [forbidden-outcome-coverage] bench/fixtures/sley2_live_judge.py:834-835,898-913,1208-1246 - "parallel boolean compatibility field" (tasks.json:50) is checked only on the three named bindings: status, param and switch result. Fresh entities are always allowed (`:1208-1211, 1245-1246`). Entry-block operations and non-Failed leaf parameters are never inspected. So a full migration that also keeps a fresh Bool constant, or a Bool value threaded from entry operations into a leaf block parameter, has no rejecting predicate. This comes from code reading and was not executed. In addition, a switch given a second, Bool compatibility parameter reaches `_harness_fail("type param")` (`:903-906`). That produces exit 2, "never a verdict", rather than a rejection, because the manifest emits no `param`/`switch_param` role (`succ_live_emit.rs:750-755`) even though the docstring claims one (`:782`). - Closure evidence: a Bool scan over fresh and target entities and block parameters, a named param role in the manifest, and witness negatives (fresh Bool constant; two-parameter switch) rejecting with ORACLE_BOOL_COMPAT_FIELD.

[P2] [contract-semantics] bench/fixtures/sley2_live_judge.py:756-1045; bench/live/TYPE-FIXTURE-REVIEW-PACKET.md:151-160 - The frozen strict oracle is `type_graph_and_execution` (tasks.json:49). The live TYPE judge never executes the candidate: `_run_driver` is used by other flows but not within `_judge_type_variant`. Packet §6 describes validation-time typechecking and the frozen S3 VM suite, which never runs the agent's candidate, as "behavioral"/"execution machinery". That overstates what is enforced for "serialized semantic values are deterministic". - Closure evidence: execute the migrated switch over all four member values (with a fixed Failed code) through the driver and check deterministic results with the payload preserved, or record an explicit, reviewed disposition that sley_2_0 TYPE execution is out of scope.

[P2] [evidence] bench/live/succ-trials-20260921/trial_type_*.log; TYPE-FIXTURE-REVIEW-PACKET.md:36-38,79-89,145-149 - The cited logs record only judge exit codes, never the rejection code or detail. The packet's claims of `MISSING_CASE` (trial_type_migration), `BOOL_COMPAT` (trial_type_neg, trial_type_full_neg_bool) and `ORACLE_FAILED_CODE` (trial_type_full_neg_code) are therefore not supported by the logs. The alt_code8, alt_queued and alt_shared logs are indistinguishable from the default positive: all read "witness/pos" and none records its TYPE_CODE/TYPE_STATUS/TYPE_LEAFS settings. So they do not show that the alternatives were actually run. The trap-arm and typedef-only negatives (§4 lines 81-85) and the s3_g1_type re-run (§4 line 92) cite no log at all. - Closure evidence: re-run the witnesses with logs that capture the judge's JSON verdict (status/code/detail) and the provenance environment, plus logs for neg_trap, neg_typedef_only and the s3_g1_type run.

[P2] [test-coverage] bench/live/tests/ (no TYPE judge tests); bench/fixtures/sley2_live_judge.py:756-1045 - None of the 198 bench/live unittest cases exercises `_judge_type_variant` or its new codes (ORACLE_TYPE_NOT_MIGRATED, ORACLE_SWITCH_NOT_MIGRATED, ORACLE_FAILED_CODE, ORACLE_TRAP_ARM). The retired pins also have no regression tests. The only evidence is ad-hoc witness exit codes. - Closure evidence: unit tests over decoded-body fixtures, one per rejection path and per retired-pin acceptance, run green in the suite.

[P3] [record-consistency] bench/live/TYPE-FIXTURE-REVIEW-PACKET.md:52-59,115-120,162-178 - The packet contradicts itself and the code:
- §3 still lists "status Failed(7)" and "leaves return distinct SInt" as enforced predicates, which §6 retires.
- §6 puts "Failed arm forwards CasePayload / Failed leaf returns its Block param" in the CORPUS tier, while the judge docstring (`sley2_live_judge.py:787-789`) puts them in the FIXTURE tier.
- §5 asks for approval of (a) and (b), while §6 asks for (a), (b) and (c).
- The sections are ordered 4, 6, 5.
- Closure evidence: a single coherent packet revision whose tier table matches the judge docstring.

[P4] [stale-doc/dead-code] bench/live/succ_witness_type_full.py:21-22; bench/fixtures/sley2_live_judge.py:797-799,894-895,998,1043-1045 - The witness docstring says TYPE_DROPPAYLOAD leads to "judge ORACLE_FAILED_CODE", but production refuses before the judge runs (the log shows phase 7). The judge docstring likewise attributes the null-code negative to the judge. `targets` and `seen_const_values` are computed but unused, and the `switch_entry`/`switch_leaf` roles are never read. - Closure evidence: corrected docstrings and removal or use of the dead bindings.

[P4] [defect-demonstration] bench/live/succ_witness_type.py:12-24; TYPE-FIXTURE-REVIEW-PACKET.md:29-35 - §2 says a complete migration "cannot validate" under v1. The direct route, editing 6d/6e, would be rejected by the judge's collateral check rather than by validation. The alternative route (a fresh entry and fresh blocks through the targeted 0x6b, leaving 6d/6e orphaned) is asserted blocked, but no retained log shows it. The defect stands on the collateral path, so this is a note. - Closure evidence: a v1-manifest witness log for the fresh-entry route, or rewording of the §2 claim.

## Assessment

The closure and manifest correction is sound and minimal:
- Adding 6d/6e is the smallest closure that lets the frozen "update switches" goal be carried out.
- Pack bytes and digest are unchanged and the original manifest is preserved.
- The frozen-packs test passes.
- Against the frozen-base judge, the TYPE judge only adds rejections, so there is no new acceptance path for a wrong migration beyond the disclosed closure widening.

The problem is the reverse direction. The packet's own reasoning for retiring the literal-7, status-is-Failed and distinct-leaves pins applies equally to the retained "fixture tier" pins. Those pins reject correct, exhaustive migrations for reasons the agent is never told, which amounts to a semantic change to the oracle under the append-protected corpus. The frozen `execution` half of the oracle is also not implemented for the candidate. Coverage of the forbidden parallel-Bool outcome is partial, and one such case becomes a harness error instead of a rejection. Finally, the cited logs cannot substantiate the specific rejection codes or the parameterised alternatives, and no unit tests pin any of it. The fixture is not adoptable to main as the sley_2_0 TYPE fixture until the P1 is resolved and the P2 evidence and coverage gaps are closed.

VERDICT: REVISE_0_P0_1_P1_4_P2_1_P3_2_P4
SECTION: type_fixture_correction
FIELD: ariadne_contract_review
SCOPE_SHA: 2c97c32f4dd5d87040cffee2af4b7519ed0f5bdf
FINDINGS: [P1] [contract-semantics] bench/fixtures/sley2_live_judge.py:780-789,919-920,987-991,1003-1043 - fixture-tier pins (SInt result, direct ConstantRef leaves, Failed leaf returns raw payload, no join blocks) are witness-shape restrictions absent from the frozen task and the agent prompt; correct migrations are rejected, a semantic oracle narrowing under mutation_rule - retire to corpus predicates with accepted alternative witnesses, or version the corpus; [P2] [forbidden-outcome-coverage] bench/fixtures/sley2_live_judge.py:834-835,898-913,1208-1246 - parallel Bool compat checked only on status/param/result; fresh Bool entities and entry-op/block-param Bool values are unchecked; a two-parameter switch is a harness error, not a rejection - add a Bool scan, a param role and rejecting witness negatives; [P2] [contract-semantics] bench/fixtures/sley2_live_judge.py:756-1045 + packet:151-160 - frozen strict_oracle type_graph_and_execution is not executed on the candidate; the packet presents typechecking as execution - execute the four member cases or record a reviewed disposition; [P2] [evidence] bench/live/succ-trials-20260921/trial_type_*.log + packet:36-38,79-89,145-149 - logs hold exit codes only; claimed rejection codes are absent; alt logs lack provenance parameters; trap, typedef-only and s3 re-run are unlogged - re-run with verdict JSON and environment captured; [P2] [test-coverage] bench/live/tests/ - no unit test exercises _judge_type_variant, its new codes or the retired-pin acceptances - add per-path tests; [P3] [record-consistency] bench/live/TYPE-FIXTURE-REVIEW-PACKET.md:52-59,115-120,162-178 - §3 lists retired pins as enforced, §6 tiering contradicts the judge docstring, §5/§6 approval requests differ, section order is 4/6/5 - issue one coherent revision; [P4] [stale-doc/dead-code] bench/live/succ_witness_type_full.py:21-22 + sley2_live_judge.py:797-799,894-895,998,1043-1045 - docstrings attribute production refusals to the judge; unused targets/seen_const_values/new roles - correct or remove; [P4] [defect-demonstration] bench/live/succ_witness_type.py:12-24 + packet:29-35 - "cannot validate" is imprecise (the collateral judge rejects) and the fresh-entry route is unlogged - add a log or reword
SUMMARY: The 6d/6e closure is a minimal, correctly emitted operationalization of the frozen TYPE task: pack and digest are unchanged, the v1 manifest is preserved, and succ_live_packs_frozen passes. Against the frozen-base judge, the judge only adds rejections. However, the retained fixture-tier predicates repeat the witness-shape over-restriction that §6 retired, so correct migrations would be rejected for reasons the agent is never told. The execution half of the oracle and parts of the parallel-Bool forbidden outcome are not enforced, and the cited logs and absent unit tests do not support the specific rejection-code and alternative-positive claims. REVISE before main adoption.
