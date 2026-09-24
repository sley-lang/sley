<!-- engine: claude-code; observed model: claude-opus-5-5; scope: d3de686b0019ccbde58939567ae34a899a2be894; role: ariadne; field: ariadne_contract_review_revision_2; dispatched: 2026-09-23T04:07:16Z; duration_s: 297; process_exit_code: 0 -->
# Ariadne Council review — type_fixture_correction

Harness: claude-code
Reviewed checkpoint: d3de686b0019ccbde58939567ae34a899a2be894

I verified the scope first. `git rev-parse HEAD` returned `d3de686b0019ccbde58939567ae34a899a2be894`, which matches SCOPE_SHA, and the worktree was clean.

Git commands run:
- `git status --short`
- `git diff --stat ab42a3a9..HEAD`: 30 files changed.
- `git diff ab42a3a9..HEAD` over `crates/sley-repo/tests/succ_live_emit.rs`, `bench/fixtures/sley2/S2B-TYPE-001/task_manifest.json`, `crates/sley-repo/tests/succ_live_judge_cases.rs` and `bench/live/SUCCESSION-COVERAGE.md`
- `git show ab42a3a9:bench/fixtures/sley2/S2B-TYPE-001/task_manifest.json | sha256sum`: `2dcb3d72…`
- `git log -- bench/fixtures/sley2/S2B-TYPE-001/base.pack`: last touched at 3badd822.
- `git diff --stat 04fa14e6 HEAD`: only the packet, the coverage file and logs changed after the log-generating commit. No code changed.
- `sha256sum` over the fixture directory, the judge and the witness.

Test runs:
- `python3 -m unittest bench.live.tests.test_judge_type_variant` exited 0: `Ran 49 tests … OK`.
- `cargo test -p sley-repo --test succ_live_emit --test s3_g1_type --test succ_live_judge_cases` exited 0:
  - s3_g1_type: `3 passed; 1 ignored`
  - succ_live_emit: `succ_live_packs_frozen ... ok` (1 passed; 1 ignored)
  - succ_live_judge_cases: `12 passed`, including `variant_unit_and_payload_members_decode` and `variant_payload_is_never_defaulted_or_dropped`.

Files read:
- my prior verdict `evidence/review/verdicts/type_fixture_correction/ariadne_contract_review-2c97c32.md` (full)
- the packet `bench/live/TYPE-FIXTURE-REVIEW-PACKET.md`, lines 1-526
- the frozen task S2B-TYPE-001 in `bench/corpus/v1/tasks.json`, and `mutation_rule` in `bench/benchmark-plan.json` (via a JSON extract)
- `bench/fixtures/sley2_live_judge.py`, lines 740-1110 and 1269-1363
- `bench/live/tests/test_judge_type_variant.py`, lines 1-510
- `crates/sley-repo/tests/succ_live_emit.rs`, lines 308-370 and 688-777
- the full driver diff, plus the `non_success` rendering at `succ_live_judge_cases.rs:563-571`
- `crates/sley-check/src/cfg.rs`, lines 790-816
- the Terminator, SwitchEdge and VariantSwitchTerminator definitions in `crates/sley-ssmc/src/lib.rs`, lines 1227-1400
- `crates/sley-repo/tests/s3_g1_type.rs`, lines 185-207
- `bench/live/succ_witness_type_full.py`, lines 1-125 and 180-330
- `bench/live/succ_witness_type.py`, lines 1-32
- all 18 `bench/live/succ-trials-20260923/trial_type_*.log` files, plus the result lines of `unittest_suites.log`, `s3_g1_type.log` and `rust_gates.log`

## Evidence checked

- **Manifest and closure.**
  - The emitter diff adds only `entities.insert("switch_param", eid(0x6C))` with a comment (`succ_live_emit.rs:754-758`).
  - The manifest diff is one added `switch_param` line. Its sha256 moves from `2dcb3d72…` to `d61216a9…`, as the packet claims.
  - `base.pack` sha256 is `875630cd…a45afd` and is unchanged since 3badd822. `task_manifest.v1-frozen.json` sha256 is still `4b9e2eac…`.
  - `succ_live_packs_frozen` passes.
  - The base contains only one other Bool binding: the unrelated SInt comparator 0x66 (`succ_live_emit.rs:308-370`), which is not a status binding.
- **Log provenance ties to the reviewed code.**
  - Every 20260923 witness log records `judge_sha256 f56fad736d622b…`, which is byte-identical to `sley2_live_judge.py` at HEAD. Each also records `manifest_sha256 d61216a9…`, `git_head 04fa14e6`, the effective design, the judge's verdict JSON and a self-check line.
  - `git diff --stat 04fa14e6 HEAD` touches no code, so the logs evidence the reviewed judge, witness and driver.
