Verification complete. All claims checked against `95d42fda` directly; read-only throughout (blobs extracted via `git show` into `~/.hermes/cache/scratch/`, no repo writes, no test execution).

═══════════════════════════════════════════
VERDICT
═══════════════════════════════════════════

SECTION: context_bounded_discovery
FIELD:   nabu_architecture_review
BASELINE: 95d42fda691bc5d8ec468478529b7b174d3a8759 (verified = branch tip of work/succession-sley20-arm)

VERDICT: REVISE_0_P0_1_P1_2_P2_1_P3_0_P4

Architecture remains settled. No redesign requested, none warranted. Every rev6 fix is correctly aimed, and the packet's factual claims are unusually accurate — but two of the five fixes do not work as written, and I verified that mechanically rather than by reading.

───────────────────────────────────────────
[P1] Fix 1 step 2 places `section` 16 lines before it is bound — the drift gate NameErrors on every `make lint`

Fix 1.1 pins the extraction "immediately after `spec = read(SPEC)` (~:155, indent 4, above and outside the `if status in IMPLEMENTATION_STATUSES:` block". Fix 1.2 then puts the drift assertion "same indent-4 region, directly after the extraction" — i.e. ~:157 — and it reads `section.get("contract_revision")`.

AST binding scan of `main()` at baseline:

    spec      first bound  155
    summary   first bound  173
    section   first bound  174  (also 177)
    status    first bound  178

`section` does not exist at :157. The drift assertion as specified raises `NameError` unconditionally, on every invocation, before any verdict logic runs. This is not a latent path — `check_sley2_trial_runner.py` runs in `make lint`, so the fix intended to make the gate fire instead makes the gate crash.

The precedent Fix 1 cites gets this right and the packet transcribed the wrong half of it. At `check_root_backed_query_profile.py`: `section` at :209, extraction at :214, `summary_revision` at :218 — extraction sits *after* the section binding, not after `spec`.

This also falsifies Fix 2's P2-1 claim in the same packet: "no gate references a variable defined below it." As written, it does.

Remedy: the hoist target is after :178 (`status = section.get("status")`) and before :201, still indent 4, still outside the `IMPLEMENTATION_STATUSES` block. That satisfies every property Fix 1 actually argues for — single extraction, indent 4, above the block, feeding :286-289 and :291 — without the ordering fault. `re` is already imported (:8); no new import.

───────────────────────────────────────────
[P2-1] Fix 4's "exact edits" omit the allowlist edit, so the new `open` command fails closed at the :322 guard

Fix 4.1 routes the agent `open` through `_view(session, "workspace.open", "")` and correctly states that `_view` → `session.call` → `_request` means "the `:322` dispatcher guard ... applies". Verified: `_view` at :390 calls `session.call` at :395; `_request` at :321-324 is `if method not in TOOL_METHODS: _fail("method")`.

`TOOL_METHODS` at :88 enumerates 18 names. `workspace.open` is **not** among them (`revision.read` is, at index 16). So the guard the packet correctly identifies as applying is precisely the guard that rejects the new command. Every agent `open` invocation returns `_fail("method")`.

The reason this is currently invisible: `workspace.open` is reachable exactly once in the module, at :261 inside `_read_head`, via `_raw_request` — which bypasses the guard. The harness's privileged path never needed the allowlist entry; the agent path does.

The packet's acceptance clause already says "`workspace.open` (allowed, 19-name surface)", so intent is clear and this is an omission from the enumerated edit list, not a contradiction. But Fix 4 is billed as the *named mechanism* with "Exact edits", and an implementer following items 1-3 literally ships a dead command.

Remedy: add `workspace.open` to `TOOL_METHODS` (:88) as edit 4.0, and carry the D1 docstring correction (:10, :84 — both read "eighteen-method") into the same commit, since this edit is what makes them stale. That folds carried-P4 D1 into the change that causes it rather than leaving it to drift again. Note the pin is documentation-only: `TOOL_METHODS` appears in 0 of the 26 files under `bench/live/tests/` (verified per-file), so no test blocks the widening — which is B4's premise, confirmed.

───────────────────────────────────────────
[P2-2] Fix 2.1 requires a `_revision_4_note` naming a rev4 transcript that does not exist for any sley2_trial_runner lane

