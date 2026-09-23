<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9; role: vulcan; field: vulcan_surface_review_revision_8; dispatched: 2026-09-23T14:33:54Z; duration_s: 547; process_exit_code: 0 -->
# Vulcan Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9

I reviewed this package independently; I am not the author. HEAD matched the scope, and I left the tree unchanged: `git status --short` is empty. My live probes ran in scratch directories under `~/.cache` (`vr8-probe-*`, `vr8-probe2-*`), which I removed at the end of each run. I killed no processes.

**Git commands**
- `git rev-parse HEAD` returned `03b25eb9b3d7e80576394109ca4b73cc21b2fab9`, which matches the scope.
- `git log --oneline 26d050e6..HEAD`: 8 commits, from `6d57eab0` to `03b25eb9`.
- `git diff --stat 26d050e6..HEAD`: 41 files.
- I read the full `git diff 26d050e6..HEAD` for:
  - `scripts/check_sley2_trial_runner.py` and `scripts/test_sley2_trial_runner.py`
  - `docs/spec/SLEY2_TRIAL_RUNNER_V1.md` and `docs/spec/SMP1.md`
  - `crates/sley-protocol/src/server.rs`
  - ADR-0032, ADR-0036, the S20-620 closeout and `docs/WORK_PACKAGES.md`
- `git show 26d050e6:scripts/check_sley2_trial_runner.py`: the base checker, exec'd in memory for the revert run.
- `git show 26d050e6:…/machine-summary.json` compared with HEAD for the `sley2_trial_runner` section.
- `git log --all -- docs/spec/SLEY2_TRIAL_RUNNER_V1.md`: 17 commits. Exactly one of them, `6519f6e0`, carries revision 8. It is an ancestor of HEAD, and its spec blob `cc180d72` is identical to HEAD's.

**Checkers and tests**
- `python3 scripts/check_sley2_trial_runner.py`: exit 0, `"result": "PASS"`, `"revision": 8`, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, `problems: []`.
- `python3 scripts/test_sley2_trial_runner.py -v`: exit 0, "Ran 7 tests … OK" (3 in `CompletionScopeBinding`, 4 in `ComposedPins`).
- `python3 scripts/check_finding_register.py`: exit 0, `"result": "PASS"`, `problems: []`.
- `cargo test --offline --locked -p sley-cli --test cli native_test_worker_entry_runs_the_unit_argv_against_the_real_binary`: 1 passed. It finished in 1.34 s with no recompile, so `$CARGO_TARGET_DIR/debug/sley` is current for this tree.
- **Revert run:** I loaded the new `CompletionScopeBinding` tests against the 26d050e checker. All three fail there:
  - two with `AssertionError: 0 != 1`, because the old gate accepts;
  - one with an `AttributeError`, because `scope_bound_problems` does not exist in the old checker.

**Files read**
- `evidence/review/verdicts/sley2_trial_runner/vulcan_surface_review_revision_7-26d050e.md:1-146`
- `scripts/check_sley2_trial_runner.py:1-382`
- `scripts/test_sley2_trial_runner.py:1-32`, plus the added lines 48-137 via the diff
- `scripts/build_finding_register.py:560-609`
- `crates/sley-protocol/src/server.rs:1165-1204, 2210-2224, 2226-2265, 2360-2479, 2915-2984`
- `crates/sley-txn/src/repository.rs:881-906, 4055-4070`
- `crates/sley-txn/src/maintenance.rs:36-111`
- `bench/live/sley2_tool.py:162-176, 289-498, 1017-1027`
- `docs/spec/SMP1.md:72-82, 764-780`
- `docs/spec/SLEY2_TRIAL_RUNNER_V1.md:18-36, 326-346`
- The machine-summary `sley2_trial_runner` section
- `evidence/review/rounds/context-r8-26d050e.json` and `evidence/review/rounds/context-r7-2b0f1c9.json`

## Evidence checked

**Status of each finding from this lane's 26d050e review**

