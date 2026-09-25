<!-- engine: claude-code; observed model: claude-opus-5-5; scope: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e; role: vulcan; field: vulcan_review_closure; dispatched: 2026-09-23T05:54:23Z; duration_s: 717; process_exit_code: 0 -->
# Vulcan Council review — s20_700_remaining_surface_audit
Harness: claude-code
Reviewed checkpoint: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e

This is a delta review at d158d26b. I made no edits to the tree.

**The shell was unavailable, so nothing was executed.** Every Bash call failed before it ran, with `ENOSPC: no space left on device, mkdir '/tmp/agent-scratch'` (the harness's `/tmp` is out of space). One retry returned "This command requires approval". As a result:
- I ran no `git log`/`git diff`, no checker, no unittest suite, no `retire_review_claims.py --check` and no cargo test. I have no exit codes or result lines to report.
- This verdict rests only on static reading of the HEAD tree with Read/Grep: code, tests, spec, ledger, register and transcripts.
- Anything below described as "pinned by test X" means I read the test source. I did not see it pass.

**Scope check.**
- The worktree's `.git` file points to `/home/dev/Work/workspaces/sley2/.git/worktrees/wt`.
- That directory's `HEAD` file reads `d158d26bfdda9fc7dc899a1d1a8a141c2da6610e`, so the scope matches.
- I could not list `8966da2e..HEAD` or diff it. I reviewed the HEAD state of the section's owned paths and used the in-code round citations ("8966da2e round, second batch") to locate the repairs.
- The REQ-11 product change (crates, test selection) is outside this lane and I did not review it.
- The untracked d158d26 verdicts in `git status` were there before I started. Some Grep output surfaced lines from other lanes' untracked d158d26 transcripts; I did not rely on them.

**Files read:**
- `vulcan_review_closure-8966da2.md` 1-145
- `vulcan_review_closure-8f774d0.md` 28-31
- `docs/spec/FINDING_REGISTER_V1.md` 60-239
- `scripts/build_finding_register.py` 540-1739
- `scripts/retire_review_claims.py` 85-100 and 216-675
- `scripts/fuzz_proof_record.py` 70-183
- `scripts/check_pack_persistent_fuzz_slice.py` 145-179
- Grep of `check_semantic_checkers_persistent_fuzz_slice.py` (the `still_crashes`/regression/pin lines)
- `bench/review/tests/test_retire_review_claims.py` 1-75 and 318-793
- `bench/review/tests/test_finding_register.py` 539-608, plus Grep hits at 391-394, 498-502 and 994-998
- Grep of `bench/review/tests/test_fuzz_proof_record.py` 82-100
- `machineresearch/sley-2.0/machine-summary.json`:
  - 3972-4255 (this section)
  - 3160-3247 (the semantic-checkers proof)
  - 5300-5400 and 7096-7190 (packaging and standards re-statements)
  - Grep of every `restates` link naming `is_closure_line`/`line_speaks_about`, and of the ten previously mis-linked claims
- `evidence/review/claim-retirements.json` 2045-2064, plus a Grep for this section
- `evidence/review/finding-register.json` 8553-8790 (Grep)
- `evidence/release/ga-acceptance-report.json`:247

## Evidence checked

**How the repaired fold works now (static trace):**
- **Named re-statements key on their own identifier.** `finding_key` (`build_finding_register.py:1508-1527`) gives a named re-statement the identifier from `_own_identifier`, or `""`, never `None`. `same_finding` is plain key equality (`:1537`).
- **A fold needs the named round.** `fold_restatements` (`retire_review_claims.py:434-444`) folds only into the single same-key claim whose scope equals the named sha. If none matches, or two do, the claim stays open.
- **A fold into a retired root needs the retirement read.** `closing_lines_name` (`:397-425`) runs `speaking_lines`, which applies the strong read when the kind is shared, the `shared` set and the OPEN-line refusal. The view excludes only the linked retired item. The closer must also strictly postdate the re-statement.
- **Quoted text is not a carry.** `unquoted` (`build_finding_register.py:1540-1546`) drops backticked and double-quoted spans before `CARRIED_FROM` runs (`:1520,1559,1570`). `LEADING_CARRY` (`:1482`) needs a sha.
- **The builder replays the named-sha rule.** See `:1657-1674`.
- **An absent identifier is not an identity.**
  - Shared-vocabulary exemption: `:927-929`.
  - Pass-2 inheritance: `retire_review_claims.py:475`.
  - Split-status refusal: `:565`.
  - `package_open_findings`: `build_finding_register.py:1728`.
- **Tagged claims with no raising line are refused** in all three lists (`:710,1377,1630`).

**The ten wrong links from my 8966da2 P2 are gone:**
- No `restates` target in the ledger names `is_closure_line`.
- The one link that names `line_speaks_about` is repository_exchange `…@6589c6e` → `…@1a9f0aa` (`machine-summary.json:1783-1784`). It names 1a9f0aab, carries the same identifier, and its target is at that scope.
- The claims I traced now sit under their true roots:
  - Ariadne `scoped_before` and `is_lane_field_name` → `revision_8@79fdcc6` (`:5323-5328,5367-5372`).
  - Ariadne `claim_finding_ids` → `revision_9@b58ac1e` (`:5335-5336,5379-5380`).
  - Nabu `is_open_line` and `raising_scope` → `nabu@b58ac1e` (`:5343-5348,5383-5388`).
  - Vulcan standards `claim_relation_problem` and the line-span `PATH_TOKEN` claim chain through 8f774d0 to their b58ac1e roots (`:7126-7131,7150-7151`).

**Fold-side tests now exist in source** (`test_retire_review_claims.py`):
- named-sha root (:318-330)
- absent named root stays open (:332-340)
- closed twin (:342-360)
- postdating closer (:362-387)
- quoted carry (:652-672)
- a distinct named re-statement never folds into a closed sibling (:674-687)
- standards fixture (:689-706)
- Nabu fixtures (:708-725)
- exact-key inheritance needs an identifier and a naming line (:727-752)
- inheritance honours the named round (:754-776)
- ambiguous root (:778-788)
- generic head under carried and identifier-less shapes, including "identifier-less leading carries" → 0 retired (:625-650)

**Per-finding status (the 8966da2e findings):**
- **[P2] [gate] `same_finding` / `is_carry` named-carry wildcard (build_finding_register.py:1307,1317,1330,1339,1348 and build_finding_register.py:1322,1330,1342,1352; retire_review_claims.py:360-378 and retire_review_claims.py:373-377,382-390) — CLOSED.** Evidence:
  - A named re-statement keys on its own identifier (`:1514,1525`), and `same_finding` is equality (`:1537`).
  - The root must sit at the named sha, exactly once (`retire_review_claims.py:438-444`). The builder replays this (`build_finding_register.py:1668-1674`).
  - A quoted or backticked `carried from <sha>` is not a carry (`:1540-1559`, test :652-672).
  - The retired-root guard uses the retirement read, the `shared` set and postdating (`retire_review_claims.py:397-425,447`).
  - Unknown scopes can no longer win the root: selection is by equality, and `scope_generation` is not used.
  - The ten links are re-listed under their true roots, as above.
  - The residual, an unquoted mid-prose parenthetical, is filed as a new P4 below.
- **[P3] [gate] `shared_vocabulary` exemption for named re-statements keyed `(lane, kind, file, None)` (build_finding_register.py:833,1330) — CLOSED.** The exemption now needs equal keys, a non-empty identifier and a carry (`:927-929`). Distinct named carries key on distinct identifiers. The shape is pinned by `test_generic_head_under_carried_and_identifier_less_shapes` (:625-650), where two identifier-less leading carries retire 0 and two distinct carries retire only the one the identifier head names. A residual that reaches the same shape through a junk token (rather than an absent identifier) is filed as a new P4 below.
- **[P4] [gate] `claim_finding_ids` whole-id split (build_finding_register.py:780-781,1104; build_finding_register.py:780-781,1095) — CLOSED.** The id regex at `:866` now matches multi-segment ids, and the document-id filter at `:867` is a full match. Test `test_8966da2_predicates_guards_and_refusals` (:548-552) asserts `S20-700-PACK-001` → itself, `RW090-DEV-01`, `V-02`, and that `ADR-0040`/`S20-540` → nothing.
- **[P4] [gate] `replay_problems` exact-claim branch (retire_review_claims.py:623-626) — OPEN.** Unchanged in mechanism. It is covered elsewhere (builder `:1444-1448`; `retire()` at `retire_review_claims.py:292-293` via `regeneration_divergence` at `:662`). Re-filed below.
- **[P4] [gate] description key on a path-first `rest` (build_finding_register.py:1520-1524) — OPEN.** Same slicing, no test. It is latent. Re-filed below.
- **[P4] [gate] open-line head read without `shared` (`open_lines_about`, `paths_ok`; build_finding_register.py:847-868, build_finding_register.py:838-860, build_finding_register.py:763-785) — CLOSED.** The shared vocabulary is now passed (`:971-974`), and the head is reduced to kinds and path tokens (`:970`). Test: `test_open_check_bounded_head_and_shared` (`test_finding_register.py:600`).
- **[P4] [gate] `strictly_later_scope` equal ids and `reopen_all` (build_finding_register.py:935-946,1384-1389; build_finding_register.py:868-879,1272-1277) — CLOSED.** Equal ids are refused at `:1055` (test :542), and `reopen_all` returns re-statements (`retire_review_claims.py:495-513`).
- **[P4] [gate] prefix scope ids in the builder (build_finding_register.py:944-957,1432) — OPEN, narrowed.** `:1055` refuses only string-equal ids. `retire.strictly_later` refuses prefixes (`retire_review_claims.py:87`). Re-filed below.
- **[P4] [gate] `is_tracked` fail-open (build_finding_register.py:1151-1162, build_finding_register.py:1142-1153, build_finding_register.py:1136-1147, build_finding_register.py:1040-1051, build_finding_register.py:879-890) — CLOSED.** A non-zero `git ls-files` now gives an empty set (`:1319`); test at `test_finding_register.py:592-598`. The remaining `OSError` return (`:1317-1318`) needs git to be absent. In that case every ancestry query (`:551`, `:1061`, `retire_review_claims.py:92`) raises uncaught, so the run halts rather than passing.
- **[P4] [gate] V-02 regression pin residual (check_pack_persistent_fuzz_slice.py:157-172, check_semantic_checkers_persistent_fuzz_slice.py:254-266, fuzz_proof_record.py:80-84,101-158) — OPEN, narrowed.**
  - Repaired: `still_crashes` and `regression_records` are now read (`fuzz_proof_record.py:142-159`, test `test_fuzz_proof_record.py:82-100`).
  - Not repaired: the per-target `targets.<name>.retested_regressions` lists and the dict-shaped `retested_prior_crashes` of the semantic-checkers proof (`machine-summary.json:3176-3187,3211-3216,3240-3243`) are never walked.
  - Record values are still presence-checked only, and `lane_paths` (`:80-84`) still omits `fuzz/regressions/`.
- **[P4] [evidence] over-open ledger (machine-summary.json s20_700 p2/p3/p4 open lists) — OPEN, narrowed.**
  - The per-round-duplicate leg is now documented: the open list is a claim count, and `package_open_findings` reports distinct keys. At HEAD the section has 2/3/47 claims and 2/3/30 distinct (`finding-register.json:8673-8678,8788-8790`). The GA row states both counts (`ga-acceptance-report.json:247`).
  - Still open: the thinned-record (db53894) and Makefile (7622776) P3s and the chronology P4s. The lane stated these closed at 8f774d0c, but no exact-claim entry was added. I did not re-verify them at HEAD.
- **[P4] [gate] cell parenthetical case (build_finding_register.py:712) — CLOSED.** The cell rule at `:791` now refuses any case of the word, and the lowercase cell is pinned at `test_finding_register.py:997`. The bold-head shape named in my b58ac1e0 copy of this claim is not repaired and is re-filed as a new P4.
- **[P4] [gate] `raising_scope` 80-character prefix (build_finding_register.py:574,588) — CLOSED.** The full carry-stripped description is tried first (`:592-599`). The prefix is only a fallback for older truncated rounds. Test :558-563. Fold roots no longer use `scope_generation` (`retire_review_claims.py:438-444`).

**My own probes (static, derived rather than executed):**
- **(a) V-02 per-target shape.** `proof_record_problems` reads only top-level `retested_regressions` (`:155`). The semantic-checkers proof keeps them under `targets`, and its top-level `retested_prior_crashes` is a dict, which the guard at `:157` skips. So `targets["graph-cfg"]["retested_regressions"][0]["still_crashes"] = true` on a PASS record reaches no refusal: the slice checker only presence-checks the record files (`:251-266`). The runner itself refuses such a PASS (`run_semantic_checkers_persistent_fuzz.py:344`), so only a hand-edited record slips through.
- **(b) Junk identifiers.** `_own_identifier` takes the first word of any backticked span. The 8966da2 P2 claim (`machine-summary.json:4030`) keys `carried`, from its quoted `carried from <sha>`, while its 8f774d0 root keys `same_finding`. So the carry does not fold: both P2s stay listed and are counted as 2 distinct. That is the safe direction. Two named carries whose first backticked token is the same prose word would key equal and non-empty, which is the closed P3's shape reached by a token instead of an absence.
- **(c) A mid-prose parenthetical still makes a carry.** Standards `…revision_11@8f774d0 [contract-precision]` (`:6888`, marked "fresh") contains an unquoted parenthetical `(carried from 6589c6ec)` that describes another claim. `is_carry` reads it as a named carry of 6589c6e. It is the fold root of `:7122-7123`. No wrong link results, because no same-key claim exists at 6589c6e.
- **(d) The builder's inheritance replay is weaker than the generator.** For an exact-key inheritance, the builder (`:1661-1678`) checks only key, identifier and that the target is retired. It does not check that the closing line names the inheriting claim, or that the closer postdates it. `--check` regeneration catches a hand-added link.
- **(e) Bold-head parenthetical.** `- **[P3] x — CLOSED (leg 2 OPEN).**` is a closure line: rule (i) at `:779-781` accepts the head, and `STATUS_OPEN` (`:755`) needs a punctuation mark before the word.
- **(f) Exact-claim binding.** The 8f774d0.md#L30 binding (`claim-retirements.json:2052-2064`) is consistent with the code. Finding ids are now shared vocabulary (`identity_tokens:882`, `line_speaks_about:1250`), which closes the leg the 79fdcc6 claim named.

## Findings

[P4] [gate] scripts/check_pack_persistent_fuzz_slice.py:158-173, scripts/check_semantic_checkers_persistent_fuzz_slice.py:251-266, scripts/fuzz_proof_record.py:80-84,142-159 - `still_crashes` is now read and `regression_records` must name existing files, but only top-level lists are walked: the semantic-checkers proof keeps `retested_regressions` under `targets.<name>` (machine-summary.json:3176-3187,3211-3216) and its dict-shaped `retested_prior_crashes` (:3240-3243) is skipped by the list guard, so a PASS record with a per-target `still_crashes: true` passes both checkers; record values are still presence-checked only, `fuzz/regressions/` is still not a lane path, and `is_file()` is not a tracked-file check; the only test (bench/review/tests/test_fuzz_proof_record.py:82-100) uses top-level lists - carried from 8966da2e, narrowed - closure evidence needed: walk per-target and dict-shaped retest lists (refuse unknown shapes), pin id/target/classification and a hex digest per record, add `fuzz/regressions/` to the declaring lanes, and test the per-target shape
[P4] [gate] scripts/retire_review_claims.py:621-626 - `replay_problems`' exact-claim branch still does not apply the OPEN-line refusal (`recorded` at :621 is unused there); the builder (scripts/build_finding_register.py:1444-1448) and `retire()` (scripts/retire_review_claims.py:292-293, re-run by `regeneration_divergence` at :662) refuse the shape, so it is caught elsewhere - carried from 8966da2e, unchanged - closure evidence needed: apply `open_lines_about` in the branch and extend `test_replay_exact_claim_branch` with a transcript that records the claim open
[P4] [gate] scripts/build_finding_register.py:1520-1524 - the description key still slices a path-first `rest` with no dash separator into a `desc:` path key; latent (reached only by an origin-less ledger-anchored claim) and untested - carried from 8966da2e, unchanged - closure evidence needed: strip the leading anchor before slicing, or refuse an origin-less ledger claim; test the path-first shape
[P4] [gate] scripts/build_finding_register.py:1051-1056,1684-1685 - the `reopen_all` and equal-id legs are closed, but `strictly_later_scope` refuses only string-equal ids, so a prefix pair (a `@76ae15ab` claim restating a `@76ae15a` claim, one commit) reads strictly later in `package_restated_claims` while `strictly_later` (scripts/retire_review_claims.py:87) refuses prefixes; only `--check` regeneration catches such a hand-added fold - carried from 8966da2e, narrowed - closure evidence needed: refuse prefix ids in `strictly_later_scope` as `strictly_later` does; test
[P4] [evidence] machineresearch/sley-2.0/machine-summary.json:4028-4088 (s20_700_remaining_surface_audit p2_open/p3_open/p4_open) - over-open ledger, narrowed: one listing per round is now the documented claim count beside `package_open_findings` (evidence/review/finding-register.json:8673-8678 vs 8788-8790, 2/3/47 claims and 2/3/30 distinct; the GA row states both at evidence/release/ga-acceptance-report.json:247), but the thinned-record db53894 and Makefile 7622776 P3s and the chronology P4s the lane stated closed at 8f774d0c are still listed with no exact-claim entry (the only later s20_700 entry is evidence/review/claim-retirements.json:2052-2064) - closure evidence needed: re-verified per-finding closure lines or exact-claim entries for those claims
[P4] [gate] scripts/build_finding_register.py:1454,1457-1475,1514,1525 - `_own_identifier` takes the first word of any backticked span as the finding's identity, quoted prose included: the section's second P2 claim (machine-summary.json:4030) keys `carried` from its quoted carry phrase while the root it names keys `same_finding`, so the named carry cannot fold and the register counts two P2 findings for one (finding-register.json:8788, safe over-count); two named carries on one file whose first backticked word is the same prose token (`None`, `carried`) key equal and non-empty, so `shared_vocabulary` (:927-929) exempts each from the other and a generic kind-and-path closure head retires both; latent - closure evidence needed: take the identifier only from a backticked span that is itself an identifier (no spaces or punctuation), skip quoted carry phrases, and test both shapes
[P4] [gate] scripts/build_finding_register.py:1481,1540-1580 - `is_carry` and `named_carry_sha` still read an unquoted mid-prose parenthetical that describes another claim's carry as the claim's own: the standards claim `vulcan_surface_review_revision_11@8f774d0 [contract-precision]` (machine-summary.json:6888, marked fresh) cites `(carried from 6589c6ec)` for a different claim, is read as a named carry of 6589c6e, and is the live fold root at :7123; no wrong link results today because the fold also needs a same-key claim at the named round - closure evidence needed: take the named sha only from a leading clause or a trailing status position, and test a mid-prose parenthetical naming another claim
[P4] [gate] scripts/build_finding_register.py:1657-1689 - `package_restated_claims` replays an exact-key inheritance by key, identifier and retired target only; it checks neither that the retired claim's cited closing lines speak about the inheriting claim nor that the closer strictly postdates it (the generator's `closing_lines_name`, scripts/retire_review_claims.py:397-425), although docs/spec/FINDING_REGISTER_V1.md:164-166,187-189 says the ledger replays the fold's rule, so a hand-added inheritance into a retired same-key claim passes `check_finding_register.py` and is caught only by `--check` regeneration - closure evidence needed: apply the naming and postdating rules in the builder's replay and test a hand-added inheritance
[P4] [gate] scripts/build_finding_register.py:755,779-781,807-808 - the parenthetical refusal covers only a table cell (:791, test bench/review/tests/test_finding_register.py:997): a leading bold head `- **[P3] x — CLOSED (leg 2 OPEN).**`, upper or lower case, is still a P3 closure line because rule (i) accepts any marker inside the head and `STATUS_OPEN` needs a punctuation mark before the word; this is the second shape of my b58ac1e0 claim (machine-summary.json:4056) - closure evidence needed: refuse an open qualifier in a bold-head parenthetical as the cell rule does, and test both cases

