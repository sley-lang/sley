<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: ariadne; field: ariadne_contract_review_revision_7; dispatched: 2026-09-23T13:57:47Z; duration_s: 733; process_exit_code: 0 -->
# Ariadne Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

What I verified myself. I made no tracked change to the tree. Side effects:
- `cargo test` found fresh artifacts and did not recompile.
- Python wrote gitignored `__pycache__` files.
- Every probe scratch directory under `~/.cache` was removed.
- When I ran the new test file against the old bench code, its stalled `sleep` child (argv[0] `sley2-stall-*`) was left orphaned, as expected on old code; I killed it.
- One first attempt of that run lost its output because my `pkill -f` matched the parent shell. Its cleanup still ran, and I checked that no `ar7-*` directory survived.

During the session, 15 untracked verdict files from other concurrent dispatches appeared under `evidence/review/verdicts/`. They are not mine and I did not read them. `HEAD` was unchanged at the end.

**Scope and history**
- `git rev-parse HEAD` returned 26d050e629b669ef4acbd54826ef4008c05a061d, which matches the scope.
- `git log --oneline 2b0f1c9f..HEAD`: 12 commits. `git diff --stat 2b0f1c9f..HEAD`: 86 files.
- Full diffs read, restricted to this package and its composed owners:
  - `docs/spec/SLEY2_TRIAL_RUNNER_V1.md`, `scripts/check_sley2_trial_runner.py`, and the new `scripts/test_sley2_trial_runner.py`
  - `crates/sley-protocol/src/server.rs`
  - the SMP1 and S20-300 changed lines
  - ADR-0036, the S20-620 closeout, and the WORK_PACKAGES S20-620 row
  - `bench/live/SUCCESSION-COVERAGE.md`
  - `bench/live/scratch.py`, `sley2_tool.py` and `prove_merge_production.py`
  - the witness wording, shown for `succ_witness_context.py` and `succ_witness_create.py`
  - `bench/live/tests/test_scratch_cleanup.py`
- `git diff --stat 2b0f1c9f..HEAD` is empty for the runner, judge, TOOLING and mediated-route code (`bench/sley2`, `sley2_live_judge.py`, `TOOLING.md`, `mediated_sley.py`, `trusted_capture.py`). So "no runner behavior changes" (spec :26) holds.
- `git log 2b0f1c9f..HEAD -- <spec> <checker>` shows only 60be11c8, the revision-7 bump.
- Round index `evidence/review/rounds/context-r7-2b0f1c9.json`: the sha256 of both committed sley2_trial_runner transcripts matches the index (ce93ba08… and f75e3711…). The recorded fields equal the transcripts' VERDICT lines.

**Checkers** (python3 subprocess)

| Checker | Exit | Result |
|---|---|---|
| `check_sley2_trial_runner.py` | 0 | PASS, revision 7, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, problems [] (includes its run of `bench/sley2/tests`) |
| `python3 -m unittest scripts/test_sley2_trial_runner.py` | 0 | 4 tests OK |
| `check_smp1_contract.py` | 0 | PASS, revision 15 |
| `check_complete_root_index_snapshot_profile.py` | 0 | PASS, revision 6 |
| `check_finding_register.py` | 0 | PASS; the register's own result is `FINDING_REGISTER_OPEN` |

**In-memory completion probe.** I monkeypatched the checker's `read` for the summary path; nothing was written.

| Case | Result |
|---|---|
| baseline | exit 0 PASS |
| status `S20_620_COMPLETE` + `implementation_complete` true + only `ariadne_…_revision_7` and `nabu_…_revision_7` PASS | **exit 0 PASS, problems []** |
| COMPLETE with only Ariadne r7 | FAIL `completion-unbound-review:nabu_architecture_review` |
| COMPLETE with no new r7 fields | FAIL for Ariadne and Nabu only |
| summary `contract_revision` 6 | FAIL `machine-summary:contract_revision:6!=spec:7` |