Fix 2.1 mandates `<base>_revision_4_note` "naming the rev4 transcript and commit, on the rbqp `_note` shape". Verified at baseline:

  - no `evidence/review/verdicts/sley2_trial_runner/` directory exists (52 verdict dirs enumerated; absent)
  - no transcript filename anywhere matches `sley2_trial|trial_runner`
  - the only files mentioning `sley2_trial_runner` under `verdicts/` are the context_bounded_discovery rev4/rev5 transcripts — this REQ's own records, which are not the lanes' rev4 verdicts

The three lanes carry `PASS_0_P0_0_P1_0_P2_0_P3` in the summary with no transcript behind them. The instruction is therefore unexecutable as written, and its most likely failure mode is an implementer synthesising a plausible path to satisfy the template — the exact defect class this packet exists to eliminate.

Remedy: state the ground truth. Either the `_revision_4_note` records that the rev4 lane verdicts predate transcript capture (naming the commit only, no transcript clause), or the field is not written for revision 4 and binding starts at revision 5, where transcripts do exist. Both are defensible; inventing a path is not.

───────────────────────────────────────────
[P3] The quoted `_note` template does not satisfy the register's own scope regex

Fix 2.1 pins the note shape as `Historical ... verdict at <sha>; ... Transcript: <path>`, quoting rbqp. The finding register derives a frozen `_revision_N` field's scope from that note at `build_finding_register.py:604-607`, via `NOTE_SCOPE = re.compile(r"\bon ([0-9a-f]{40})\b")` (:540).

The template as quoted uses "**at** `<sha>`". `NOTE_SCOPE` requires the literal word "**on**" followed by a full 40-hex sha. The rbqp note the packet copied from is `nabu_architecture_review_revision_3_note` = "Historical independent verdict **at c1d4177**; transcript ..." — 7-hex, wrong preposition, and it yields `scope = None`. The sibling `_revision_4_note` ("...**at** e050fe75a86c...40 hex") also fails, on the preposition alone.

So a note written exactly to the quoted template is scope-less to the register. Given that Fix 2 justifies itself by adopting "the tree's existing convention verified this round", the convention should be adopted in the form the consuming code actually parses — `on <40-hex>` — not in the form that happens to appear in a field whose scope resolution silently degrades. Low severity: it affects register scope attribution, not a gate PASS/FAIL, and the GA predicate is unaffected.

───────────────────────────────────────────
CONFIRMED CORRECT (no action)

