<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: vulcan; field: vulcan_surface_review_revision_7; dispatched: 2026-09-23T14:01:23Z; duration_s: 788; process_exit_code: 0 -->
# Vulcan Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

I reviewed this package independently; I am not the author. HEAD matched the scope and I left the tree unchanged: no edits, creates or commits, and `git status` shows no tracked changes. Other review lanes wrote untracked `evidence/review/verdicts/**-26d050e.md` files during my session, including Ariadne and Nabu for this package. I did not read them. My probes ran in a throwaway directory outside the tree (`~/.cache/vr8`), which I removed at the end.

**Side effect I caused.** While running the new tests against the old code, a cleanup loop of mine SIGKILLed every process whose `/proc/*/cmdline` contained `sley2-stall-`:
- pid 2254340: the old tool's orphaned `sleep 30` stall child. This was expected.
- pid 2254334: most likely my own tool shell, whose command line carried the heredoc text.
- pid 2248567: I did not knowingly spawn it and could not identify it afterwards. It may have belonged to another concurrent review worker (`review_dispatch.py --workers 5`).

No file in the tree was touched. A later scan found no process carrying the marker.

**Git commands**
- `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, which matches the scope.
- `git log --oneline 2b0f1c9f..HEAD`: 12 commits, from `531805ae` to `26d050e6`.
- `git diff --stat 2b0f1c9f..HEAD`: 86 files.
- Full `git diff 2b0f1c9f..HEAD` of this package's owned paths:
  - `docs/spec/SLEY2_TRIAL_RUNNER_V1.md`, `docs/adr/ADR-0036-…`, `docs/audits/S20_620_…CLOSEOUT.md`
  - `scripts/check_sley2_trial_runner.py`, `scripts/test_sley2_trial_runner.py`, `Makefile`
  - `bench/live/{scratch,sley2_tool,prove_merge_production}.py`, all 13 `succ_witness_*.py`, `bench/live/tests/test_scratch_cleanup.py`, `SUCCESSION-COVERAGE.md`
  - the `sley2_trial_runner` machine-summary hunks and the S20-620 WORK_PACKAGES row
- `git diff --stat 2b0f1c9f..HEAD -- bench/sley2 bench/fixtures`: empty, so the runner code did not change.
- `git show 2b0f1c9f:docs/spec/SLEY2_TRIAL_RUNNER_V1.md`: the Status line reads revision 6.
- `git show 2b0f1c9f:…/machine-summary.json`: `contract_revision` is 6 and there is no `vulcan_surface_review_revision_7` field.
- `git log -S vulcan_surface_review_revision_7`: the field was added in `1d8086fc`.

**Checkers**
- `python3 scripts/check_sley2_trial_runner.py`: exit 0, `"result": "PASS"`, `"revision": 7`, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, `problems: []`.
- `python3 scripts/test_sley2_trial_runner.py -v`: exit 0, "Ran 4 tests … OK".
- `python3 scripts/check_finding_register.py`: exit 0, `"result": "PASS"`.

**Cargo**
- `cargo test --offline --locked -p sley-cli --test cli native_test_worker_entry_runs_the_unit_argv_against_the_real_binary`: 1 passed. It finished in 0.03 s with no recompile, so `$CARGO_TARGET_DIR/debug/sley` matches this tree's sources.

**Tests** (`TMPDIR=~/.cache/vr8`, `PYTHONWARNINGS=default`)
- `bench.live.tests.test_scratch_cleanup` without a binary: exit 0, "Ran 11 … OK (skipped=2)".
- The same suite with `SLEY2_SLEY_BINARY` set: exit 0, "Ran 11 … OK".
- Neither run emitted a ResourceWarning. In round 7 this suite emitted "subprocess … is still running".
- `bench.live.tests.test_witness_provenance`: exit 0, 4 OK.
- `bench/sley2/tests`: exit 0, 23 OK.
- No `sley2-*` leftovers.

**Tests on the old code.** I exec'd the `2b0f1c9f` modules from `git show` into `sys.modules` before importing the test module:
- `StalledServeIsReaped` against the old `sley2_tool`: FAIL, `[2254340] != []`, "serve child reaped".
- `MergeProverStages` against the old prover: FAIL, leftover `sley2-merge-side-cwc2bisu`.
- The hard-link test against the old `scratch.py`: FAIL, `384 != 256`.
- The parent-mode test against the old `scratch.py`: FAIL, `ScratchRemovalError not raised`.

**Files read**
- `evidence/review/verdicts/sley2_trial_runner/vulcan_surface_review_revision_7-2b0f1c9.md:1-120`
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:784-965`
- `bench/live/scratch.py:1-126`
- `bench/live/sley2_tool.py:162-197,255-374,482-497`
- `bench/live/prove_merge_production.py:100-339`
- `bench/live/tests/test_scratch_cleanup.py:1-246`
- `bench/sley2/runner.py:346-474`
- `bench/fixtures/sley2_live_judge.py:425-479,636-690`
- `scripts/check_sley2_trial_runner.py:143-348`
- `docs/spec/SLEY2_TRIAL_RUNNER_V1.md:1-50,316-345`
- `docs/spec/SMP1.md:750-774`
- `crates/sley-protocol/src/server.rs:1170-1219,2375-2473,2915-2971`
- `crates/sley-txn/src/repository.rs:876-905`
- `crates/sley-txn/src/maintenance.rs:40-179`
- `machine-summary.json` `sley2_trial_runner` section (4394-4431 and following)
- `evidence/review/rounds/context-r7-2b0f1c9.json`: the sha256 of both trial-runner transcripts matches the index.

