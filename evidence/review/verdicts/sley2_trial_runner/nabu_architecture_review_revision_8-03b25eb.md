<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9; role: nabu; field: nabu_architecture_review_revision_8; dispatched: 2026-09-23T14:33:54Z; duration_s: 554; process_exit_code: 0 -->
# Nabu Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9

I am an independent reviewer, not the author. I did not change the reviewed tree: no edits, formatting or commits, and no writes to `.git`. Every probe ran in memory by patching the checker's `read` function. My only write was a note to my own reviewer memory, which is outside the tree. `git status --short` was empty at the start and at the end.

**Scope**
- `git rev-parse HEAD` returned `03b25eb9b3d7e80576394109ca4b73cc21b2fab9`, which matches the scope. I checked it again at the end and it was unchanged.
- Prior transcript: `evidence/review/verdicts/sley2_trial_runner/nabu_architecture_review_revision_7-26d050e.md`. I read all of it (verdict `REVISE_0_P0_0_P1_1_P2_0_P3_2_P4`).

**Git commands**
- `git log --oneline 26d050e6..HEAD`: 8 commits, `6d57eab0` to `03b25eb9`.
- `git diff --stat 26d050e6..HEAD`: 41 files, +3833/−232.
- I read the full diffs of:
  - `scripts/check_sley2_trial_runner.py` and `scripts/test_sley2_trial_runner.py`;
  - `docs/spec/SLEY2_TRIAL_RUNNER_V1.md` and `docs/spec/SMP1.md`;
  - `crates/sley-protocol/src/server.rs`;
  - ADR-0032, ADR-0036, the S20-620 closeout and `docs/WORK_PACKAGES.md`;
  - the gate record §14;
  - the sibling checkers (CLI, bridge, session, S20-300, SMP1).
- `git show --stat 22971434`: the gate commit, 4 files.
- `git show --stat 83bace58`: the verdict-recording commit, touching the machine summary only.
- `git log --format=%H 26d050e6..HEAD -- docs/spec/SLEY2_TRIAL_RUNNER_V1.md`: only `6519f6e0` changes the spec, so the spec is identical from `6519f6e0` to HEAD.
- The spec at `22971434` still reads revision 7.
- `git log --all` against the spec: 17 commits, 0 of them outside HEAD's ancestry.
- The `sley2_trial_runner` machine-summary section, compared field by field against `git show 26d050e6:…/machine-summary.json`.
- `git show 26d050e6:evidence/review/finding-register.json`, compared with HEAD.

**Checkers and tests** (each through a python3 subprocess wrapper)

| Command | Exit | Result |
|---|---|---|
| `python3 scripts/check_sley2_trial_runner.py` | 0 | `"result": "PASS"`, revision 8, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, problems `[]` |
| `python3 scripts/test_sley2_trial_runner.py -v` | 0 | Ran 7, OK (4 composed-pin cases, 3 completion-binding cases) |
| `check_smp1_contract.py` | 0 | PASS, revision 16 |
| `test_smp1_contract.py` | 0 | Ran 18, OK |
| `check_complete_root_index_snapshot_profile.py` | 0 | PASS, revision 6 |
| `test_complete_root_index_snapshot_profile.py` | 0 | Ran 23, OK |
| `check_cli_contract.py` | 0 | PASS, revision 10 |
| `check_smp1_json_bridge_contract.py` | 0 | PASS, revision 12 |
| `check_session_handle_profile.py` | 0 | PASS, `smp1_revision` 15 |
| `check_finding_register.py` | 0 | PASS |

The S20-620 checker also runs the offline `bench/sley2/tests` suite, and that passed.

**Completion-gate probes** (in memory; status set to COMPLETE; `_revision_8` fields injected)

| Probe | Exit | Problems |
|---|---|---|
| A. Positive control: three PASS notes `on 03b25eb9…` | 0 | `[]` |
| B. Notes `on 6519f6e0…` (same revision 8 text) | 0 | `[]` |
| C. Mis-scoped: the 26d050e Vulcan verdict (transcript SCOPE_SHA 26d050e, contract revision 7) filed as `vulcan_surface_review_revision_8`, note naming 03b25eb9 and the r7 transcript path | **0** | `[]` |
| D. Note with two SHAs, HEAD first then 26d050e | **0** | `[]` |
| D′. Same two SHAs, 26d050e first | 1 | `completion-scope-mismatch:nabu_architecture_review:reviewed-revision-7` |
| E. Spec §9 amended in place (a normative clause added) under the same revision 8 Status line, notes on HEAD | **0** | `[]` |
| F. No `_revision_8` fields (only the moved `vulcan_surface_review_delta_2b0f1c9`) | 1 | all three lanes `completion-unbound-review` |
| G. Short SHA in the note | 1 | `completion-unscoped-review` |