- **Accepted designs execute.** `trial_type_alt_arith.log` shows `5454={"Result":{"Ok":{"SInt":"107"}}}`, and `alt_uint` shows UInt values. `alt_join`, `alt_failed_fixed` (Failed→3) and `alt_shared` behave as §5 states.
- **Rejections match the claimed codes.**
  - `neg_bool_const` → `constant fb5dac3f… Bool-typed`
  - `neg_two_param` → `function parameter d7e848e9… Bool-typed`
  - `neg_trap` → ORACLE_TRAP_ARM, with compose valid True
  - `neg_typedef_only` → switch result Bool
  - `neg_dropcode` → ORACLE_FAILED_CODE, with compose valid True
  - `neg_droppayload` → production refusal, phase 7 `CFG_TARGET_ARGUMENTS`
  - `neg_nullcode` → production refusal, phase 6 `TYPE_CONST_SHAPE`
- **Execution half.**
  - `_type_execute` (`sley2_live_judge.py:1082-1110`) runs all four members twice through `_run_driver`. A result that is not ok, is null, or carries `non_success` (rendered by the driver at `succ_live_judge_cases.rs:567`) becomes ORACLE_MISSING_CASE. Runs that differ become ORACLE_NONDETERMINISTIC.
  - The driver's variant decoding (`variant_of`) requires the payload exactly when the case declares one. It rejects unknown members and stray keys, and range-checks the payload. Its tests pass.
  - No other bench code relied on the removed "variant definition unsupported" error: a grep found no matches.
- **Bool scan.** `_type_bool_scan` (`:879-907`) is applied at `:1012-1013` over fresh entities, the `switch_param` role and the targets. `_type_has_bool` recurses through composite types.
- **Unit tests.** All 49 run. The per-code counts in packet §4 match the file: 6 TYPE_NOT_MIGRATED, 3 SWITCH_NOT_MIGRATED, 5+2 MISSING_CASE, 4 FAILED_CODE, 1 TRAP_ARM, 9 BOOL_COMPAT_FIELD and 1 NONDETERMINISTIC. There are 12 retired-pin acceptance tests at `:345-437`. The execution tests stub the driver; live execution is evidenced by the witness logs.
- **Frozen S3 reference switch.** `s3_g1_type.rs:191,194-200` declares `Failed(u16)` and handles it as `JobState::Failed(_) => "failed"`. So the frozen suite's own exhaustive switch discards the code.

Prior-finding status (from `ariadne_contract_review-2c97c32.md`):
- [P1] contract-semantics (fixture-tier pins: SInt result / direct ConstantRef leaves / raw-param Failed leaf / entry-or-target blocks): CLOSED. All four pins are gone from `_type_structure` (`:930-1080`). Acceptance is proved live by `alt_uint`, `alt_arith`, `alt_join` and `alt_failed_fixed`, and by unit tests at `:348-419`. (A residual pin of the same class is raised below as a new finding.)
- [P2] forbidden-outcome-coverage (parallel Bool): CLOSED. The scan covers fresh and target Constants, Parameters, GlobalValues, TypeDef members and the switch result. The `switch_param` role is required (`:945-947`) and always scanned, so there is no harness error any more. Live rejections: `neg_bool_const` and `neg_two_param`. Unit tests cover block parameters, record fields and globals.
- [P2] contract-semantics (execution not performed): CLOSED. `_type_execute` plus driver variant inputs, with executed values recorded in every accepted log.
- [P2] evidence (exit-code-only logs): CLOSED. The logs carry provenance and verdict JSON, the judge sha matches HEAD, and neg_trap, neg_typedef_only and the s3 re-run are now logged. Packet §5 lines 286-296 mark the old claims unsupported.
- [P2] test-coverage: CLOSED. 49 tests, green in my run.
- [P3] record-consistency: CLOSED. There is one tier table (§3), sections are in order, there is one approval request (§7), and the docstring at `:782-830` agrees with §3.
- [P4] stale-doc/dead-code: CLOSED. The witness docstring (`succ_witness_type_full.py:49-55`) now attributes the refusals to production, and `targets` is now used at `:1013`.
- [P4] defect-demonstration: CLOSED. The claim is reworded at `succ_witness_type.py:14-29`, and the fresh-entry route is stated as unlogged.

Rulings requested by the packet:
- (1a) Two unit cases sharing one arm block: APPROVED. Every case is still an explicit Member key with no default key (`:1059-1070`). Production accepts only canonical exhaustive cases. Execution drives all four members. "All four cases are exhaustively handled" does not require distinct arm blocks.
- (1b) Dead blocks marked ExplicitlyUnreachable are ignored: APPROVED. Production enforces that a block is reachable exactly when it is Required (`cfg.rs:807-813`), so such a block is provably unreachable from the entry and handles no case. Fresh block parameters are still scanned for Bool.
- (2a) The error code must be an integer: APPROVED. "error_code" plus "explicit error code" reads naturally as an integer code. The frozen S3 vector is `Failed(u16)` (`s3_g1_type.rs:191`). Any SInt or UInt width is accepted.
- (2b) The switch keeps a single input: APPROVED. The base switch is a function of the status alone (`succ_live_emit.rs:698-707`). "Update switches" migrates that switch; it does not authorise new inputs. An extra Bool input is independently the forbidden outcome.
- (2c) A Failed edge that never delivers the code is rejected, while an arm that binds and ignores it is accepted: NOT APPROVED. See the P1 finding.

## Findings