## Assessment

Both findings from my 8966da2 verdict above P4 are closed, judged on the HEAD source:
- **The P2 (named-carry wildcard).** Named re-statements key on their own identifier. A fold resolves only to the single same-key claim at the round the carry names. Quoted carry phrases are not carries. The retired-root guard uses the retirement read, the `shared` set and postdating. The builder replays the named-sha rule. The ten live wrong links are gone, and the claims now sit under their true roots.
- **The P3 (shared-vocabulary exemption).** The exemption needs a non-empty identifier, and the identifier-less-carries shape is pinned.

Of the ten P4s:
- **Closed (5):** `claim_finding_ids`, the OPEN-head `shared` read, `is_tracked`, the cell parenthetical, and `raising_scope`. Each has code and a test in source.
- **Narrowed (3):**
  - `strictly_later_scope`: equal ids and `reopen_all` closed; the prefix leg is open.
  - V-02: `still_crashes` is read, but not for the per-target shape.
  - Over-open ledger: the claim count is now documented, but the stated-closed claims are still listed.
- **Unchanged (2):** the `replay_problems` exact-claim branch and the description key.

My probes found four new P4s:
- a junk first-token identifier;
- a mid-prose carry parenthetical still read as the claim's carry;
- the builder's inheritance replay is weaker than the generator's;
- the bold-head parenthetical is still a closure line.