## Evidence checked

**Status of each round-7 finding of this lane**

1. [P3] [cleanup-race] unreaped `sley serve` child on a failed `Session` construction (`bench/live/sley2_tool.py`): CLOSED.
   - `Session.__init__` (`sley2_tool.py:306-314`) wraps `_open` in `except BaseException`, then `_abandon` (`:269-286`) kills, drains or waits with 10 s bounds, and marks the endpoint closed.
   - The new test passes at HEAD and fails on the 2b0f1c9 tool.
   - I injected a failure after the seed import and `session.open` on the real binary. At HEAD the serve child had `returncode -9` and no `/proc` entry right after the raise. On the old tool it was `None`, the `/proc` entry was present, and Python emitted "ResourceWarning: subprocess … is still running".
2. [P3] [cleanup-gap] merge-prover side stages leaked on failure (`bench/live/prove_merge_production.py`): CLOSED.
   - Each stage is appended the moment it is created (`:129-130`).
   - The `finally` attempts every tree and raises the first failure (`:318-333`).
   - `__main__` runs under `scratch_root` (`:336-339`).
   - The new test fails on the old prover (`sley2-merge-side-*` left) and passes at HEAD.
3. [P4] [scope] `_grant` widened the root's parent and hard-linked file modes (`bench/live/scratch.py`): CLOSED.
   - `_grant_directory` (`:33-50`) touches only real directories lexically at or below the root.
   - No parent is granted, no file mode is changed, and symlinks are skipped via `lstat` + `S_ISDIR`.
   - The hard-link test and the parent test both fail on the old code and pass at HEAD.
4. [P4] [toctou] path-based retry gives up rmtree's fd-based symlink safety (`bench/live/scratch.py`): CLOSED.
   - This was closed by the documented-precondition option: the `remove_scratch` docstring (`:84-89`) states the no-concurrent-writer precondition and the path-based residual.
   - I checked every caller: the judge, the 13 witnesses and the prover. Each removes only after its sessions are closed, and a failed constructor now reaps its child (item 1).
5. [P4] [evidence-wording] "(removed at exit)" was logged before removal ran (all 13 witnesses): CLOSED.
   - No `bench/live/*.py` file contains "(removed at exit)" any more. All 13 read "(scheduled for removal at exit; a failed removal exits nonzero)".
   - The entrypoints raise out of `scratch_root`, so a failed removal does exit 1.