**Register probes** (`build_finding_register.py` loaded as a module)
- `supersedes("vulcan_surface_review_delta_2b0f1c9", "vulcan_surface_review_revision_5")` is True.
- `supersedes("…_revision_6", "…_revision_5")` is False.
- `field_core(delta_2b0f1c9)` is `{delta, 2b0f1c9, surface}`, and `field_early` is False for it.
- Open reviews for this section:
  - at 26d050e: `{ariadne,nabu,vulcan}_revision_5`;
  - at HEAD: `{ariadne,nabu}_revision_{5,7}`.
- Total open reviews went from 48 to 40.

**Files read, with line ranges**
- `check_sley2_trial_runner.py`: 1–60, 120–390.
- `test_sley2_trial_runner.py`: the full diff.
- `SLEY2_TRIAL_RUNNER_V1.md`: 1–40, plus 51 and 329–345 through the diff and grep.
- `SMP1.md`: 1–100 and 760–780 through the diff; lines 75 and 80 through grep.
- `server.rs`: 1170–1200, 1285–1300, 2205–2235, 2360–2500, 2915–2990.
- `server_tests.rs`: 30–40, 7544–7595; grep for `workspace_open*` and `remove_dir_all`.
- `sley-txn/src/maintenance.rs`: 36–80, 145–165.
- `sley-txn/src/repository.rs`: 881–893.
- `build_finding_register.py`: 150–240, 378–415.
- `check_smp1_contract.py`: 395–440.
- The gate record: 962–1010 (§14).
- `context-r8-26d050e.json`: the S20-620 entries.
- The Vulcan transcripts `…revision_7-2b0f1c9.md` and `…revision_7-26d050e.md`: footers.
- The finding register: obligations 444–456, `open_reviews`, `unclaimed_carried_findings`.

## Evidence checked

**Status of each finding in this lane's revision-7 transcript (26d050e)**

- **Prior [P2] [evidence-binding] completion bound by field name only — CLOSED.**
  - `scope_bound_problems` (`check_sley2_trial_runner.py:171-198`, called at `:363-365`) requires three things:
    - a PASS field;
    - an `on <40-hex>` scope in its note;
    - that commit's contract Status line reading revision N.
    Anything else fails closed: unbound, unscoped, unknown-commit or scope-mismatch.
  - My r7 scenario is refused. The existing 2b0f1c9 verdict now binds nothing (probe F). The regression test `test_the_reviewers_probe_…` covers both directions: it refuses `reviewed-revision-6` and admits the 26d050e scope. The gate is satisfiable when properly scoped (probe A).
  - The historical verdict's value is byte-exact under `vulcan_surface_review_delta_2b0f1c9` (`machine-summary.json:4539`), and its original note is embedded byte-exact after `Original note: `. The genuine 26d050e Vulcan PASS now sits in `vulcan_surface_review_revision_7`, which matches the r8 round index (field, verdict and scope_sha all agree).
  - Residual weaknesses in the new mechanism are filed below as a P3 and a P4. They do not reopen the demonstrated scenario.
- **Prior [P4] [ownership] `_abandon` reaches into runner-owned `Endpoint._process`/`_closed` — OPEN.** Nothing under `bench/` changed in this delta except the gate record. The code at `bench/live/sley2_tool.py:269-286,313` is unchanged. The finding is recorded as a `p4_open` claim.
- **Prior [P4] [record] corpus task-input amendment ratification — OPEN.** The gate record still lists it at `:960` and `:1006`.

**Revision-8 delta**