None of them produces a wrong link or a false closure in the HEAD ledger. The ledger-shaped ones are caught by `--check` regeneration, or fail in the over-open direction.

That is 0 P0–P3 and 9 P4, so this is a PASS. It is not closable as `PRIOR_P3_P4_CLOSED`, because five of my prior P4s stay open.

The main caveat: this PASS is not backed by any executed run. I could not run a checker, the unittest suites or `--check` in this session (harness ENOSPC), and I did not see the `8966da2e..HEAD` diff. A run of the three section checkers, `check_finding_register.py`, `retire_review_claims.py --check` and `python3 -m unittest discover -s bench/review/tests -t .` at d158d26b should be recorded before this verdict is relied on.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_9_P4
SECTION: s20_700_remaining_surface_audit
FIELD: vulcan_review_closure
SCOPE_SHA: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e
FINDINGS: [P4] [gate] scripts/check_pack_persistent_fuzz_slice.py:158-173, scripts/check_semantic_checkers_persistent_fuzz_slice.py:251-266, scripts/fuzz_proof_record.py:80-84,142-159 - `still_crashes` is now read and `regression_records` must name existing files, but only top-level lists are walked: the semantic-checkers proof keeps `retested_regressions` under `targets.<name>` (machine-summary.json:3176-3187,3211-3216) and its dict-shaped `retested_prior_crashes` (:3240-3243) is skipped by the list guard, so a PASS record with a per-target `still_crashes: true` passes both checkers; record values are still presence-checked only, `fuzz/regressions/` is still not a lane path, and `is_file()` is not a tracked-file check; the only test (bench/review/tests/test_fuzz_proof_record.py:82-100) uses top-level lists - carried from 8966da2e, narrowed - closure evidence needed: walk per-target and dict-shaped retest lists (refuse unknown shapes), pin id/target/classification and a hex digest per record, add `fuzz/regressions/` to the declaring lanes, and test the per-target shape
[P4] [gate] scripts/retire_review_claims.py:621-626 - `replay_problems`' exact-claim branch still does not apply the OPEN-line refusal (`recorded` at :621 is unused there); the builder (scripts/build_finding_register.py:1444-1448) and `retire()` (scripts/retire_review_claims.py:292-293, re-run by `regeneration_divergence` at :662) refuse the shape, so it is caught elsewhere - carried from 8966da2e, unchanged - closure evidence needed: apply `open_lines_about` in the branch and extend `test_replay_exact_claim_branch` with a transcript that records the claim open
[P4] [gate] scripts/build_finding_register.py:1520-1524 - the description key still slices a path-first `rest` with no dash separator into a `desc:` path key; latent (reached only by an origin-less ledger-anchored claim) and untested - carried from 8966da2e, unchanged - closure evidence needed: strip the leading anchor before slicing, or refuse an origin-less ledger claim; test the path-first shape
[P4] [gate] scripts/build_finding_register.py:1051-1056,1684-1685 - the `reopen_all` and equal-id legs are closed, but `strictly_later_scope` refuses only string-equal ids, so a prefix pair (a `@76ae15ab` claim restating a `@76ae15a` claim, one commit) reads strictly later in `package_restated_claims` while `strictly_later` (scripts/retire_review_claims.py:87) refuses prefixes; only `--check` regeneration catches such a hand-added fold - carried from 8966da2e, narrowed - closure evidence needed: refuse prefix ids in `strictly_later_scope` as `strictly_later` does; test
[P4] [evidence] machineresearch/sley-2.0/machine-summary.json:4028-4088 (s20_700_remaining_surface_audit p2_open/p3_open/p4_open) - over-open ledger, narrowed: one listing per round is now the documented claim count beside `package_open_findings` (evidence/review/finding-register.json:8673-8678 vs 8788-8790, 2/3/47 claims and 2/3/30 distinct; the GA row states both at evidence/release/ga-acceptance-report.json:247), but the thinned-record db53894 and Makefile 7622776 P3s and the chronology P4s the lane stated closed at 8f774d0c are still listed with no exact-claim entry (the only later s20_700 entry is evidence/review/claim-retirements.json:2052-2064) - closure evidence needed: re-verified per-finding closure lines or exact-claim entries for those claims
[P4] [gate] scripts/build_finding_register.py:1454,1457-1475,1514,1525 - `_own_identifier` takes the first word of any backticked span as the finding's identity, quoted prose included: the section's second P2 claim (machine-summary.json:4030) keys `carried` from its quoted carry phrase while the root it names keys `same_finding`, so the named carry cannot fold and the register counts two P2 findings for one (finding-register.json:8788, safe over-count); two named carries on one file whose first backticked word is the same prose token (`None`, `carried`) key equal and non-empty, so `shared_vocabulary` (:927-929) exempts each from the other and a generic kind-and-path closure head retires both; latent - closure evidence needed: take the identifier only from a backticked span that is itself an identifier (no spaces or punctuation), skip quoted carry phrases, and test both shapes
[P4] [gate] scripts/build_finding_register.py:1481,1540-1580 - `is_carry` and `named_carry_sha` still read an unquoted mid-prose parenthetical that describes another claim's carry as the claim's own: the standards claim `vulcan_surface_review_revision_11@8f774d0 [contract-precision]` (machine-summary.json:6888, marked fresh) cites `(carried from 6589c6ec)` for a different claim, is read as a named carry of 6589c6e, and is the live fold root at :7123; no wrong link results today because the fold also needs a same-key claim at the named round - closure evidence needed: take the named sha only from a leading clause or a trailing status position, and test a mid-prose parenthetical naming another claim
[P4] [gate] scripts/build_finding_register.py:1657-1689 - `package_restated_claims` replays an exact-key inheritance by key, identifier and retired target only; it checks neither that the retired claim's cited closing lines speak about the inheriting claim nor that the closer strictly postdates it (the generator's `closing_lines_name`, scripts/retire_review_claims.py:397-425), although docs/spec/FINDING_REGISTER_V1.md:164-166,187-189 says the ledger replays the fold's rule, so a hand-added inheritance into a retired same-key claim passes `check_finding_register.py` and is caught only by `--check` regeneration - closure evidence needed: apply the naming and postdating rules in the builder's replay and test a hand-added inheritance
[P4] [gate] scripts/build_finding_register.py:755,779-781,807-808 - the parenthetical refusal covers only a table cell (:791, test bench/review/tests/test_finding_register.py:997): a leading bold head `- **[P3] x — CLOSED (leg 2 OPEN).**`, upper or lower case, is still a P3 closure line because rule (i) accepts any marker inside the head and `STATUS_OPEN` needs a punctuation mark before the word; this is the second shape of my b58ac1e0 claim (machine-summary.json:4056) - closure evidence needed: refuse an open qualifier in a bold-head parenthetical as the cell rule does, and test both cases
SUMMARY: Static review of HEAD d158d26b; no checker, test suite or git diff could be run because every shell call failed with ENOSPC in the harness temp directory, so this verdict rests on reading code, tests, spec, ledger and register. My 8966da2 P2 (named-carry wildcard) and P3 (shared-vocabulary exemption) are CLOSED: named carries key on their own identifier and fold only at the named round, quoted carries are ignored, the retired-root guard uses the retirement read with postdating, the ten wrong links sit under their true roots, and the shapes are pinned in tests. Of the ten P4s, five are closed, three narrowed and two unchanged, and four new latent P4s were found; none produces a false closure or wrong link in the HEAD ledger, so PASS with nine P4s, not closable as PRIOR_P3_P4_CLOSED.