**Can the round-7 `vulcan_surface_review_revision_7` field satisfy the revision-7 binding?**
Nothing in the implementation prevents it.
- The completion gate (`check_sley2_trial_runner.py:324-331`) checks only that `section[f"{key}_revision_{spec_revision}"]` starts with "PASS".
- The field was written in `1d8086fc` from the 2b0f1c9 round, where the spec was at revision 6. It now sits under the name the revision-7 gate reads.
- I copied the summary, set status `S20_620_COMPLETE` and `implementation_complete: true`, and added only the Ariadne and Nabu `_revision_7` PASS fields. The checker returned exit 0, `PASS`, `problems: []`.
- Removing the Vulcan field, or setting it to REVISE, returned exit 1 with `completion-unbound-review:vulcan_surface_review`.
- The finding register does not guard this either: its `open_reviews` lists only the revision-5 REVISE fields.
- What prevents a false completion today is procedural: the status is still PENDING, and recording this verdict overwrites the field. Finding 1 below.

**Revision 7 spec delta**
- The composed pins are consistent: SMP1 15 at `:43` and `:324`, and S20-300 6 at `:327`.
- `composed_pin_problems` fails closed when a status line is unreadable or a pin is missing, and it has revert tests. `make quick` now runs `test_sley2_trial_runner.py`.
- The new absent-boundary sentence (`:330-332`) is contradicted by the code and by a live probe:
  - I seeded a repository and deleted `repo/locks/maintenance.lock` between `session.open` and `workspace.open`.
  - `workspace.open` did not fail: it returned a 211-byte body with head tx `5548efbb7107fa5f`, and the lock file was recreated.
  - Finding 2 below.

**Slow-serve judge replay**
- I used a wrapper that sleeps 2 s before `exec`ing the real serve, with `SESSION_TIMEOUT=1`.
- At HEAD the judge returned `harness_error`. There were no leftovers after 5 s and no live process naming the private root.
- The old-tool control also left nothing, because with this wrapper the late serve finds its repository gone. So this replay does not discriminate old from new code. The discriminating evidence for item 1 is the old-code test failure and the injected post-seed probe.

**Residual note (not filed)**
- `_abandon` signals only the direct child. With a wrapper binary that does not `exec`, the real serve is a grandchild and survives the kill.
- `communicate()` then closes its stdin, so it exits at end of input. The wait for that is bounded at 10 s.

## Findings

[P3] [evidence-binding] scripts/check_sley2_trial_runner.py:324-331 with machineresearch/sley-2.0/machine-summary.json:4430-4431 - The completion gate binds a lane verdict to the contract revision by field name only. The pre-existing `vulcan_surface_review_revision_7` PASS (recorded in 1d8086fc from the 2b0f1c9 round, where the spec Status line read revision 6; its note says it reviews the revision 6 text) therefore satisfies the revision-7 binding. Probe: status COMPLETE plus only the Ariadne and Nabu `_revision_7` PASS fields gives checker exit 0 PASS with no problems; removing the field or setting it to REVISE gives `completion-unbound-review:vulcan_surface_review`. Only the PENDING status and the expectation that this review overwrites the field prevent a false completion, and `scripts/test_sley2_trial_runner.py` has no completion-binding case. - Closure evidence: bind each `<lane>_revision_N` field to a round-index entry whose `scope_sha` carries spec Status revision N (or move a value recorded at another revision to a non-binding name in the recording commit), plus a negative test where the pre-existing Vulcan value alone yields `completion-unbound-review`.

[P3] [error-precedence] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:21-26,330-332 with crates/sley-protocol/src/server.rs:2931-2933 - Revision 7 states that "an absent maintenance boundary never reaches the probe: the opener's head load requires the boundary and fails the method first (SMP1 appendix A)". That is false. The `workspace.open` session check (server.rs:1184-1186 → `session_check_mixed_retained` :2410-2419 → `head_mixed` :2391-2392) calls `Server::maintenance()` (:2469-2473). That runs `initialize_repository_maintenance` (crates/sley-txn/src/maintenance.rs:50-100), which creates the missing lock before any head load or probe. Live probe: with `locks/maintenance.lock` deleted before `workspace.open`, the method succeeded (211-byte body) and the lock was recreated. The method fails only when initialization fails (TXN_IO 39019). The server docstring repeats the claim, and the ADR-0036, closeout and WORK_PACKAGES entries call it a correction. The claim is inherited from SMP1.md:763-765. No runner behavior depends on it. - Closure evidence: spec and docstring text saying the session check initializes an absent boundary, as every session-checked read does, so the probe never sees it absent and the method fails only when initialization fails; plus a server test pinning `workspace.open` over a repository whose lock was removed.