- **Absent-boundary correction.** The code bears it out:
  - `dispatch` runs `session_check_mixed_retained` before `workspace_open` (`server.rs:1184-1186`, `:1295`).
  - That goes `head_binding_mixed` → `head_mixed` → `self.maintenance()` (`:2375-2392`).
  - `maintenance()` runs `initialize_repository_maintenance` and then the blocking shared acquire (`:2469-2472`).
  - `initialize_repository_maintenance_inner` creates `locks/` and `maintenance.lock` when they are absent (`maintenance.rs:54-80`).
  - Nothing in production removes the boundary.
  - So the revision 8 text (`SLEY2_TRIAL_RUNNER_V1.md:338-342`), SMP1 appendix A and the docstring (`server.rs:2931-2935`) are correct for `workspace.open`.
  - Candidly: my revision-7 transcript said "the code bears out" the old sentence that an absent boundary fails the method. That was wrong, because I read the acquire and missed the initialization in `maintenance()`. The Ariadne and Vulcan lanes caught it.
- **Out-of-scope observation (SMP1's lane, not filed here).** By code reading, the version-aware entity-read path does not initialize the boundary: `dispatch_entity_read` → `session_check_retained` → `head_binding` → `head()` → `accepted_head()` → `acquire_shared_repository_maintenance` (`repository.rs:881-884`). SMP1 revision 16's wording, "the session check that precedes every session-bound answer", is therefore wider than the code. S20-620's own sentence is scoped to `workspace.open` and is correct. In a trial, `session.open` (a mixed check) always runs first.
- **Revision bump and history.** The Status line reads revision 8. The history (`:21-34`) names the scope of each lane and the moved delta verdict, and says there are no runner behaviour changes.
  - `git diff --stat 26d050e6..HEAD -- bench/sley2 bench/fixtures bench/live/*.py` is empty.
  - The ADR-0036, closeout and WORK_PACKAGES rows agree: the revision 8 review is pending and the status is `IMPLEMENTED_REVIEW_PENDING`.
- **Composed pins.** `composed_pin_problems` still anchors on SMP1's document revision (`:155`). The spec pins 16 at `:51` and `:331`. The four composed-pin tests pass.
  - The CLI, bridge and session checkers use a new "errata-only over normative revision N" rule and pin 15.
  - The difference is intentional per the gate record (`:988`), but it creates two pin semantics for one authority (P4 below).

## Findings

[P3] [evidence-binding] scripts/check_sley2_trial_runner.py:171-198,363-365 (same copy at scripts/check_complete_root_index_snapshot_profile.py:223-250) - The reviewed commit is taken from the first `on <40-hex>` in the hand-written `_note` (:185). The checker never reads the reviewer's own evidence: not the transcript footer (FIELD/SCOPE_SHA/VERDICT), and not the round index, which records field, verdict, scope_sha, path and sha256 for every review (evidence/review/rounds/context-r8-26d050e.json). It also does no ancestry check. Mis-scoped PASS demonstrated: the 26d050e Vulcan verdict, whose transcript and round index both scope contract revision 7, filed as `vulcan_surface_review_revision_8` with a note naming 03b25eb9 and the r7 transcript path gives exit 0 and problems []. A note carrying two SHAs binds by order, not by what was reviewed (probe D gives exit 0). - Closure evidence: resolve the scope from the round-index entry for `<field>`. Require the verdict to equal the summary value, the transcript sha256 to match, the entry's scope_sha to show revision N, and that commit to be an ancestor of HEAD. Fail closed when there is no entry. Add the mis-scoped-note probe as a refused regression case in both checkers.

[P4] [identity] scripts/check_sley2_trial_runner.py:171-198 - The gate binds the Status revision number, not the reviewed text, although its docstring (:174) and commit 22971434 say "to the text it reviewed". Probe E: I added a normative clause to §9 under the same revision 8 Status line (letting the runner read raw repository files when the probe is absent), with notes on 03b25eb9; the result was exit 0. So an in-place amendment after the reviews (the class of my r6 P3) would keep all three verdicts bound. - Closure evidence: compare the contract text below the Status/history preamble at the scoped commit with the working copy, and fail on any difference, with a test. Alternatively, reword the docstring and records to claim only "revision N".

[P4] [record] machineresearch/sley-2.0/machine-summary.json:4502 (sley2_trial_runner.p4_open[1]) - The claim recorded for my r7 corpus finding has absorbed the transcript's separate SUMMARY line ("…design chain SUMMARY: Four of this lane's five…") and is cut off mid-word ("the corpus r"). The register ingests it as `package_open_claims` `sley2_trial_runner.p4_open`. - Closure evidence: re-extract the claim byte-exact from the r7 transcript's FINDINGS line (:140), splitting on ` | ` and stopping at the line end. Make the extractor refuse any claim that contains `SUMMARY:`.

[P4] [composition] docs/spec/SMP1.md:75-81, docs/adr/ADR-0032-smp1-transport-boundary.md:25-29 vs docs/spec/SLEY2_TRIAL_RUNNER_V1.md:51,331 and scripts/check_sley2_trial_runner.py:155 - SMP1 revision 16 and ADR-0032 state without qualification that "consumers keep their revision 15 pins". But S20-620 (an SMP1 consumer) pins revision 16 under a second pin rule: the document revision, where the CLI, bridge, session and NATIVE checkers use the normative revision. The exception is recorded only in the gate record (:988). The spec also says at :51 that the `workspace.open` response is one "that SMP1 revision 16 defines", yet revision 16 defines no response change. The result is two pin semantics for one authority. This fails closed, since a future errata revision turns only S20-620 red, hence P4. - Closure evidence: SMP1 and ADR-0032 name S20-620's document-revision pin as the deliberate exception. Alternatively, S20-620 pins normative revision 15 and cites the revision 16 erratum only for the §9 sentence.

[P4] [test-coverage] crates/sley-protocol/src/server.rs:2931-2935 (with docs/spec/SLEY2_TRIAL_RUNNER_V1.md:338-342 and SMP1 appendix A) - The corrected clause is now normative in two contracts: the session check's `maintenance()` creates an absent boundary, so `workspace.open` succeeds and the probe finds the boundary. No test covers it. `server_tests.rs` never removes `locks/`, and its `maintenance_guard` helper (:35-38) initializes the boundary itself. The false revision 7 text survived a full round, including my own r7 reading. - Closure evidence: an owner-side server test that deletes `locks/maintenance.lock` and `locks/`, calls `workspace.open`, and asserts success and the recreated boundary.

[P4] [record] evidence/review/finding-register.json obligations[454] (scripts/build_finding_register.py:183-208; bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:1005) - After the move, the register's token rule reads `vulcan_surface_review_delta_2b0f1c9` (core {delta, 2b0f1c9, surface}, not early) as a later general PASS: `supersedes(delta, revision_5)` is True, while `supersedes(revision_6, revision_5)` is False. As a result, `vulcan_surface_review_revision_5` REVISE, which was PENDING at 26d050e, became HISTORICAL_ROUND "superseded_by" the 2b0f1c9 delta verdict and left `open_reviews`. The Ariadne and Nabu revision 5 REVISE rows stay open. The gate record (:1005) states that no later round supersedes an earlier one and does not mention this effect, which feeds into the dossier's "40 open reviews". - Closure evidence: choose a field name the register reads as a round-scoped historical verdict, or record the Vulcan revision 5 supersession against its real closer (revision_6 at f073811) through S20-740. Note the effect in the gate record either way.

[P4] [ownership] bench/live/sley2_tool.py:269-286,306-314 (with bench/sley2/runner.py:346-422) - Carried from r7 and still OPEN; the code is unchanged in this delta. `_abandon` writes into the runner-owned `Endpoint._process`/`_closed`, so renaming either attribute would silently disable the reap. Its only test needs the binary. - Closure evidence: an owner-side `Endpoint.abort()` with an offline `bench/sley2` test, or `_abandon` failing loudly when the attribute is missing.

[P4] [record] bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:960,1006 - Carried from revisions 5 to 7 and still OPEN: the settled task-input amendment (REQ-10-rev2:49-51) is still neither implemented nor ratified. The stand-in fails closed. - Closure evidence: the corpus owner ratifies dropping or deferring the amendment in the REQ-10 design chain.

## Assessment

**Why I accept revision 8.** My revision-7 P2 is closed:
- The completion gate now requires the reviewed commit to show revision N. It fails closed on unscoped or unknown scopes.
- The exact scenario I demonstrated is refused and is pinned as a regression test.
- The gate is still satisfiable by properly scoped verdicts.
- The historical 2b0f1c9 verdict is kept byte-exact under a name that binds nothing.

**Other checks.**
- The absent-boundary correction is accurate for `workspace.open` by the code path, and the correction was needed: my own r7 reading of this sentence was wrong.
- The runner, judge and bench code are unchanged.
- Every package and sibling checker passes.

**The P3.** I could satisfy the gate with a mis-scoped PASS. The scope comes from a hand-written note, not from the reviewer's transcript or the round index, both of which already carry the authoritative scope and verdict. This is not P2: it needs a wrong note, while the r7 defect was satisfied by the existing truthful record.

**The P4s** concern:
- binding to the revision number rather than the text;
- a corrupted open-claim record;
- two SMP1 pin semantics;
- no test for the corrected clause;
- a register side effect of the field move;
- two findings carried from earlier rounds.

None of them blocks acceptance. I make no GA or release-readiness claim, and I do not review SMP1 16, S20-300 6, or the CLI, bridge or session packages as packages of their own.

The Google Calendar and Google Drive connectors need authorizing in claude.ai connector settings, and Vercel through `claude mcp` or /mcp. They stay unavailable until then; this review did not need them.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_7_P4
SECTION: sley2_trial_runner
FIELD: nabu_architecture_review_revision_8
SCOPE_SHA: 03b25eb9b3d7e80576394109ca4b73cc21b2fab9
FINDINGS: [P3] [evidence-binding] scripts/check_sley2_trial_runner.py:171-198,363-365 (same copy at scripts/check_complete_root_index_snapshot_profile.py:223-250) - scope comes from the first `on <sha>` in the hand-written note; transcript footer and round index (field/verdict/scope_sha/sha256) unread, no ancestry check; mis-scoped PASS (26d050e revision-7 Vulcan verdict as _revision_8, note naming 03b25eb9) gives exit 0; two-SHA notes bind by order - bind to the round-index entry (verdict equal, sha256 match, scope_sha shows N, ancestor of HEAD), fail closed with no entry, add the mis-scoped-note refusal test | [P4] [identity] scripts/check_sley2_trial_runner.py:171-198 - binds the Status revision number, not the reviewed text (docstring :174 and 22971434 claim text); an in-place §9 amendment under revision 8 passes with notes on 03b25eb9 - compare the contract text below the preamble at the scoped commit with the working copy, with a test, or reword the claims | [P4] [record] machineresearch/sley-2.0/machine-summary.json:4502 - p4_open[1] absorbed the transcript's SUMMARY line and is cut off mid-word - re-extract byte-exact from the r7 FINDINGS line and refuse claims containing SUMMARY: | [P4] [composition] docs/spec/SMP1.md:75-81, ADR-0032:25-29 vs SLEY2_TRIAL_RUNNER_V1.md:51,331 and check_sley2_trial_runner.py:155 - SMP1/ADR say consumers keep revision 15 pins; S20-620 pins 16 under a second (document-revision) rule, the exception recorded only in the gate record; :51 says revision 16 defines the response - name the exception in SMP1/ADR-0032, or pin normative 15 and cite the revision 16 erratum | [P4] [test-coverage] crates/sley-protocol/src/server.rs:2931-2935 (with SLEY2_TRIAL_RUNNER_V1.md:338-342) - the corrected absent-boundary clause has no test; maintenance_guard pre-initializes - server test deleting locks/ then workspace.open succeeds and recreates the boundary | [P4] [record] evidence/review/finding-register.json obligations[454] (build_finding_register.py:183-208; gate record :1005) - the delta_2b0f1c9 rename makes the register fold vulcan revision_5 REVISE as superseded by the 2b0f1c9 verdict (it left open_reviews); Ariadne/Nabu revision 5 stay open; not recorded - rename so the register reads it as round-scoped historical, or record the supersession against revision_6 via S20-740, and note it in the gate record | [P4] [ownership] bench/live/sley2_tool.py:269-286,306-314 (with bench/sley2/runner.py:346-422) - carried, OPEN: _abandon writes into runner-owned Endpoint internals - owner-side Endpoint.abort() with an offline test, or fail loudly | [P4] [record] bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:960,1006 - carried, OPEN: task-input amendment (REQ-10-rev2:49-51) unratified; stand-in fails closed - corpus-owner ratification
SUMMARY: My revision-7 P2 is closed: the completion gate refuses the 2b0f1c9 verdict under the revision-7 name, fails closed on unscoped or unknown scopes, has a two-direction regression test, and keeps the historical verdict byte-exact under a name that binds nothing; the absent-boundary correction matches the server code for workspace.open. I could still satisfy the gate with a mis-scoped PASS, because the scope comes from a hand-written note rather than from the transcript or round index (P3). The P4s cover binding by revision number rather than text, a corrupted open-claim record, two SMP1 pin semantics, a missing test for the corrected clause, a register side effect of the field move, and two carried items. PASS: this lane accepts contract revision 8.
