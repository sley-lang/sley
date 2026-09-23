<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9; role: ariadne; field: ariadne_contract_review_revision_8; dispatched: 2026-09-23T14:33:54Z; duration_s: 660; process_exit_code: 0 -->
# Ariadne Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9

What I verified myself. I made no tracked change to the tree; at the end `git rev-parse HEAD` was still 03b25eb9 and `git diff --stat HEAD` was empty. Side effects:
- `cargo test` rebuilt the sley-protocol lib-test artifact under the preset `CARGO_TARGET_DIR`.
- Python wrote gitignored `__pycache__` files.
- Three scratch directories under `~/.cache` (`ar8-clone-*`, `ar8-live-*`, `ar8-revert-*`) were created and removed; each removal was confirmed. The side commit in the clone probe was written only inside that scratch clone.

During the session, five untracked verdict files from other concurrent dispatches appeared under `evidence/review/verdicts/`. They are not mine and I did not read them.

**Scope and history**
- `git rev-parse HEAD` returned 03b25eb9b3d7e80576394109ca4b73cc21b2fab9, which matches the scope.
- `git log --oneline 26d050e6..HEAD`: 8 commits. `git diff --stat 26d050e6..HEAD`: 41 files.
- Full diffs read:
  - `docs/spec/SLEY2_TRIAL_RUNNER_V1.md`, `scripts/check_sley2_trial_runner.py`, `scripts/test_sley2_trial_runner.py`
  - `crates/sley-protocol/src/server.rs` and `docs/spec/SMP1.md`
  - ADR-0036, the S20-620 closeout, WORK_PACKAGES, and gate record §14
  - the machine-summary `sley2_trial_runner` section, key by key against 26d050e6
- `git log --all -- docs/spec/SLEY2_TRIAL_RUNNER_V1.md`: 17 commits, all ancestors of HEAD. Only 6519f6e0 reads revision 8.
- `git merge-base --is-ancestor`: every historical review scope (5b538f3, f073811, 2b0f1c9, 26d050e) and 60be11c8 is an ancestor of HEAD.

**Checkers** (python3 subprocess; exit codes captured)

| Checker | Exit | Result |
|---|---|---|
| `check_sley2_trial_runner.py` | 0 | PASS, revision 8, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, problems [] |
| `test_sley2_trial_runner.py -v` | 0 | 7 tests OK (4 ComposedPins, 3 CompletionScopeBinding) |
| `check_smp1_contract.py` | 0 | PASS, revision 16 |
| `test_smp1_contract.py` | 0 | 18 tests OK |
| `check_complete_root_index_snapshot_profile.py` | 0 | PASS, revision 6 |
| `check_finding_register.py` | 0 | PASS |

**Revert run.** I ran HEAD's `test_sley2_trial_runner.py` against the 26d050e checker in a `git archive` scratch tree. It exited 1: one ERROR (`scope_bound_problems` is missing) and two FAILs. Every new completion-binding test fails on the old code.