1. [P3] [evidence-binding] The completion gate bound verdicts by field name only (`scripts/check_sley2_trial_runner.py:324-331` at 26d050e): **CLOSED.**
   - The stale value moved byte-exact into `vulcan_surface_review_delta_2b0f1c9`, with a historical note. It still equals the 2b0f1c9 transcript's `VERDICT:` line.
   - `vulcan_surface_review_revision_7` now holds the genuine 26d050e PASS.
   - `scope_bound_problems` (`:171-198`) now requires the note to name a 40-hex commit whose spec Status line reads the current revision. An unscoped note, an unknown commit or another revision each fails closed.
   - The reviewers' probe is a regression test (`test_sley2_trial_runner.py:94-108`): the pre-existing value with its 2b0f1c9 note yields `completion-scope-mismatch:vulcan_surface_review:reviewed-revision-6`.
   - All three new tests fail on the 26d050e checker and pass at HEAD.
   - The three recorded `_revision_7` values and notes match the round-8 index entries, the transcripts' `VERDICT:` and `SCOPE_SHA:` lines, and the transcripts' sha256.
   - A residual is filed as finding 1 below.

2. [P3] [error-precedence] The false "absent boundary fails the method" sentence (SLEY2_TRIAL_RUNNER_V1 section 9, server docstring, inherited from SMP1): **OPEN (carried as finding 2).**
   - For `workspace.open`, the corrected sentences in the spec (`:338-342`) and the docstring (`server.rs:2931-2935`) are true.
   - The call chain is: session check (`:1184-1186`) → `head_mixed` (`:2391-2392`) → `maintenance()` (`:2469-2473`), which runs `initialize_repository_maintenance`.
   - I re-ran the live probe at HEAD with `locks/maintenance.lock` deleted. `workspace.open` succeeded with a 211-byte, 8-field body, and the lock was recreated.
   - The server test I named as closure evidence was not added. The server.rs diff is docstring-only, and no test under `crates/sley-protocol` or `crates/sley-cli` removes the lock.
   - The SMP1 revision 16 erratum that the corrected sentence cites as its authority states a broader rule that is false (finding 2).

**Attempts to satisfy the revision-8 gate with a mis-scoped PASS**

Each probe below flips the status in memory to COMPLETE with `implementation_complete: true`, using the same `read` override as the tests.

- **A (accepted).** I copied older-round PASS values into `<lane>_revision_8`:
  - Ariadne `_revision_6` (reviewed at 2b0f1c9), Nabu `_revision_6` (reviewed at f073811), and the Vulcan 2b0f1c9 delta;
  - each with a note "verdict on 03b25eb9…; transcript …-2b0f1c9.md".
  - Result: exit 0, `problems: []`. The gate accepts a note whose own transcript stamp contradicts its sha.