Verified exact, in the tree, this round:

  - Fix 1.1 anchor/precedent: `:155`, block opens `:201`, bare `re.search(r"revision (\d+)", spec)` at `:291`; precedent `check_root_backed_query_profile.py:214` matches the cited form verbatim. Spec Status line is `SLEY2_TRIAL_RUNNER_V1.md:3`, "revision 4"; the preamble does carry "revision 1", "Revision 2/3/4" in prose at :5-8. Both regexes happen to return 4 here, so the anchoring is correctness-for-the-future, not a live bug — the packet's P3-1 framing is accurate.
  - `contract_revision = 4` is present in the summary section; the `int`/`int` drift comparison is type-correct (fires 4-vs-5, silent 5-vs-5).
  - Fix 2 convention: exactly **129** `*_review_revision_N` fields across the summary — the packet's figure is exact. rbqp carries `vulcan_surface_review_revision_5/6`, `ariadne_contract_review_revision_5`, `nabu_architecture_review_revision_5`. `is_lane_field_name` (:1085) / `verdict_field_names` (:1104) derive shape from the summary itself, never a fixed list — as claimed.
  - GA predicate at `build_ga_acceptance_report.py:81` is `^PASS_0_P0_0_P1_0_P2_0_P3(?:_0_P4)?$`, exact. Base-field values stay in grammar; the withdrawal of value-encoded `PASS_r5_...` is the right call — the register has no rule for it.
  - Fix 3: `WORK_PACKAGES.md:59` is present-tense and stale on all three axes ("revision 4, 2026-09-09", "Council reviews PASS", "`S20_620_COMPLETE` (2026-09-15)"). `WORK_PACKAGE_MARKERS` at :73 is only the spec path and "ADR-0036" — it genuinely does not pin the stale text, and the checker does read the file (:148, :166-169). Token-set widening is justified by count: `eighteen` = 2 hits and neither is the row; `Council reviews PASS` = 10, `revision 4` = 6. The `eighteen`-only grep could not have seen it. Correct.
  - Fix 3.3: `status_note` verbatim asserts "the WORK_PACKAGES row and closeout still read 'reviews pending'" while the row reads "Council reviews PASS". The stated correction is required and correctly scoped.
  - Fix 4 anchors: `dispatch` :703; `:717-718` is `if command == "revision" and not rest: ... session.head["tx"]`; guard :322; `_command_evidence` :968 with `side` at :1026-1027 and the `read`/`raw` cases as described. B1 consumers verified individually: `:381-382` (`head` property), `:424`, `:482`, `:751`, `:754-755`, `:876`, `mediated_sley.py:137` — all read `session.head`/`self._head`, populated by `_read_head` (:256-268) at construction (:229). Agent path never writes `self._head`; the separation claim holds. `:8` "No state survives across invocations" confirms the one-dispatch-per-invocation premise behind choosing (a) over (b). Option (a) is the right call.
  - Fix 5: `context_bounded_discovery` is absent from `machine-summary.json` — 0 occurrences as substring across the whole file, 114 top-level sections, no fuzzy match. Statement is exactly true and worth keeping.
  - D2: `95d42fda` subject line reads "Nabu REVISE 0P1/2P2/1P3" while the rev5 transcript line 4 reads `REVISE_0_P0_1_P1_2_P2_1_P3_1_P4` — the history/record discrepancy is real and correctly characterised as uncorrected-in-history.
  - D3: `verify_cached_snapshot` is at `crates/sley-repo/src/index_cache.rs:278`, exact.
  - Baseline delta: `git diff --name-status c1e8db56..95d42fda` = 3 added files (rev5 gate record, rev5 request, rev5 transcript), one commit. "Records only; no Rust sources, fuzz targets, or lint inputs changed" — verified true.

───────────────────────────────────────────
ASSUMPTIONS STATED

1. The rev6 packet (`2ba40e08…`) and rev6 transcript (`4f8de2f0…`) are **not** in the baseline tree or worktree — no file under `context_bounded_discovery/` hashes to either; the directory holds rev1-rev5 only. I did not treat this as a finding: the rev5 transcript was itself added *by* commit `95d42fda`, so in-flight round records landing with the round commit is the established pattern here. I therefore reviewed rev7 on its own terms against the baseline sources, and did not verify rev6's verdict text or its PACKET_SHA256 record. If the rev6 transcript is expected to already be committed, that is a record gap worth a separate check.
2. Downstream GA/dossier row ranges are carried from the rev6 verdict per the packet's own statement; I verified the `:81` predicate directly but did not re-read those rows.
3. Severity assignment is my architectural judgement, not gate output — no checker currently fails on any of these conditions.
4. No writes, no test execution, no service starts. Findings derive from `git show`/`git grep` against the commit plus AST and regex analysis of extracted copies outside the repo tree.

───────────────────────────────────────────
DISPOSITION

The architecture is settled and I am not reopening it. Fixes 3 and 5 are correct and complete as written. Fix 2's convention choice is right and its withdrawal of the value-encoded token is a genuine improvement. Fixes 1 and 4 are correctly reasoned but have transcription faults that make them non-functional as specified — the same defect class rev7 was convened to close, which is why this returns REVISE rather than PASS-with-notes. All four items are mechanical and none requires a design decision; the remedies are stated inline and land in the same edit sites the packet already names.
---
Writing-lane record (not reviewer text): the rev7 packet landed at
evidence/review/requests/REQ-10-rev7-context-bounded-discovery.md;
sha256sum
24d2b5056f14afe619737306a592c47071384ce11540439edddf47574f0bfe08.
Reviewer text above the delimiter is byte-verbatim from
resume-20260918/review-REQ10r7-nabu.log (dispatch exit 0, engine hermes).
Reviewer verdict: REVISE_0_P0_1_P1_2_P2_1_P3_0_P4.