**Cargo**
- `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 5 passed.
- `cargo test --locked --offline -p sley-cli --test cli workspace_open`: 0 matched, no recompile, so the `sley` binary is fresh for HEAD.

**Live probe (absent boundary).**
- Setup: that binary, a `sley2_tool.Session` over a `~/.cache` workspace seeded with `S2B-CONTEXT-001/base.pack`.
- With the boundary present, `workspace.open` answered failed=False, 212 bytes.
- I deleted `repo/locks/maintenance.lock`. The next `workspace.open` answered failed=False, 212 bytes, byte-identical to the first, and the lock was re-created.

**Gate probes.** In memory, I monkeypatched `CHECKER.read` for the summary and, in probe H, for the spec. I set status `S20_620_COMPLETE` and `implementation_complete` true, then ran the full `main()`:

| Probe | Result |
|---|---|
| A: all three `_revision_8` PASS scoped `on 6519f6e0…` | exit 0, [] |
| B: all three scoped `on 03b25eb9…` | exit 0, [] |
| C: Vulcan scoped `on 26d050e…` | exit 1, `completion-scope-mismatch:vulcan_surface_review:reviewed-revision-7` |
| D: Ariadne note names HEAD's tree object `400a8412…`, which is not a commit | **exit 0, []** |
| E: uppercase SHA | exit 1, `completion-unscoped-review` |
| F: my round-8 probe replayed at revision 8 (only Ariadne and Nabu `_revision_8`) | exit 1, `completion-unbound-review:vulcan_surface_review` |
| G: note "re-filed on \<HEAD\>; the review ran on 26d050e…" | **exit 0, []** |
| H: HEAD spec reverted in memory to the revision-7 false clause, Status still revision 8, verdicts on 6519f6e0 | **exit 0, []** |
| Scratch clone: side commit off 26d050e, not an ancestor of HEAD, spec blob ≠ HEAD's (the revision-7 text with only Status bumped to 8) | `scope_bound_problems` returns **[]** |

**Files read** (line ranges)
- Transcripts: my `ariadne_contract_review_revision_7-26d050e.md` (1-180); the finding lines of `nabu_architecture_review_revision_7-26d050e.md` (107-111, 136-140) and `vulcan_surface_review_revision_7-26d050e.md` (122-124, 140-144).
- Round indices: the `sley2_trial_runner` entries of `context-r8-26d050e.json` and `context-r7-2b0f1c9.json`. All four transcript sha256s match.
- Spec: `SLEY2_TRIAL_RUNNER_V1.md` 1-40 and 310-384.
- Checkers and tests:
  - `check_sley2_trial_runner.py` 1-60 and 140-381
  - `test_sley2_trial_runner.py` 1-32 plus the added 53-137
  - `build_finding_register.py` 160-219 and 385-414
  - Makefile 84, 109, 209
- Rust:
  - `server.rs` 1170-1204, 2370-2484, 2912-2980
  - `sley-txn/src/maintenance.rs` 40-110
  - a grep of `gc.rs` `lock_path` (it removes only the GC lock, never `maintenance.lock`)
- Bench: `bench/live/sley2_tool.py` 289-458.
- SMP1 diff: Status line, history 72-102, appendix A 761-778.
- Records:
  - gate record 965-1010
  - finding register `sley2_trial_runner` obligations (7646-7664), open_reviews, superseded_rounds (10331-10335), unclaimed_carried_findings

## Evidence checked

- **Revision 8 text.**
  - The Status line (:3) says revision 8.
  - The history sentence (:21-34) names per-lane revision-6 scopes (Nabu and Vulcan f073811, Ariadne 2b0f1c9), which match the `_revision_6` notes' scopes. It also names the round-7 Vulcan delta verdict under `vulcan_surface_review_delta_2b0f1c9` and the revision-7 round outcome at 26d050e (Vulcan PASS; Ariadne and Nabu REVISE), which matches the r8 index.
  - The SMP1 pins at :56 and :332 read 16, which equals `SMP1.md:3`. The S20-300 pin (:335) reads 6.
  - The nineteen names (:317-323) are unchanged.
- **§9 absent-boundary sentence (:338-342).**
  - The code path: `workspace.open` → `session_check_mixed_retained` (server.rs:1184-1186, 2410-2419) → `head_binding_mixed` → `head_mixed` (:2375-2376, 2391-2392) → `maintenance()` (:2469-2473). `maintenance()` runs `initialize_repository_maintenance`, then takes the shared lock, blocking. The probe then takes it shared without waiting and never initializes it (:2965-2973).
  - No production path deletes `maintenance.lock`.
  - The live probe matches the text.
  - SMP1 r16 appendix A and the docstring at server.rs:2931-2935 say the same.
- **Completion gate.**
  - `scope_bound_problems` (checker :171-198) is wired at :356-365.
  - It fails closed on a missing or non-PASS field, a missing `on <40-hex>`, a `git show` failure, or a Status revision ≠ N.
  - A correctly scoped PASS is admitted (probes A and B), in the note format every current `_revision_6` and `_revision_7` note uses.
  - It does not check ancestry, object type, the cited transcript or round index, or the text itself (probes D, G, H and the side commit): finding 1.
- **Records.**
  - Summary: `contract_revision` 8, status `…REVIEW_PENDING`, `implementation_complete` false.
  - `vulcan_surface_review_delta_2b0f1c9` holds the old value byte-for-byte and embeds its original note.
  - `vulcan_surface_review_revision_7` now equals the 26d050e transcript's VERDICT.
  - The Ariadne and Nabu `_revision_7` REVISE values equal their transcripts.
  - `p4_open` carries Nabu's two P4s.
  - ADR-0036, the closeout and WORK_PACKAGES agree that revision 8 is pending review.
  - The round-7 transcript and its index are unchanged, so history is preserved.
  - The register side effect of the rename is finding 3.

### Round-8 findings of this lane (ariadne_contract_review_revision_7-26d050e)

- **[P2] [completion-binding] `scripts/check_sley2_trial_runner.py:324-331` (field-name-only binding): CLOSED.**
  - The 2b0f1c9 value moved, byte-exact, to a non-binding name.
  - Under the revision-7 name it is now refused as `reviewed-revision-6` (test :94-108).
  - My probe, replayed at revision 8, fails `completion-unbound-review:vulcan_surface_review` (F).
  - The new tests fail on the 26d050e checker.
  - A residual weakness in the new binding is filed as finding 1.
- **[P3] [spec-conformance] `docs/spec/SLEY2_TRIAL_RUNNER_V1.md:330-332` (false "fails the method" clause): CLOSED.**
  - The corrected text is at :338-342, SMP1 r16 appendix A and server.rs:2931-2935.
  - Code trace and a live probe at HEAD agree.
  - The server test in my closure evidence was not added. That residual is filed as finding 2.
- **[P4] [record] `docs/spec/SLEY2_TRIAL_RUNNER_V1.md:21-22` (one scope for all three revision-6 PASSes): CLOSED.**
  - :21-26 names the per-lane scopes and the delta verdict under its non-binding field name.

## Findings

[P3] [completion-binding] scripts/check_sley2_trial_runner.py:171-198 (with docs/spec/SLEY2_TRIAL_RUNNER_V1.md:31-33, docs/adr/ADR-0036-sley2-trial-runner-boundary.md:9-10) - The gate binds a `_revision_N` PASS to a revision number, taken from the first `on <40-hex>` in a free-text note. It does not bind to the text reviewed: it checks no ancestry, no object type, no text identity, and not the transcript or round index the note itself cites. As a result, mis-scoped PASSes satisfy it: (a) a side commit off 26d050e whose spec is the revision-7 text, false clause included, with only Status bumped to 8 (not an ancestor of HEAD, blob ≠ HEAD's) → `scope_bound_problems` returns []; (b) a note naming HEAD's tree object 400a8412… (not a commit) → full main() exit 0 PASS; (c) a note "re-filed on <HEAD>; the review ran on 26d050e…" → exit 0 PASS; (d) HEAD's revision-8 text reverted in memory to the revision-7 clause, verdicts on 6519f6e0 → exit 0 PASS. ADR-0036 ("verdicts scoped to the reviewed text") and the docstring ("to the text it reviewed") claim more than the check does. - Closure evidence: (1) require the named object to be a commit (`git cat-file -t`) that is an ancestor of HEAD; every historical review scope is one, so this is feasible. (2) Resolve the scope from the round index or transcript the note cites (sha256-matched SCOPE_SHA and FIELD), not from free text. (3) Either refuse contract edits after the scope beyond the Status and completion lines, or say in the spec that the revision number is the binding and gate that every contract text change bumps it. (4) Add tests for non-ancestor, non-commit and multi-scope notes.

[P4] [test-coverage] crates/sley-protocol/src/server_tests.rs (no test; with docs/spec/SLEY2_TRIAL_RUNNER_V1.md:338-342, crates/sley-protocol/src/server.rs:2469-2473,2931-2935, bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:983) - §9 and SMP1 r16 now assert that the session check creates an absent boundary, so it never reaches the probe, but no test pins this. No test under crates/ removes `locks/maintenance.lock` before `workspace.open`, and server.rs changed only in its docstring. Both this lane's and Vulcan's round-8 P3 closure evidence asked for such a test, and the gate record marks the finding FIXED without one. Dropping `initialize_repository_maintenance` from `maintenance()` would make the contract sentence false again with every test green. - Closure evidence: a sley-protocol server test that removes the lock between two `workspace.open` requests and asserts success, byte-identical bodies and a re-created lock (optionally also the TXN_IO 39019 path when initialization fails).

[P4] [record] evidence/review/finding-register.json:7646-7664,10331-10335 (with scripts/build_finding_register.py:183-207, bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:969-976,990,1001-1007) - The rename to `vulcan_surface_review_delta_2b0f1c9` changed the register's classification, and gate record §14 does not disclose it. Because `supersedes` does not treat `_delta_` as an early-round token, the delta PASS now marks Vulcan's revision-5 REVISE (5b538f3) HISTORICAL_ROUND with `superseded_by: vulcan_surface_review_delta_2b0f1c9`. At 26d050e that REVISE was an open review, and the Ariadne and Nabu revision-5 REVISEs still are. The closure is right in substance (Vulcan r6 PASS at f073811 closed those findings), but it is credited to a verdict on the leak fix and the revision-6 text. §14.1 reports only the move and §14.4 describes the opposite rule, so this change to the "40 open reviews" count is unexplained. - Closure evidence: record the side effect in the gate record, or (S20-740 lane) have the register credit the fold to the actual closing verdict, or not fold under `_delta_<scope>` fields.

## Assessment

Revision 8 does what it says.
- **Completion gate.** Name-only binding is gone. The historical verdict is preserved under a name that cannot bind, and the reviewers' probe is a regression test that fails on the old code.
- **Absent-boundary text.** The false clause is corrected consistently in §9, SMP1 appendix A and the docstring, and it now matches both the code path and a live probe.
- **History.** The history sentence attributes scopes correctly.
- **Runner and records.** The runner, the pins and the frozen allowlist are unchanged and the checkers are green. The records agree, and ADR-0036, the closeout, WORK_PACKAGES and the summary say revision 8 is pending review.

All three of this lane's round-8 findings are closed.

On the dispatch's request, I did satisfy the new gate with mis-scoped PASSes: a non-ancestor side commit, a tree object, a note that names the wrong commit first, and a same-revision text drift. The binding holds only under honest notes and the revision-bump convention (P3). That is a hardening gap, not a present false completion: no current record exploits it, and the package is still REVIEW_PENDING. Two records need attention: the new behavior sentence has no pinning test, and the rename's register side effect is undisclosed (P4 each). None of these blocks acceptance of the revision-8 contract text.

This verdict covers the trial-runner contract only. It does not accept SMP1 revision 16 or the S20-300 completion binding, which are reviewed in their own lanes. I make no GA, release-readiness or live-trial claim.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_2_P4_PRIOR_P3_P4_CLOSED
SECTION: sley2_trial_runner
FIELD: ariadne_contract_review_revision_8
SCOPE_SHA: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9
FINDINGS: [P3] [completion-binding] scripts/check_sley2_trial_runner.py:171-198 (with SLEY2_TRIAL_RUNNER_V1.md:31-33, ADR-0036:9-10) - the gate binds a _revision_N PASS to a revision number from the first `on <40-hex>` in a free-text note, not to the reviewed text: no ancestry, object-type, text-identity or cited transcript/round-index check; probes pass a non-ancestor side commit carrying the revision-7 text with Status 8, HEAD's tree SHA, a note naming HEAD first though the review ran on 26d050e, and an in-memory revision-8 text reverted to the revision-7 clause - require a commit that is an ancestor of HEAD, resolve the scope from the cited round index/transcript, refuse post-scope text edits or gate revision bumps on every text change, and add tests for these cases | [P4] [test-coverage] crates/sley-protocol/src/server_tests.rs (no test; with SLEY2_TRIAL_RUNNER_V1.md:338-342, server.rs:2469-2473,2931-2935, gate record:983) - the corrected absent-boundary behavior (session check creates the boundary; workspace.open succeeds) is pinned by no test though both round-8 P3 closures asked for one - a server test removing maintenance.lock between two workspace.open requests asserting success, identical bodies and a re-created lock | [P4] [record] evidence/review/finding-register.json:7646-7664,10331-10335 (with build_finding_register.py:183-207, gate record:969-976,990,1001-1007) - the rename to vulcan_surface_review_delta_2b0f1c9 makes the register fold Vulcan's revision-5 REVISE as superseded by the leak-fix delta verdict (open at 26d050e; the Ariadne and Nabu r5 REVISEs stay open), undisclosed in gate record §14 - record the side effect, or credit the fold to the actual closing verdict or not fold under _delta_ fields (S20-740 lane)
SUMMARY: Revision 8 closes this lane's round-8 P2, P3 and P4: the stale Vulcan verdict can no longer satisfy the gate, the absent-boundary sentence matches the code and a live probe at HEAD, and the history names per-lane scopes; checkers and the new tests pass, and those tests fail against the 26d050e checker. The new binding is by revision number through a free-text note, so a mis-scoped PASS still passes via a non-ancestor commit, a tree object, a misleading note or a same-revision text drift (P3). The corrected boundary behavior has no pinning server test, and the rename silently changed the register's supersession of Vulcan's revision-5 REVISE (P4 each). This lane accepts contract revision 8.