**Live probe (absent boundary).**
- Setup: the HEAD-built `sley` binary, fresh by cargo fingerprint and confirmed by the no-recompile `cargo test` above. `sley2_tool.Session` over a `~/.cache` mkdtemp workspace seeded with `bench/fixtures/sley2/S2B-CONTEXT-001/base.pack`.
- With the boundary present, `workspace.open` answered failed=False, 212 bytes.
- I then deleted `repo/locks/maintenance.lock`. The next `workspace.open` answered failed=False, 212 bytes, byte-identical to the first answer, and the lock file existed again.
- The scratch was removed.

**Tests**
- `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 5 passed. This includes the new `workspace_open_answers_from_the_single_checked_head_load`.
- `cargo test --locked --offline -p sley-repo --lib index_cache`: 13 passed.
- `cargo test --locked --offline -p sley-cli --test cli workspace_open`: 0 matched, no recompile.
- `bench.live.tests.test_scratch_cleanup` with `SLEY2_SLEY_BINARY` bound and a private TMPDIR: 11 OK. `bench.live.tests.test_witness_provenance`: 4 OK. The private TMPDIR was empty afterwards.
- HEAD's `test_scratch_cleanup.py` run against the 2b0f1c9f `bench/` tree (`git archive` into `~/.cache`): exit 1 with 4 FAILs. These are exactly the four new tests:

  | Test | Failure on old code |
  |---|---|
  | `test_a_stalled_serve_child_is_reaped_and_no_scratch_reappears` | `[2231325] != []` |
  | `test_a_failing_side_read_leaves_no_merge_scratch` | `['sley2-merge-side-…'] != []` |
  | `test_a_hard_linked_outside_file_keeps_its_mode` | `384 != 256` |
  | `test_the_parent_is_never_widened` | `ScratchRemovalError` not raised |

- `bench.live.tests.test_tooling` + `test_mediated_context`: 22 ran, OK. The 8 integrated CONTEXT proofs were skipped because `SUCC_JUDGE_TEST_BINARY` was unbound. For them, and for the 258-test `bench/live` run, I rely on gate record §13.4, and no finding below depends on them.
- Not run: witnesses, `make quick`, `make lint`.

**Files read** (line ranges)
- Transcripts, in full: my `ariadne_contract_review_revision_6-2b0f1c9.md` and Vulcan's `vulcan_surface_review_revision_7-2b0f1c9.md`.
- Gate record 784-964.
- Specs:
  - `SLEY2_TRIAL_RUNNER_V1.md` 1-70 and 280-374
  - `SMP1.md` 3 and 730-790
  - `COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` 3 plus the diff
- Checkers and tests:
  - `check_sley2_trial_runner.py` 1-347 (all)
  - `test_sley2_trial_runner.py` (all)
  - `build_finding_register.py` 415-471
  - `Makefile` 84, 109, 209
- Rust:
  - `server.rs` 339-342, 1130-1230, 2375-2430, 2469-2486, 2913-2971
  - `sley-txn/src/maintenance.rs` 60-200
- Bench code:
  - `sley2_tool.py` 260-499
  - `prove_merge_production.py` 128-150, 185-212
  - `test_mediated_context.py` 40-59
  - `sley2_live_judge.py` 205-229
- Records:
  - the machine summary `sley2_trial_runner` section (every key, diffed against 2b0f1c9f)
  - finding register obligations 429-438, open_reviews 42-44, unclaimed_carried_findings 4-5

## Evidence checked

- **Contract revision 7 text.**
  - The Status line at :3 says revision 7. The history sentence at :21-26 names the pin moves and the absent-boundary sentence.
  - The Boundary (:43) pins "SMP1 revision 15 defines". §9 pins `docs/spec/SMP1.md` revision 15 (:323-324) and S20-300 section 5, revision 6 (:327).
  - These match `SMP1.md:3` (revision 15) and the profile's `:3` (revision 6).
  - The frozen nineteen names (:309-315), the anchors and the continuation rule (:349-357) are unchanged.
- **Composed-pin gate.**
  - `composed_pin_problems` (`check_sley2_trial_runner.py:146-168`) reads both owner Status lines. It requires every "SMP1 revision N defines" / "`docs/spec/SMP1.md` revision N" pin and every S20-300 "section 5, revision N" pin to equal them. It is wired at :181-185 and in `make quick` (Makefile:109).
  - The four tests cover the current pins, a stale SMP1 pin, a stale S20-300 pin, and an authority moving. All pass.
  - No other SMP1 or S20-300 revision pin exists in the spec (grep).
- **SMP1 r15 and S20-300 r6 against what §9 composes.**
  - SMP1 r15 changes the per-selection negotiation filter text and pins. `open_summary` and its version-2 applicability are unchanged apart from "revisions 13 to 15" and the S20-300 r6 pin.
  - S20-300 r6 hardens the probe (canonical guard root, real-directory components, and a gate that also forbids initialization in the wrapper). Every such failure is still absence, which §9's "any probe failure" covers.
- **Server delta.**
  - `workspace.open` now answers from the head its session check retained (`server.rs:1179-1189`, `2410-2420`, `2939-2945`). This strengthens §9's "Disclosure and discovery pin one revision by construction".
  - The docstrings are re-pinned to SMP1 15 and S20-300 6 (:2916, :2955, :3326).
  - The single-load test passes.
- **Vulcan round-7 bench findings**, part of revision 7 per the gate record's §13.3 rows at :933-937:
  - Reaping on a failed `Session.__init__`: `sley2_tool.py:269-314`.
  - Merge-prover stages are registered as created, every tree is attempted, and the prover runs under `scratch_root`: `prove_merge_production.py:130-131`, `:203-210`, `:318-339`.
  - Grants are limited to directories in the tree: `scratch.py:33-50`.
  - The no-concurrent-writer precondition is documented at `scratch.py` `remove_scratch` docstring.
  - The witness wording now says the removal is scheduled.
  - All four new tests fail on the 2b0f1c9f code and pass at HEAD, as listed above.
- **Records.**
  - ADR-0036 Status, the closeout Revision paragraph and the WORK_PACKAGES S20-620 row all say revision 7 is pending review. WORK_PACKAGES names the correct scopes for the revision-6 PASSes.
  - Summary: `contract_revision` 7, status `…REVIEW_PENDING`, `implementation_complete` false.
  - The historical fields are byte-identical to 2b0f1c9f. The only additions are `ariadne_…_revision_6` and `vulcan_…_revision_7` with notes, and those values equal the transcripts.
  - The register obligations 431 and 438 carry them with round_scope 2b0f1c9.
- **The pre-existing `vulcan_surface_review_revision_7`**, which the dispatch asked about:
  - Its note says it "reviews the contract revision 6 text plus the scratch-workspace leak fix d307a22f".
  - The checker's completion binding (:324-331) tests only `section.get(f"{key}_revision_{spec_revision}")` for a "PASS" prefix. It reads no scope, date or note.
  - The spec's revision-7 sentence does not say this field binds nothing. Revision 6's sentence did say so for the revision-5 verdicts (:19-20).
  - The status_note says only "completion needs PASS _revision_7 fields".
  - The probe above shows this stale field satisfies the Vulcan lane. The implementation does not prevent it (finding 1).

### Round-7 findings of this lane (ariadne_contract_review_revision_6-2b0f1c9)

- **[P3] contract-composition, stale SMP1 13 / S20-300 4 pins (`SLEY2_TRIAL_RUNNER_V1.md:37,317-318,321`): CLOSED.** The spec pins SMP1 15 at :43 and :324 and S20-300 6 at :327, matching both owners' Status lines. The anchor `composed_pin_problems` has revert tests (stale SMP1, stale S20-300, authority moved), is in quick, and passes.
- **[P4] spec-precision, §9 counted an absent boundary as probe absence (`SLEY2_TRIAL_RUNNER_V1.md:323-327`): OPEN (superseded by finding 2).**
  - The requested edit landed: the probe-failure list no longer names an absent boundary, and §9 now echoes SMP1 appendix A.
  - But the finding's premise, mine, was wrong. I said "the code agrees with SMP1" and traced only `accepted_head` → `acquire_shared_repository_maintenance`. I missed that the session check's head load (`head_mixed` → `Server::maintenance()`, `server.rs:2391-2392`, `2469-2473`) calls `initialize_repository_maintenance` first.
  - The live probe shows an absent boundary is re-created and `workspace.open` answers. The adopted sentence is therefore false, so the precision defect persists in a new form.
- **[P4] record, SUCCESSION-COVERAGE stale row and paragraph (`bench/live/SUCCESSION-COVERAGE.md:29,540-543`): CLOSED.**
  - The CONTEXT row (:29) is refreshed and dated. It gives: the revision-6 reviews PASS in all three lanes, revision 7 pending, SMP1 r14 and S20-300 r5 PASS, and r15 and r6 pending.
  - The "Discovery gate retained" paragraph (:540-547) is marked "historical; superseded 2026-09-23" and points to the REQ-10 route.

## Findings

[P2] [completion-binding] scripts/check_sley2_trial_runner.py:324-331 (with machine-summary.json sley2_trial_runner.vulcan_surface_review_revision_7 and docs/spec/SLEY2_TRIAL_RUNNER_V1.md:21-26) - The revision-7 completion gate binds lane verdicts by field name and a "PASS" prefix only. The pre-existing `vulcan_surface_review_revision_7` = PASS_0_P0_0_P1_0_P2_2_P3_3_P4 (scope 2b0f1c9; its note says it reviews the contract revision 6 text and d307a22f) therefore satisfies the Vulcan lane at revision 7. An in-memory probe with status S20_620_COMPLETE, `implementation_complete` true and only Ariadne and Nabu `_revision_7` PASS returns exit 0 PASS. The checker, spec history sentence, status_note and register do not exclude it. A genuine Vulcan revision-7 verdict could only be recorded by overwriting that historical field. So the package can reach COMPLETE with no Vulcan review of the revision-7 text or of the 236b7640 repairs that answered Vulcan's own findings. - Closure evidence: bind by scope as well as name, or move the contract to a revision whose lane field names are unused:
- for scope, require each `<lane>_revision_7_note` scope to descend from the revision-7 commit 60be11c8, or explicitly exclude the 2b0f1c9-scoped field;
- say in the revision-7 history sentence that the round-7 Vulcan verdict binds nothing, as revision 6 did for the revision-5 verdicts;
- add a revert test in `scripts/test_sley2_trial_runner.py` showing that the current summary plus Ariadne and Nabu `_revision_7` PASS fails `completion-unbound-review:vulcan_surface_review`.

[P3] [spec-conformance] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:330-332 (with crates/sley-protocol/src/server.rs:2931-2933; owner text docs/spec/SMP1.md:763-765) - Revision 7's new clause "the opener's head load requires the boundary and fails the method first" is false against the code. The session check loads the head through `session_check_mixed_retained` → `head_binding_mixed` → `head_mixed` → `Server::maintenance()` (server.rs:1184-1186, 2410-2415, 2375-2376, 2391-2392, 2469-2473), which initializes an absent boundary before the blocking shared acquire. A live probe at HEAD deleted `repo/locks/maintenance.lock`; `workspace.open` then answered failed=False with a byte-identical 212-byte body and re-created the lock. Only "never reaches the probe" holds. The server docstring and SMP1 appendix A carry the same false clause, which my round-7 P4 wrongly endorsed. There is no trial impact. - Closure evidence: state in §9 and the docstring that the head load initializes an absent boundary before taking it shared, or make an absent boundary fail as an explicit S20-390 decision. Have the SMP1 lane amend appendix A to match. Add a server test that removes the lock between two `workspace.open` requests and pins the chosen behavior.

[P4] [record] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:21-22 - "answers the revision 6 round (PASS in all three lanes with P3/P4 findings, 2b0f1c9)" gives one scope to all three revision-6 PASSes. The Nabu and Vulcan revision-6 PASSes are scoped f073811 (answered in gate record §12). The findings revision 7 answers are Ariadne r6 and the round-7 Vulcan verdict (field `_revision_7`), both at 2b0f1c9. WORK_PACKAGES states this correctly. - Closure evidence: name the per-lane scopes as the WORK_PACKAGES row does, and name the round-7 Vulcan verdict and its non-binding status (see the P2).

## Assessment

On substance, revision 7 does what it says:
- **Pins.** The composed pins are current, and a new, tested anchor now fails on a stale pin in either direction.
- **Server and records.** `workspace.open` answers from its single checked head load. The coverage record is refreshed.
- **Vulcan round-7 bench findings.** All five are answered, and the two P3s and the permission-scope P4 are each backed by a test that I confirmed fails on the 2b0f1c9f code and passes at HEAD.
- **Runner.** The runner, judge, TOOLING and mediated route are untouched. The summary, ADR, closeout, WORK_PACKAGES and register agree, and historical fields are preserved.

Two things keep this from PASS:

1. **The completion gate question the dispatch raised.** Nothing prevents the pre-existing, revision-6-scoped `vulcan_surface_review_revision_7` PASS from satisfying the revision-7 binding. I demonstrated that the checker would accept COMPLETE with only two new lane reviews (P2).
2. **The absent-boundary sentence.** The sentence added to close my own round-7 P4 asserts a failure the code does not produce: the session check re-initializes an absent boundary, as I observed live. The fault traces to my prior premise and to SMP1 appendix A (P3).

One history sentence also misattributes review scopes (P4).

This verdict does not accept SMP1 revision 15 or S20-300 revision 6, which are reviewed in their own lanes. I make no GA, release-readiness or live-trial claim.

VERDICT: REVISE_0_P0_0_P1_1_P2_1_P3_1_P4
SECTION: sley2_trial_runner
FIELD: ariadne_contract_review_revision_7
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P2] [completion-binding] scripts/check_sley2_trial_runner.py:324-331 (with machine-summary.json sley2_trial_runner.vulcan_surface_review_revision_7 and docs/spec/SLEY2_TRIAL_RUNNER_V1.md:21-26) - the revision-7 completion gate binds by field name + "PASS" prefix only, so the pre-existing 2b0f1c9-scoped vulcan_surface_review_revision_7 PASS (a contract revision 6 verdict per its note) satisfies the Vulcan lane; in-memory probe: COMPLETE with only Ariadne/Nabu _revision_7 PASS gives exit 0 PASS; nothing excludes it, and a real Vulcan r7 verdict could only overwrite that historical field - bind by scope (note scope descends from 60be11c8, or exclude the 2b0f1c9 field) or move to an unused revision, state the non-binding in the history sentence, add a revert test failing completion-unbound-review:vulcan_surface_review | [P3] [spec-conformance] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:330-332 (with server.rs:2931-2933; owner SMP1.md:763-765) - the new clause that the opener's head load fails the method on an absent boundary is false: the session check's head_mixed -> Server::maintenance() (server.rs:2391-2392,2469-2473) initializes the boundary; live probe: after deleting repo/locks/maintenance.lock, workspace.open answered failed=False with a byte-identical 212-byte body and re-created the lock; my round-7 P4 premise was wrong - say the head load initializes an absent boundary (or make it fail as an S20-390 decision), amend SMP1 appendix A in its lane, add a server test pinning the behavior | [P4] [record] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:21-22 - the history sentence gives 2b0f1c9 as the scope of all three revision-6 PASSes; Nabu and Vulcan r6 are f073811, and revision 7 answers Ariadne r6 plus the round-7 Vulcan verdict - name the per-lane scopes as WORK_PACKAGES does, and name the round-7 Vulcan verdict and its non-binding status
SUMMARY: Revision 7 closes the stale-pin P3 with a tested composed-pin anchor and the coverage-record P4. All five Vulcan round-7 bench findings are answered, and each of the four new tests fails on the 2b0f1c9f code and passes at HEAD. The completion gate lets the pre-existing, revision-6-scoped vulcan_surface_review_revision_7 PASS satisfy the revision-7 binding, so COMPLETE is reachable with two new lane reviews (P2). The new absent-boundary sentence claims a failure the code does not produce, because the session check re-initializes the boundary, as a live probe showed; this also means my prior P4 stays open in a new form (P3). One history sentence misattributes review scopes (P4).
