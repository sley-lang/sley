<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: vulcan; field: vulcan_surface_review_revision_7; dispatched: 2026-09-23T11:55:14Z; duration_s: 647; process_exit_code: 0 -->
# Vulcan Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

I am reviewing independently; I am not the author. HEAD matches the scope and the reviewed tree is unchanged: I edited, created and committed nothing in it. Three untracked `evidence/review/verdicts/**/ariadne_*-2b0f1c9.md` files appeared during my session. Another review lane wrote them, not me. My probes ran in a throwaway directory outside the tree (`/home/dev/.cache/vr7`), which I removed at the end.

**Git commands**
- `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, which matches the scope.
- `git diff --stat f0738119..HEAD -- bench/`: 25 files, +774/−122.
- `git log --oneline f0738119..HEAD`: 13 commits, from `1a32ceca` to `2b0f1c9f`.
- Full `git diff f0738119..HEAD` of:
  - `bench/fixtures/sley2_live_judge.py`
  - all 13 `bench/live/succ_witness_*.py`
  - `bench/live/prove_merge_production.py` and `bench/live/witness_provenance.py`
  - the four `succ-trials-20260923/*.log` files and `SUCCESSION-COVERAGE.md`
- `git diff --stat f0738119..HEAD -- crates Cargo.*`: 5 files, including `index_cache.rs`. These are outside the bench scope; I note them only because they affect where the test binary came from.
- `git status --short`: the only entries are the three untracked Ariadne files above.

**Tests and probes** (Python 3.14.7; `shutil.rmtree.avoids_symlink_attacks` is True)
- `python3 -m unittest bench.live.tests.test_scratch_cleanup -v` with `TMPDIR=/home/dev/.cache/vr7`:
  - Without a binary: exit 0, "Ran 7 tests … OK (skipped=1)". It emitted `ResourceWarning: subprocess … is still running`, which feeds finding 1.
  - With `SLEY2_SLEY_BINARY=/home/dev/Work/checkpoints/sley2-review-target/debug/sley`: exit 0, "Ran 7 tests … OK".
  - No `sley2-*` leftovers after either run.
  - I did not verify that this binary was built from HEAD; crates changed in the range.
- `python3 -m unittest bench.live.tests.test_witness_provenance -v`: exit 0, "Ran 4 tests … OK".
- Heredoc probes of `remove_scratch` (results under Evidence checked).
- The judge harness-error scenario run against the f0738119 judge (loaded via `git show` + exec) and against the HEAD judge.
- A simulated slow `sley serve` wrapper: it delegates the offer calls to the real binary and patches `SESSION_TIMEOUT` to 1.
- A git-fixture check of `source_identity`.
- I ran no `scripts/` checkers; none bear on this bench delta.

**Files read**
- `bench/live/scratch.py:1-116` (the whole file)
- `bench/live/tests/test_scratch_cleanup.py:1-152`
- `bench/live/tests/test_witness_provenance.py:1-58`
- `bench/fixtures/sley2_live_judge.py`: 70-129, 634-690, 1095-1231, 1455-1509, 2636-2724, 2908-2924, 3025-3106
- `bench/live/sley2_tool.py`: 269-328, 452-459
- `bench/sley2/runner.py`: 346-422, 448-473
- `bench/live/prove_merge_production.py`: 120-219, 296-330
- `bench/live/confined.py`: 1-40 and 76-105
- `bench/live/mediated_client.py`: 970-1004
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: 603-782

## Evidence checked

**Can removal be steered outside the scratch root?**
- **Symlinks inside the tree:** links pointing outside, placed in 0500 and 0000 directories, were unlinked and never followed. Outside modes and contents were unchanged.
- **Root is a symlink:** only the link was removed; the target was intact.
- **Path traversal:** failing paths are built from `scandir` entry names, so `..` and `/` cannot appear.
- **Mode handling:**
  - A 40-level chain of 0500 directories with 0400 files was removed.
  - An unlistable root was removed.
  - Unlistable directories nested 1 or 2 deep were removed; 3 deep raised `ScratchRemovalError`. That is loud and bounded, with no unbounded recursion.
- **Permissions changed outside the tree** (finding 3):
  - The parent of a root was widened from 0o500 to 0o700 and not restored.
  - A hard link inside the tree widened an outside file's inode from 0o400 to 0o600.
  - Nothing in the current code can plant such a link. The judge copies the trial with `copytree`, which makes fresh inodes, and staging copies files. Sley's own `fs::hard_link` calls link stage→final inside one repository (`crates/sley-store/src/lib.rs:298`, `crates/sley-txn/src/repository.rs:3019,3842`, `crates/sley-repo/src/refs.rs:2439`).
- **Race:** I simulated a swap of `ro` for a symlink between the rmtree failure and the retry. `remove_scratch` deleted an outside file and returned normally (finding 4). No concurrent writer exists today:
  - Agents run under bwrap with `--unshare-all`, `--die-with-parent`, a private `--tmpfs /tmp`, and only their own scratch bind-mounted writable (`confined.py:89-105`).
  - The judge's copy is a 0700 `mkdtemp` directory.

**Does every path clean up?**
- **Judge `_main`** (`sley2_live_judge.py:1195-1229`): `copytree`, connect and the flows all sit inside the outer `try`, and `_remove_workdir` runs in the `finally`. It runs before `_emit(accepted)`, so a removal failure can never report "accepted".
- **Precedence:** a removal failure replaces a rejection or a harness error with `harness_error` (exit 2). That fails closed.
- **Old vs new judge:** the f0738119 judge left `sley2-judge-6eeikpg4` on the harness-error path; the HEAD judge left nothing. This matches gate record §12.6.
- **Helpers:**
  - `_staged_session` (`:674-686`) removes its workdir on any `BaseException`.
  - The corrupt helper (`:1469-1509`) and both helper cleanups use `_remove_workdir`.
  - `_judge_merge` (`:2710-2723`) runs every cleanup, still ignores session-close errors, and re-raises the first removal failure.
- **Witnesses:** every witness entrypoint runs `main()` under `scratch_root`, so the witness workspace and the judge's copies land under one private root. The body's exception still propagates, and `TMPDIR` is restored (tested).
- **Exceptions:**
  - `Endpoint.close` reaps its child (`runner.py:406-422`).
  - `Session.__init__` never closes its `Endpoint` on failure (`sley2_tool.py:286-306`), which leads to finding 1.
  - With the simulated slow `serve`, the judge returned `harness_error` with zero leftovers. 4.5 s later, `sley2-judge-*/ws/repo/objects/zz` had been recreated after the `lexists` check.

**Is a removal failure a loud harness error?**
- In the judge it becomes `LIVE_SLEY2_JUDGE_INVALID: scratch removal: …`, covered by the test that mocks `remove_scratch`.
- In witnesses and the merge prover it surfaces as an uncaught `ScratchRemovalError` (exit 1).
- The whole-tree `lexists` check (`scratch.py:92-93`) catches any residue at the time of the check.

**Other delta items**
- `witness_provenance.py` names the dirty code paths and returns "source unknown" when git fails. My fixture reported the dirty code path correctly.
- The rewritten judge docstrings (`:3121-3148`, `:3383-3396`) match `_ContinuationLedger` (`:3025-3106`): scope is the whole trial, keyed by (query, cursor), `after` must equal the previous page's `next`, and a page still open at trial end rejects.
- The committed witness logs are still the c21ff945 re-runs and read "workspace kept at". Gate record §12.4 correctly calls the d307a22f CONTEXT pos run an untracked log.

## Findings

[P3] [cleanup-race] bench/live/sley2_tool.py:286-306 with bench/fixtures/sley2_live_judge.py:1195-1229,674-686 - If a Session constructor fails after `Endpoint(...)` has spawned `sley serve` (hello/open timeout, negotiation failure, seed-import timeout in `_staged_session`), the child is never closed or reaped, so `_remove_workdir` runs while that child is still alive. The unit run shows `ResourceWarning: subprocess … is still running`. With a simulated slow `serve` and `SESSION_TIMEOUT=1`, the judge reported harness_error with no leftovers, then the orphaned child recreated `sley2-judge-*/ws/repo/objects/zz` after the final `lexists` check. The result is a silent leak plus an orphaned process. The real store writes with `create_dir_all`. The judgment still fails closed; only the "removed on every path" guarantee breaks. Session.__init__ is pre-existing code outside the diff. - Closure evidence: the Endpoint is closed and reaped on any Session.__init__ failure (or the judge reaps it before removal), plus a test with a stalling binary asserting the child has exited and no `sley2-*` reappears after a grace period.

[P3] [cleanup-gap] bench/live/prove_merge_production.py:127-145,200-209,317-325 - Side stages join `stages` only after all three `_read_live_state` calls succeed, and `_read_live_state` never removes its own `sley2-merge-side-*` stage when Session or dispatch fails. The prover does not run under `scratch_root`, so a failed second or third side read leaks every earlier stage plus the failing one into the operator's TMPDIR. A `remove_scratch(workdir)` failure also skips the stage loop. - Closure evidence: register each stage as it is created (or run main under `scratch_root`), plus a test that forces a side-read failure and asserts no `sley2-merge-*` directory survives.

[P4] [scope] bench/live/scratch.py:54-55,87 - Permission restoration reaches outside the tree. `_grant(root.parent)` widens the root's parent (probe: 0o500→0o700, not restored). `_grant(path)` chmods failing non-directory entries, although unlinking only needs the parent's w+x. Through a hard link this widens an outside inode (probe: 0o400→0o600). Current callers cannot reach it: the judge's copies are fresh inodes, sley's hard links stay inside one repository, and the parents are operator TMPDIRs. - Closure evidence: grant only on directories inside the tree, leave `root.parent` alone or restore it, and add a hard-link regression test asserting outside modes are unchanged.

[P4] [toctou] bench/live/scratch.py:32-65 - The retry is path-based: `lstat` then `os.chmod` (which follows a final-component symlink), then `os.unlink` or `shutil.rmtree` on the re-resolved full path. This gives up rmtree's fd-based symlink protection. A simulated directory→symlink swap between failure and retry deleted an outside file, and `remove_scratch` returned normally. Nothing can exploit this today, because no untrusted writer runs concurrently with harness-private trees (bwrap confinement, 0700 mkdtemp). - Closure evidence: an fd-relative retry (`dir_fd` + `O_NOFOLLOW`, `fchmod`/`unlinkat`), or a module docstring precondition that callers pass only trees with no concurrent writers.

[P4] [evidence-wording] bench/live/succ_witness_context.py:101,114,119,126 (same pattern in all 13 witnesses) - The log line "workspace: … (removed at exit)" is written, and the log file saved, before `scratch_root` runs the removal. A removal failure then shows only as a traceback and exit 1, while the saved log reads like a clean run. - Closure evidence: append a removal-outcome line after `scratch_root` exits, or word the line as intent rather than an observed result.

## Assessment

The delta fixes the leak it targets. The HEAD judge removes its `sley2-judge-*` copy on accept, reject and harness-error paths, where the f0738119 judge leaked it in the same scenario.

A removal failure can never turn into an acceptance. In the judge it is a loud `LIVE_SLEY2_JUDGE_INVALID: scratch removal` harness error, and in the scripts it is an uncaught exception. The whole-tree `lexists` check stops quiet residue at the time of the check.

Symlinks inside the tree, a symlink root, and path traversal cannot steer removal outside the root. The helper does change permissions outside the tree in two cases (the root's parent, and hard-linked inodes), and its path-based retry is not race-safe against a concurrent writer. The current callers expose neither to hostile input, because agents are bwrap-confined with a private `/tmp` and the judge copies into fresh inodes. Both are recorded as P4.

The real residual gaps are two leak paths:
- a Session constructor failure leaves a live `sley serve` child that can recreate the tree after the check (P3);
- the manual merge prover leaks side stages on failure (P3).

Neither affects any judgment. With no P0–P2 findings, the delta is accepted.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_3_P4
SECTION: sley2_trial_runner
FIELD: vulcan_surface_review_revision_7
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P3] [cleanup-race] bench/live/sley2_tool.py:286-306 with bench/fixtures/sley2_live_judge.py:1195-1229,674-686 - Session constructor failure leaves the sley serve child unreaped while the scratch is removed; a simulated slow serve recreated sley2-judge-*/ws/repo/objects/zz after the lexists check (silent leak, orphaned process; judgment still harness_error) - close/reap the Endpoint on any Session.__init__ failure plus a stalling-binary test asserting child exit and no sley2-* reappearance; [P3] [cleanup-gap] bench/live/prove_merge_production.py:127-145,200-209,317-325 - side stages registered only after all three _read_live_state calls succeed, and _read_live_state never removes its own stage on failure; the prover is not under scratch_root, so failure paths leak sley2-merge-side-* and a workdir removal failure skips the stage loop - register stages as created or use scratch_root, plus a forced side-read-failure test with no sley2-merge-* left; [P4] [scope] bench/live/scratch.py:54-55,87 - _grant widens the root's parent (0o500->0o700, not restored) and chmods failing non-directories, widening hard-linked outside inodes (0o400->0o600); unreachable by current callers - grant only in-tree directories, leave root.parent alone, add a hard-link regression test; [P4] [toctou] bench/live/scratch.py:32-65 - path-based retry gives up rmtree's fd-based symlink protection; a simulated dir->symlink swap deleted an outside file while remove_scratch returned normally; no concurrent untrusted writer exists today - fd-relative retry or a documented no-concurrent-writer precondition; [P4] [evidence-wording] bench/live/succ_witness_context.py:101,114,119,126 (all 13 witnesses) - "(removed at exit)" is logged and the log saved before scratch_root removal runs; a removal failure shows only as exit 1 - append a removal-outcome line after scratch_root exits or word the line as intent
SUMMARY: The scratch-workspace fix works: the judge now removes its copy on accept, reject and harness-error paths (the f0738119 judge leaked sley2-judge-* in the same scenario), and every removal failure is a loud harness error or uncaught exception that can never yield an acceptance. Symlinks inside the tree, a symlink root and path traversal cannot steer removal outside the root; permission changes outside the tree and a path-based retry race exist but current callers cannot reach them (P4). Two leak paths remain: a Session constructor failure leaves a live sley serve child that can recreate the scratch after the final check, and the merge prover leaks side stages on failure (P3 each). With no P0-P2 findings, the delta is accepted.