## Assessment

Every round-7 finding of this lane is closed with evidence.
- The two P3 fixes and the P4 scope fix each come with tests that I ran against the 2b0f1c9 code: all four fail there and pass at HEAD.
- An injected constructor failure on the real binary shows the serve child killed and reaped at HEAD, where the old tool left it running.
- The TOCTOU P4 is closed by the documented-precondition option, and I verified the precondition against every caller. The witness wording P4 is fixed in all 13 witnesses.
- The removal paths still fail closed: a removal failure is a harness error in the judge and exits nonzero in the scripts.

I found two new issues, both P3.
- **The stale field can satisfy the revision-7 completion binding.** The implementation does not prevent this; I showed the checker would accept a COMPLETE status backed by the revision-6-era Vulcan verdict. This review, recorded into that field, removes the concrete collision. The gate needs a mechanical binding so the next collision is refused.
- **The one semantic sentence revision 7 adds is wrong.** It says an absent boundary fails the method. Actually the session check initializes the boundary, and the method succeeds after writing the lock file into the repository.

No P0 to P2 issue remains in this package's owned paths. The runner code is unchanged and its 23 tests pass.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_0_P4_PRIOR_P3_P4_CLOSED
SECTION: sley2_trial_runner
FIELD: vulcan_surface_review_revision_7
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P3] [evidence-binding] scripts/check_sley2_trial_runner.py:324-331 with machineresearch/sley-2.0/machine-summary.json:4430-4431 - the completion gate binds lane verdicts to the contract revision by field name only, so the pre-existing vulcan_surface_review_revision_7 PASS (recorded in 1d8086fc from the 2b0f1c9 round at spec revision 6; its note says it reviews the revision 6 text) satisfies the revision-7 binding; probe: status COMPLETE plus only the Ariadne and Nabu _revision_7 PASS fields gives checker exit 0 PASS, and removing or REVISE-ing the field gives completion-unbound-review:vulcan_surface_review; only the PENDING status and the expected overwrite prevent a false completion, and no test covers the binding - bind each <lane>_revision_N field to a round-index entry whose scope_sha carries spec revision N (or move a value recorded at another revision to a non-binding name in the recording commit), plus a negative test where the pre-existing Vulcan value alone yields completion-unbound-review; [P3] [error-precedence] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:21-26,330-332 with crates/sley-protocol/src/server.rs:2931-2933 - revision 7 says an absent maintenance boundary fails the method at the opener's head load, but the workspace.open session check (head_mixed -> Server::maintenance(), server.rs:2391-2392,2469-2473) runs initialize_repository_maintenance (maintenance.rs:50-100) first; live probe: with locks/maintenance.lock deleted before workspace.open, the method succeeded (211-byte body) and the lock was recreated; the method fails only when initialization fails; the docstring, ADR-0036, closeout and WORK_PACKAGES repeat the claim, which is inherited from SMP1.md:763-765 - text and docstring saying the session check initializes an absent boundary so the probe never sees it absent, plus a server test pinning workspace.open over a repository whose lock was removed
SUMMARY: All five round-7 findings of this lane are closed. Each code fix has a test that fails on the 2b0f1c9 code and passes at HEAD, and an injected failure on the real binary shows the serve child killed and reaped. Nothing mechanical stops the round-7 Vulcan field from satisfying the revision-7 completion binding; a checker probe accepted COMPLETE with it, so only the PENDING status and this review's overwrite prevent a false completion (P3). The new absent-boundary sentence contradicts the server, which initializes the boundary and succeeds (P3); with no P0-P2 findings, this lane accepts the package at contract revision 7.