- **B (accepted).** A note reading "carried on 03b25eb9…; Lane verdict dated … on 26d050e6…" gives exit 0: the first `on <sha>` wins. With the order reversed it gives `completion-scope-mismatch`.
- **C (accepted).** A tree id (`400a8412…`, HEAD's tree) instead of a commit gives exit 0, because `git show <tree>:<path>` resolves.
- **D (accepted).** A bare value `"PASS"` with a note naming HEAD and no transcript gives exit 0.
- **Refused as expected.** Uppercase, 12-character and 41-hex shas, and "upon <sha>", each give `completion-unscoped-review`. An unknown commit gives `completion-unknown-commit`.
- **Related checks.** No other checker ties a machine-summary `_revision_N` value to the round index or to a transcript: there is no in-tree reader of `COUNCIL_REVIEW_OUTPUT_INDEX`, and no stage checker compares against `VERDICT:` lines. The gate checks neither ancestry nor the spec blob. No revision-8 text drift exists today.

**Absent-boundary behaviour off the `workspace.open` path**

These are live probes on the real binary with the trial-runner profile (`--protocol-profile v2-capable`, version 2), each run after deleting `locks/maintenance.lock`:

| Method | Result | Lock recreated? |
|---|---|---|
| `entity.version` | failed with `TXN_IO` | no |
| `entity.version` (control, lock present) | failed at the query owner (`QUERY_UNRESOLVED_ENTITY`) | not applicable |
| `revision.read` | ran (payload refusal) | yes |
| `workspace.open` | succeeded | yes |

The entity-read path is:
- `dispatch_entity_read` (`server.rs:2243`)
- → `session_check_retained` (`:2210-2224`)
- → `head_binding` (`:2360-2369`)
- → `head()`
- → `accepted_head()` (`repository.rs:881-885`)
- → `acquire_shared_repository_maintenance`, which "Returns an I/O error when the boundary is absent" (`maintenance.rs:102-111`).

This path does not use `maintenance()`.

**Other delta text**
- ADR-0036, the closeout, WORK_PACKAGES and the spec history agree with the recorded round-8 state: revision 7 got Vulcan PASS with Ariadne and Nabu REVISE, and revision 8 is pending.
- The spec's SMP1 pin (16) equals SMP1's Status line.
- Other consumers keep revision 15, as SMP1 permits. The trial runner citing the erratum revision is defensible.
- `make`-level coverage is unchanged from round 7.

## Findings

[P3] [evidence-binding] scripts/check_sley2_trial_runner.py:171-198,363-365 (tests scripts/test_sley2_trial_runner.py:57-132) - The revision-8 completion gate binds a lane PASS to whatever commit the recorder's `_note` names first, never to filed evidence. In-memory COMPLETE probes accepted four mis-scoped PASSes (exit 0, `problems: []`): (A) older-round PASS values (Ariadne and Nabu `_revision_6`, the Vulcan 2b0f1c9 delta) copied into `<lane>_revision_8` with notes naming HEAD while citing `-2b0f1c9.md` transcripts; (B) a note whose first `on <sha>` is a revision-8 commit and whose actual scope, 26d050e, comes second; (C) a tree id instead of a commit; (D) a bare "PASS" with no transcript. The gate checks neither ancestry nor the reviewed spec blob, and the `VERDICT:` value is not checked either, although the in-tree round index records field, verdict, scope_sha, path and sha256 for every review. Recorded state is consistent today, and the round-7 dispatch-naming collision is now refused, so this is residual. - Closure evidence: require each `<lane>_revision_N` value to match a round-index entry with the same section and field whose scope_sha is a commit (`git cat-file -t`) that is an ancestor of HEAD and reads revision N, and whose transcript sha256, `VERDICT:` and `SCOPE_SHA:` lines match; alternatively, bind the note's scope uniquely (reject notes with more than one `on <sha>`); plus negative tests for probes A to D.

[P3] [error-precedence] docs/spec/SMP1.md:770-777 with docs/spec/SLEY2_TRIAL_RUNNER_V1.md:338-342, crates/sley-protocol/src/server.rs:2210-2224,2243,2360-2369, crates/sley-txn/src/repository.rs:881-885 and crates/sley-txn/src/maintenance.rs:102-111 (carried from 26d050e, revised) - The revision-16 erratum replaces one false precedence claim with another. It says "the S20-390 blocking shared maintenance acquisition … first creates the maintenance boundary when it is absent (`initialize_repository_maintenance`, run by the session check that precedes every session-bound answer)". But the S20-390 acquisition requires an existing boundary; only the server's `maintenance()` wrapper creates one. And the version-aware entity reads 306/307 run their session check through `head_binding` → `head()` → `accepted_head()`, which never initializes. Live probe with the lock deleted: `entity.version` failed with TXN_IO and the lock stayed absent, while `revision.read` and `workspace.open` recreated it. The trial runner's corrected sentence is behaviourally right for `workspace.open` but repeats the misattribution ("the shared maintenance acquisition, which creates the boundary"). The server pin test requested at 26d050e was not added. No runner behaviour depends on this. - Closure evidence: SMP1 appendix A text scoped to the generic session check (the `maintenance()` path), stating that the version-aware 306/307 path fails TXN_IO on an absent boundary (or a separately reviewed behaviour change that initializes there); section 9 attributing creation to the session check rather than the S20-390 acquisition; plus server tests pinning `workspace.open` success with lock recreation, and the 306/307 outcome, over a removed lock.

## Assessment

The revision-8 checker change closes the concrete defect from my round-7 review.
- The collided round-7 value was moved byte-exact into a non-binding historical field.
- The gate now refuses a PASS whose recorded scope reads another revision, as well as unscoped and unknown-commit notes.
- The reviewers' probe is a regression test, and I showed that all three new tests fail on the 26d050e checker.
- The recorded revision-7 verdicts match their transcripts and the round-8 index.

The binding still trusts a self-asserted note. When asked to beat the gate with a mis-scoped PASS, I did it four ways:
- a copied older-round PASS with a note naming HEAD;
- a note whose first sha is not the reviewed scope;
- a tree id;
- a bare "PASS" with no transcript.

Exploiting these needs a mis-written note rather than just a mis-named field, and the status is still PENDING, so this is P3, not P2.

The absent-boundary correction is right for `workspace.open`, and I re-verified it live. But the SMP1 revision-16 erratum overgeneralizes: version-aware entity reads fail TXN_IO on an absent boundary and do not recreate it. The requested server pin test is also absent, so the prior error-precedence finding stays open at P3.

Nothing in the delta touches runner code. I found no P0 to P2 issue in this package's owned paths. This lane accepts contract revision 8 with two P3 findings.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_0_P4
SECTION: sley2_trial_runner
FIELD: vulcan_surface_review_revision_8
SCOPE_SHA: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9
FINDINGS: [P3] [evidence-binding] scripts/check_sley2_trial_runner.py:171-198,363-365 (tests scripts/test_sley2_trial_runner.py:57-132) - the revision-8 completion gate binds a lane PASS to whatever commit the recorder's _note names first, never to filed evidence; in-memory COMPLETE probes accepted (exit 0, problems []) (A) older-round PASS values copied into <lane>_revision_8 with notes naming HEAD while citing -2b0f1c9.md transcripts, (B) a note whose first `on <sha>` is a revision-8 commit and whose actual scope 26d050e comes second, (C) a tree id instead of a commit, (D) a bare "PASS" with no transcript; the gate checks neither ancestry, the reviewed spec blob nor the VERDICT value, although the round index records field/verdict/scope_sha/path/sha256 - require each value to match a round-index entry (same section and field, scope_sha a commit that is an ancestor of HEAD and reads revision N, transcript sha256/VERDICT/SCOPE_SHA matching) or reject multi-sha notes, plus negative tests for probes A-D; [P3] [error-precedence] docs/spec/SMP1.md:770-777 with docs/spec/SLEY2_TRIAL_RUNNER_V1.md:338-342, server.rs:2210-2224,2243,2360-2369, repository.rs:881-885, maintenance.rs:102-111 (carried from 26d050e, revised) - the revision-16 erratum says the S20-390 shared acquisition creates an absent boundary and that the creating session check precedes every session-bound answer, but acquire_shared_repository_maintenance requires an existing boundary (only the server's maintenance() creates it), and version-aware 306/307 check the session via head_binding -> head() -> accepted_head() without initializing; live probe with the lock deleted: entity.version failed TXN_IO and the lock stayed absent, while revision.read and workspace.open recreated it; section 9 repeats the misattribution, and the requested server pin test was not added - SMP1 text scoped to the generic session check with the 306/307 TXN_IO outcome stated (or a reviewed behaviour change), section 9 attributing creation to the session check, plus server tests pinning workspace.open and 306/307 over a removed lock
SUMMARY: Revision 8 closes the collided-field defect: the stale Vulcan value moved byte-exact, the gate refuses a PASS whose recorded scope reads another revision, and the new tests fail on the 26d050e checker and pass at HEAD. The binding still trusts a self-asserted note, and four mis-scoped PASS probes (copied older verdict, first-sha-wins note, tree id, bare PASS) satisfied the gate (P3). The absent-boundary correction is live-verified for workspace.open, but the SMP1 revision-16 erratum overgeneralizes (version-aware entity reads fail TXN_IO without creating the boundary) and the requested pin test is absent (P3, carried). With no P0-P2 findings, this lane accepts contract revision 8.