[P1] [contract-semantics] bench/fixtures/sley2_live_judge.py:1071-1077,810-812; bench/live/TYPE-FIXTURE-REVIEW-PACKET.md:82,124-129; bench/live/succ_witness_type_full.py:42-43,118-119; bench/live/tests/test_judge_type_variant.py:235-239 - Predicate D3 rejects a Failed case edge that carries no CasePayload with ORACLE_FAILED_CODE, even though production validates it (`trial_type_neg_dropcode.log`: compose valid True). That design behaves exactly like the accepted `alt_failed_fixed`: both return 3 for Failed (`succ_witness_type_full.py:280-283`), and the two differ only in whether the unused code is bound. The frozen text's "Failed carries an explicit error code" / "null error" governs the variant type and its values, which A2/A3 already enforce (`:983-1010`). Nothing in tasks.json S2B-TYPE-001 requires a switch arm to consume the code. The frozen S3 reference switch discards it: `JobState::Failed(_) => "failed"` (`s3_g1_type.rs:199`). `neg_dropcode` is the direct IR transliteration of that arm. So D3 is a consumer-shape pin of the same class as the retired "Failed leaf returns its own block param" pin. It rejects correct exhaustive migrations for a reason the agent is never told, which is a semantic oracle narrowing under `mutation_rule` (packet §1 lines 35-38 adopts that same standard). My prior closure list said "payload carried"; the only derivable reading is that the Failed *value* carries its code, and A3 enforces that. - Closure evidence: remove the D3 rejection, or reduce it to a non-verdict observation; turn `neg_dropcode` into an accepted alternative with a fresh provenance log that shows the executed values; turn the unit test at `:235-239` into an acceptance; update the judge docstring, packet §3 D3 and the interpretation boundaries, and the coverage row. Alternatively, record D3 in a new corpus version per mutation_rule.

## Assessment

Revision 2 closes all eight prior findings, and I verified each closure against code, tests and logs rather than packet assertions:
- The four witness-shape pins are gone, and live accepted runs cover non-SInt results, arithmetic arms, a mapped Failed code and a join block.
- The parallel-Bool scan now covers fresh and target bindings, and the named role turns the two-parameter case into a rejection.
- The strict oracle's execution half runs the candidate switch over all four members twice through the frozen driver, and the additive variant-input support is tested.
- Every log is tied by sha256 to the judge at HEAD.
- The closure and manifest change is minimal: one role line, with the pack bytes and digest unchanged and the v1 manifest preserved.

I approve the two base-proxy restatements (1a, 1b) and interpretation boundaries 2a and 2b as derivable from the frozen text. One residual narrowing remains in boundary 2c: the judge distinguishes two behaviourally identical Failed arms by a syntactic binding that the frozen text does not require and the frozen S3 reference switch does not use. That keeps the judge from being "narrows nothing beyond the frozen text" as packet §4 claims. The fixture is therefore not yet adoptable to main. The fix is small and contained: remove D3's rejection path and re-prove `neg_dropcode` as accepted.

Two disclosed boundaries I accept without a finding: transient operation results and fresh helper-function result types are outside the Bool scan (packet lines 130-133), and determinism is checked as two runs within one judge invocation (lines 137-139).

VERDICT: REVISE_0_P0_1_P1_0_P2_0_P3_0_P4_PRIOR_P3_P4_CLOSED
SECTION: type_fixture_correction
FIELD: ariadne_contract_review_revision_2
SCOPE_SHA: d3de686b0019ccbde58939567ae34a899a2be894
FINDINGS: [P1] [contract-semantics] bench/fixtures/sley2_live_judge.py:1071-1077,810-812; bench/live/TYPE-FIXTURE-REVIEW-PACKET.md:82,124-129; bench/live/succ_witness_type_full.py:42-43,118-119; bench/live/tests/test_judge_type_variant.py:235-239 - D3 rejects a production-valid Failed edge that carries no CasePayload (neg_dropcode), which behaves identically to the accepted alt_failed_fixed (both return 3); the frozen text requires only that the Failed value carry an explicit code (already enforced by A2/A3), and the frozen S3 reference switch discards it (s3_g1_type.rs:199 `JobState::Failed(_)`), so D3 is a consumer-shape pin that narrows the oracle under mutation_rule - remove or demote the D3 rejection, re-prove neg_dropcode as accepted with a fresh provenance log, flip the unit test, and update the docstring, packet §3 and the coverage row (or version the corpus)
SUMMARY: All eight findings of the 2c97c32 review are verifiably closed. The pins are retired with live acceptances, the Bool scan and switch_param role are in place, the four-member execution half runs twice through the frozen driver, the logs carry provenance and are sha-bound to the HEAD judge, and 49 unit tests plus the emit, driver and S3 cargo gates pass. Rulings 1a, 1b, 2a and 2b are approved as derivable from the frozen text. Ruling 2c is not approved: D3 rejects a Failed arm that ignores the error code without binding it, which behaves identically to an accepted design and matches the frozen S3 `Failed(_)` arm, so it is a residual oracle narrowing. REVISE before main adoption; the fix is confined to D3 and its evidence.
