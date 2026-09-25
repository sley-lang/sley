<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 8966da2e0a52a6f6e56949e9a5902464c86613c0; role: vulcan; field: vulcan_review_closure; dispatched: 2026-09-23T03:06:06Z; duration_s: 435; process_exit_code: 0 -->
# Vulcan Council review — s20_700_remaining_surface_audit
Harness: claude-code
Reviewed checkpoint: 8966da2e0a52a6f6e56949e9a5902464c86613c0

This is a closure re-review of the twelve findings in `vulcan_review_closure-8f774d0.md` (1 P2, 1 P3, 10 P4), plus a fresh adversarial pass over the 69907ddf repairs (1)–(4). I used static tracing, the python3 checkers, python3 unittest, in-memory probes of `build_finding_register`/`retire_review_claims` against the real HEAD ledger and the tracked transcripts, and one `tempfile` git fixture outside the tree. That fixture copies `bench/review/tests/test_retire_review_claims.py:20-50`. No cargo, no fuzz binaries, no writes to the tree. The untracked `vulcan_review_closure-8966da2.md` was already in `git status` before I started; I did not read it or modify it.

What I ran:
- `git rev-parse HEAD` → `8966da2e0a52a6f6e56949e9a5902464c86613c0`, so scope matches. `git log --format='%h %s' 8f774d0c..HEAD` → 69907ddf (fix) and 8966da2e (records-only; 18 files, no scripts or tests). `git diff --stat 8f774d0c..HEAD` → 33 files. The code, test and spec changes are only `scripts/build_finding_register.py` (+54/−), `scripts/retire_review_claims.py` (+49/−), `bench/review/tests/test_retire_review_claims.py` (+13/−) and `docs/spec/FINDING_REGISTER_V1.md` (+15/−). `git diff --name-only 8f774d0c..HEAD -- crates fuzz Cargo.lock Makefile scripts/run_* scripts/fuzz_proof_record.py scripts/check_*_persistent_fuzz_slice.py scripts/check_s20_700_frontier.py` → empty.
- Checkers, each exit 0:
  - `check_vm_persistent_fuzz_slice.py` → `"result": "PASS"`
  - `check_semantic_checkers_persistent_fuzz_slice.py` → `"result": "PASS"`
  - `check_pack_persistent_fuzz_slice.py` → `"result": "PASS"`
  - `check_finding_register.py` → `"problems": [], "result": "PASS"`
  - `retire_review_claims.py --check` → `retirable 0, stale_closures [], regeneration_divergence [], "result": "PASS"`
- `python3 -m unittest bench.review.tests.test_finding_register bench.review.tests.test_retire_review_claims` → `Ran 58 tests … OK`. That is the same count as at 8f774d0c: one test was extended and one re-shaped, none added.
- In-memory probes on the HEAD ledger:
  - Section lists and `finding_key` for every open claim of this lane.
  - Ledger-wide tallies: open by severity [0, 4, 14, 81, 507], 312 closed, 91 restated, 0 `desc:` keys among open claims.
  - Every one of the 91 `restates` links checked for a named `carried from <sha>` that differs from the root's raising scope, and for an own identifier that differs from the root's (38 flagged, 21 identifier mismatches, 6 into closed roots).
  - For each re-statement folded into a closed root: `line_speaks_about` weak versus strong on the root's cited closing line.
  - The location of each wrongly folded re-statement's true root.
  - `is_carry`/`finding_key` on the untruncated text of my own 8f774d0 P2.
- Fixture probe: two `(carried from <sha>, OPEN)` claims naming distinct identifiers against one generic `[P3] [robustness] scripts/build_finding_register.py — CLOSED` head, a control with the same claims as originals, and `fold_restatements` on a named re-statement of `closure_head` beside a `finding_key` original.

Files read:
- `vulcan_review_closure-8f774d0.md` 1-82
- The full diff of the four changed code/spec/test files
- `scripts/retire_review_claims.py` 76-90, 340-475
- `scripts/build_finding_register.py` 540-600, 705-716, 776-786, 812-834, 847-868, 940-960, 1151-1162, 1294-1365, 1370-1445
- `bench/review/tests/test_retire_review_claims.py` 1-60, 236-275, 313-321
- `evidence/review/claim-retirements.json` diff (5 new entries: 4 packaging, 1 reproducibility, 0 for this section)
- `ariadne_contract_review-1a9f0aa.md` L20 and `nabu_architecture_review-1a9f0aa.md` L36, L38-40 (packaging), through the probe

## Evidence checked

**Repairs that hold.**
- **(1) Prose words no longer make a re-statement.** `is_carry` (`build_finding_register.py:1355-1362`) is now `CARRIED_FROM` or `LEADING_CARRY` only. `finding_key` (`:1322,1330,1342`) gives `None` only to such a claim, and an identifier-less original keys on `""`. `same_finding` (`:1352`) matches `""` only to `""`.
- **(2) Shared vocabulary uses strict keys.** `shared_vocabulary` (`:815-833`) now skips only an equal key or a ledger-linked re-statement. Shapes B (identifier-less) and C (prose `again`/`unchanged`) are pinned at `test_retire_review_claims.py:266-274` and pass.
- **(3) Root choice and regeneration order.** The fold root is now chosen by `scope_generation` (`retire_review_claims.py:373-377`), not list position. `reopen_all` (`:407-424`) returns re-statements too. Regeneration runs reopen → retire → fold (`:433-435`, `main`).
- **This section's wrong folds are gone.** It has zero re-statements. The four findings I called open-in-fact at 8f774d0 are back in `p4_open`: the 79fdcc6 equal-scope root, and the b58ac1e description-key, OPEN-head and `raising_scope` claims.
- **Ledger counts match the record.** 312 closed, 91 restated, `regeneration_divergence []`, 0 `desc:` keys.

**Where the fold still slips (evidence for the carried P2).**
- **Named carries still match any finding on the file.** A named re-statement keys identifier `None`, and `same_finding` (`:1352`) treats `None` as matching any identifier on the same lane, kind and file. The named `<sha>` is used only for ledger-anchored keys (`:1336-1338`), never for file-anchored ones.
- **Fixture proof:** a `(carried from <first>, OPEN)` claim about `closure_head` folds into a `finding_key` original (`fold_restatements` → 1, `restates` = the `finding_key` claim).
- **The retired-root guard is weaker than the spec says.** It calls `line_speaks_about(text[n - 1], entry)` at `retire_review_claims.py:388` with the default weak read and no `shared` set. By contrast, `retire()` itself passes `strong`/`shared` at `:229`. The spec paragraph (`FINDING_REGISTER_V1.md`, 8f774d0c round) says a re-statement folds into a retired root "only when the root's closing line names the re-statement too", and the code does not enforce that.
- **Live wrong links into a closed root (release_candidate_packaging):**
  - Five Ariadne re-statements restate `ariadne_contract_review@76ae15a … (is_closure_line)`, which is closed on `ariadne_contract_review-1a9f0aa.md#L20`: `scoped_before` ×2 and `is_lane_field_name` ×2 (both `(carried from 79fdcc63, OPEN)`), and `claim_finding_ids` `(carried from b58ac1e0, OPEN)`.
  - That line names only `is_closure_line`: weak read True, strong read False for all five.
  - Their true roots (`ariadne_contract_review_revision_8@79fdcc6` `scoped_before`/`is_lane_field_name`, `…_revision_9@b58ac1e` `claim_finding_ids`) are still in `p4_open`.
- **Live wrong links into open roots of other findings:**
  - Nabu packaging `is_open_line`, `raising_scope` and `p4_open` (carried from b58ac1e0) → `nabu_architecture_review@1a9f0aa … line_speaks_about`, while `nabu_architecture_review@b58ac1e` `is_open_line`/`raising_scope` are open.
  - Vulcan standards `claim_relation_problem` and `PATH_TOKEN` (8f774d0) → `vulcan_surface_review@6589c6e … line_speaks_about`, while the b58ac1e `claim_relation_problem`/`PATH_TOKEN` claims are open.
  - With the five above, that is ten wrong links. No true root I traced was hidden, since each is still listed open.
- **The checkers accept all of it.** `package_restated_claims` (`:1413-1432`) accepts every one (`same_finding`, `is_carry`, later scope, chain end), and `--check` regenerates the same folds.
- **A quoted phrase re-opens the prose shape.** `CARRIED_FROM.search(body)` matches anywhere, so a fresh finding that merely quotes `carried from <sha>` becomes a named re-statement. My own 8f774d0 P2 on its untranscribed full line gives `is_carry → True`, `finding_key → ('vulcan','gate','scripts/build_finding_register.py', None)`. Only the ledger's truncation at 436 characters keeps it an original.
- **Unknown scopes win root selection.** The root-selection key sorts an unknown raising scope (`-1`/`None`) first (`:376`), so an untraceable claim wins as root.
- **No fold-side test pins any new rule.** `test_restatements_fold_into_the_earliest_claim` (`:313-321`) was only re-shaped to leading clauses. Nothing tests prose-word non-carry, a named carry with a distinct identifier, root-by-scope, or the retired-root guard.

**Where the closure relation still follows (evidence for the P3).**
- **Named re-statements of distinct findings share a key.** Two such re-statements on one file both key `(lane, kind, file, None)`, so `shared_vocabulary` (`:833`, `other_key == own_key`) removes each from the other's vocabulary.
- **Fixture proof:** a generic `[P3] [robustness] scripts/build_finding_register.py — CLOSED` head retires 2 (both `verified the carried P3 closed`). The control, with the same two claims as originals, retires 0.
- **Why it is latent:** it needs the roots to be absent at that severity, for example after a severity change. When the roots are listed, their distinct keys restore the shared read.

**Unchanged P4 mechanisms, re-read at HEAD.**
- `replay_problems` exact-claim branch (`retire_review_claims.py:469-472`) tests `wanted <= closure` and `exact_claim_binding` only. `recorded` is computed at `:467` and not consulted.
- `open_lines_about` (`build_finding_register.py:847-868`) still calls `line_speaks_about(head, claim, strong=True, paths_ok=False)` without `shared`.
- `strictly_later_scope` (`:944-957`) and its use in `package_restated_claims` (`:1432`) still accept equal ids. The fold path's `retire.strictly_later` (`:76-78`) refuses equal and prefix ids, and `reopen_all` now un-folds, so a hand-added equal-scope fold is caught by `--check` only.
- `is_tracked` (`:1151-1162`) still `return _tracked is None or path in _tracked`.
- `_own_status` (`:712`) still `(?![^)]*OPEN)`.
- `raising_scope` (`:574,588`) still matches on the first 80 characters, and it now also orders fold roots.
- `claim_finding_ids` (`:780-781`) still excludes `S20-700` and yields `PACK-001`. The lookbehind at `:1104` still refuses the full id. The docstring at `:1036` still lists `S20-700-PACK-001`.
- The `desc:` branch (`:1336-1340`) still slices a path-first `rest`, and 0 open claims reach it.
- No checker, validator or lane path changed in 8f774d0c..HEAD.

**Ledger state for this section.**
- `p2_open` 1 (my 8f774d0 P2), `p3_open` 4, `p4_open` 37, P3 closed 20, P4 closed 14, restated 0.
- Because prose `prior … OPEN` no longer folds, each of this lane's carried findings is listed once per round, with equal keys. For example, `is_tracked` appears ×4 (6589c6e, 79fdcc6, b58ac1e, 8f774d0), the V-02 residual ×5 and the over-open-ledger claim ×6.
- The thinned-record P3 (db53894), the Makefile P3 (7622776) and five chronology P4s are still open, although I stated them CLOSED at 8f774d0. None of the five new exact-claim entries is for this section.

**Per-finding status (the 8f774d0c findings):**
- **[P2] [gate] scripts/build_finding_register.py:1307,1317,1330,1339,1348; scripts/retire_review_claims.py:360-378 — `same_finding` wildcard / `is_carry` prose word — OPEN, narrowed.** Fixed: the prose-word carry, the wildcard for originals, list-order roots, and folds kept through regeneration; this section's five are back in `p4_open`. Still open: the named-carry `None` wildcard ignores both the named sha and the claim's own identifier; the retired-root guard uses the weak read; and a quoted `carried from <sha>` anywhere makes a named carry. The ten live wrong links, five into a closed root, are re-filed below.
- **[P3] [gate] scripts/build_finding_register.py:824,835,1118 — `shared_vocabulary` over `same_finding` — CLOSED.** Strict key equality plus the ledger link is at `:815-833`. Shapes B and C are pinned at `test_retire_review_claims.py:266-274` and pass. The named-re-statement shape D is a new residual, filed as its own P3.
- **[P4] `claim_finding_ids` `S20-700-PACK-001` split (:780-781,1095) — OPEN.** Unchanged; the lookbehind is now at `:1104`.
- **[P4] `replay_problems`' exact-claim branch (retire_review_claims.py:445-448) — OPEN.** Unchanged; now at `:469-472`.
- **[P4] the description key (:1325-1328) — OPEN, latent.** Now at `:1336-1340`; 0 `desc:` keys.
- **[P4] the OPEN-head match without `shared` (:838-860) — OPEN.** Unchanged; now at `:847-868`.
- **[P4] `strictly_later_scope` equal ids / `reopen_all` never un-folds (:935-946; retire :386-401) — OPEN, narrowed.** The `reopen_all` leg is CLOSED (`:407-424`). The builder still accepts equal ids (`:944-957,1432`), which only `--check` catches.
- **[P4] `is_tracked` fail-open (:1142-1153) — OPEN.** Unchanged; now at `:1151-1162`.
- **[P4] V-02 pin residual (check_pack :157-172, check_semantic_checkers :254-266, fuzz_proof_record :80-84,101-158) — OPEN.** No diff.
- **[P4] over-open ledger (machine-summary.json s20_700_remaining_surface_audit p2/p3/p4_open) — OPEN, changed shape.** The under-open leg is CLOSED, since the four mis-folded claims are listed open again. The list is now 1/4/37: equal-key duplicates per round, and seven entries closed in fact with no exact-claim entry.
- **[P4] parenthetical refusal case-sensitive (:712) — OPEN.** Unchanged.
- **[P4] `raising_scope` 80-character prefix (:574,588) — OPEN.** The mechanism is unchanged, and the claim-restoration leg is CLOSED. It now also feeds fold-root ordering at `retire_review_claims.py:376`.

## Findings

[P2] [gate] scripts/build_finding_register.py:1322,1330,1342,1352; scripts/retire_review_claims.py:373-377,382-390 - OPEN, narrowed (carried from 8f774d0c): a named re-statement (leading `(carried from …)` clause, or a `carried from <sha>` phrase anywhere in the prose) keys identifier `None`, and `same_finding` then matches it to any same-lane/kind/file claim, ignoring both the named sha (used only for ledger-anchored keys) and its own identifier. The retired-root guard reads the root's closing line with the weak `line_speaks_about` (no strong read, no `shared`), not the "names the re-statement" rule the spec states. Live at HEAD, ten links are wrong. In packaging, Ariadne `scoped_before` ×2, `is_lane_field_name` ×2 and `claim_finding_ids` restate the CLOSED `is_closure_line@76ae15a` via `ariadne_contract_review-1a9f0aa.md#L20` (weak True, strong False), while their true 79fdcc6/b58ac1e roots are open. Nabu `is_open_line`/`raising_scope`/`p4_open` restate `line_speaks_about@1a9f0aa`. In standards, Vulcan `claim_relation_problem`/`PATH_TOKEN` restate `line_speaks_about@6589c6e`. Fixture: a named `closure_head` re-statement folds into a `finding_key` original. My own 8f774d0 P2's untruncated text is a named carry because it quotes `carried from 79fdcc63`. `package_restated_claims` (:1413-1432) and `--check` accept and regenerate all of this, and no fold-side test pins the new rules - closure: key a named re-statement on its own identifier when it has one, and require the root to be raised at (or chain to) the named `<sha>` for file-anchored keys; match `carried from <sha>` only in a leading clause or structural position; use the strong read with `shared` in the retired-root guard; do not sort unknown scopes first; regenerate and re-list the ten under their true roots; add fold tests for named-carry-distinct-identifier, quoted-phrase, root-by-scope and retired-root-guard shapes
[P3] [gate] scripts/build_finding_register.py:833,1330 - two named re-statements of distinct findings on one file both key `(lane, kind, file, None)`, so `shared_vocabulary` excludes each from the other (`other_key == own_key`). The kind reads as unshared, and a generic `[<kind>] <path> — CLOSED` head retires both. Fixture: 2 retired; the same claims as originals retire 0. Latent: it needs the roots to be absent at that severity - closure: exclude only the ledger-linked root/re-statement pair (not key-equal `None` keys) from shared vocabulary, and pin the two-named-re-statements shape in `test_retire_review_claims`
[P4] [gate] scripts/build_finding_register.py:780-781,1104 - `claim_finding_ids` still reduces `S20-700-PACK-001` to `PACK-001`, the lookbehind refuses the full id, and the docstring at :1036 still lists it; latent - closure: match the whole multi-segment id and test `S20-700-PACK-001`
[P4] [gate] scripts/retire_review_claims.py:469-472 - `replay_problems`' exact-claim branch still skips `open_lines_about` (`recorded` at :467 is not consulted), so an exact-claim entry citing a transcript that records the claim OPEN replays clean - closure: check `open_lines_about` in the exact-claim branch and add the test
[P4] [gate] scripts/build_finding_register.py:1336-1340 - the description key still slices a path-first `rest`; latent (0 `desc:` keys among open claims) - closure: strip the anchor before slicing, or refuse an origin-less ledger claim; test
[P4] [gate] scripts/build_finding_register.py:847-868 - the OPEN-head match still runs `strong=True, paths_ok=False` without the closing side's `shared` set - closure: pass the shared/ledger exclusions to the OPEN read and test both b58ac1e0 pairs
[P4] [gate] scripts/build_finding_register.py:944-957,1432 - OPEN, narrowed (the `reopen_all` leg is CLOSED): the builder's `strictly_later_scope` and `package_restated_claims` still accept equal scope ids; only `--check` regeneration catches a hand-added equal-scope fold - closure: refuse equal/prefix ids in `strictly_later_scope` as `retire.strictly_later` does; test
[P4] [gate] scripts/build_finding_register.py:1151-1162 - `is_tracked` still fails open: a failing `git ls-files` makes every path count as tracked - closure: fail closed on git failure and test it
[P4] [gate] scripts/check_pack_persistent_fuzz_slice.py:157-172, scripts/check_semantic_checkers_persistent_fuzz_slice.py:254-266, scripts/fuzz_proof_record.py:80-84,101-158 - V-02 pin residual carried unchanged (no diff 8f774d0c..HEAD): values are presence-checked only, `fuzz/regressions/` is not a lane path, and `still_crashes` is never read - closure: pin id/target/classification and a hex digest per record, add the directory to the declaring lanes, require every `still_crashes` false
[P4] [evidence] machineresearch/sley-2.0/machine-summary.json (s20_700_remaining_surface_audit p2_open/p3_open/p4_open) - over-open ledger, changed shape (1/4/37, 0 restated; the under-open leg is CLOSED): each carried finding of this lane is listed once per round with equal keys (e.g. `is_tracked` ×4, V-02 ×5, over-open ×6), and seven entries closed in fact (thinned record db53894, Makefile 7622776, chronology ×5) are still open with no exact-claim entry - closure: exact-claim entries citing the lane's CLOSED status lines for those seven, and one fold rule that recognises this lane's `prior … OPEN` re-statements structurally, or equal-key de-duplication
[P4] [gate] scripts/build_finding_register.py:712 - the parenthetical refusal `(?![^)]*OPEN)` is still case-sensitive, so `| **CLOSED (leg 2 open)** |` is a closure line - closure: refuse `open` case-insensitively and test the lowercase shape
[P4] [gate] scripts/build_finding_register.py:574,588 - `raising_scope` still matches on the first 80 characters (claim-restoration leg CLOSED), and it now also orders fold roots (retire_review_claims.py:376), so a prefix collision can pick the wrong root generation - closure: match the full description (or kind + anchor + first clause) and test a prefix collision

## Assessment

The 69907ddf repair holds for original claims:
- A prose word no longer makes a re-statement.
- Identifier-less originals match only each other.
- Shared vocabulary uses strict key equality, and shapes B and C are pinned.
- Fold roots are chosen by ancestry.
- Regeneration un-folds and re-derives.
- This section's five mis-folds are back in `p4_open`.
- The three section checkers, the register checker, `--check` and 58 tests pass.
- No fuzz, checker or lane path changed.

My 8f774d0 P3 is CLOSED on its named shapes, and two P4 legs closed (`reopen_all`; the under-open ledger and `raising_scope` restoration).

The wildcard has moved to the named-carry side. A `None` identifier still matches any same-lane/kind/file claim, the named sha is ignored for file anchors, a quoted `carried from <sha>` anywhere makes a named carry, and the retired-root guard uses the weak read the spec says it does not. At HEAD, ten re-statements are linked to distinct findings, five of them to a closed root whose status line does not name them. Both checkers accept and regenerate these links, and no fold-side test would catch them. Evidence-integrity is still wrong on the mechanism all eight reviews named, so the P2 stays open, narrowed. The same key equality leaves a latent closure-side residual for named re-statements (P3). The other P4s are unchanged or narrowed. This is not closable as `PRIOR_P3_P4_CLOSED`.

VERDICT: REVISE_0_P0_0_P1_1_P2_1_P3_10_P4
SECTION: s20_700_remaining_surface_audit
FIELD: vulcan_review_closure
SCOPE_SHA: 8966da2e0a52a6f6e56949e9a5902464c86613c0
FINDINGS:
[P2] [gate] scripts/build_finding_register.py:1322,1330,1342,1352; scripts/retire_review_claims.py:373-377,382-390 - OPEN, narrowed (carried from 8f774d0c): a named re-statement (leading `(carried from …)` clause, or a `carried from <sha>` phrase anywhere in the prose) keys identifier `None`, and `same_finding` then matches it to any same-lane/kind/file claim, ignoring both the named sha (used only for ledger-anchored keys) and its own identifier. The retired-root guard reads the root's closing line with the weak `line_speaks_about` (no strong read, no `shared`), not the "names the re-statement" rule the spec states. Live at HEAD, ten links are wrong. In packaging, Ariadne `scoped_before` ×2, `is_lane_field_name` ×2 and `claim_finding_ids` restate the CLOSED `is_closure_line@76ae15a` via `ariadne_contract_review-1a9f0aa.md#L20` (weak True, strong False), while their true 79fdcc6/b58ac1e roots are open. Nabu `is_open_line`/`raising_scope`/`p4_open` restate `line_speaks_about@1a9f0aa`. In standards, Vulcan `claim_relation_problem`/`PATH_TOKEN` restate `line_speaks_about@6589c6e`. Fixture: a named `closure_head` re-statement folds into a `finding_key` original. My own 8f774d0 P2's untruncated text is a named carry because it quotes `carried from 79fdcc63`. `package_restated_claims` (:1413-1432) and `--check` accept and regenerate all of this, and no fold-side test pins the new rules - closure: key a named re-statement on its own identifier when it has one, and require the root to be raised at (or chain to) the named `<sha>` for file-anchored keys; match `carried from <sha>` only in a leading clause or structural position; use the strong read with `shared` in the retired-root guard; do not sort unknown scopes first; regenerate and re-list the ten under their true roots; add fold tests for named-carry-distinct-identifier, quoted-phrase, root-by-scope and retired-root-guard shapes
[P3] [gate] scripts/build_finding_register.py:833,1330 - two named re-statements of distinct findings on one file both key `(lane, kind, file, None)`, so `shared_vocabulary` excludes each from the other (`other_key == own_key`). The kind reads as unshared, and a generic `[<kind>] <path> — CLOSED` head retires both. Fixture: 2 retired; the same claims as originals retire 0. Latent: it needs the roots to be absent at that severity - closure: exclude only the ledger-linked root/re-statement pair (not key-equal `None` keys) from shared vocabulary, and pin the two-named-re-statements shape in `test_retire_review_claims`
[P4] [gate] scripts/build_finding_register.py:780-781,1104 - `claim_finding_ids` still reduces `S20-700-PACK-001` to `PACK-001`, the lookbehind refuses the full id, and the docstring at :1036 still lists it; latent - closure: match the whole multi-segment id and test `S20-700-PACK-001`
[P4] [gate] scripts/retire_review_claims.py:469-472 - `replay_problems`' exact-claim branch still skips `open_lines_about` (`recorded` at :467 is not consulted), so an exact-claim entry citing a transcript that records the claim OPEN replays clean - closure: check `open_lines_about` in the exact-claim branch and add the test
[P4] [gate] scripts/build_finding_register.py:1336-1340 - the description key still slices a path-first `rest`; latent (0 `desc:` keys among open claims) - closure: strip the anchor before slicing, or refuse an origin-less ledger claim; test
[P4] [gate] scripts/build_finding_register.py:847-868 - the OPEN-head match still runs `strong=True, paths_ok=False` without the closing side's `shared` set - closure: pass the shared/ledger exclusions to the OPEN read and test both b58ac1e0 pairs
[P4] [gate] scripts/build_finding_register.py:944-957,1432 - OPEN, narrowed (the `reopen_all` leg is CLOSED): the builder's `strictly_later_scope` and `package_restated_claims` still accept equal scope ids; only `--check` regeneration catches a hand-added equal-scope fold - closure: refuse equal/prefix ids in `strictly_later_scope` as `retire.strictly_later` does; test
[P4] [gate] scripts/build_finding_register.py:1151-1162 - `is_tracked` still fails open: a failing `git ls-files` makes every path count as tracked - closure: fail closed on git failure and test it
[P4] [gate] scripts/check_pack_persistent_fuzz_slice.py:157-172, scripts/check_semantic_checkers_persistent_fuzz_slice.py:254-266, scripts/fuzz_proof_record.py:80-84,101-158 - V-02 pin residual carried unchanged (no diff 8f774d0c..HEAD): values are presence-checked only, `fuzz/regressions/` is not a lane path, and `still_crashes` is never read - closure: pin id/target/classification and a hex digest per record, add the directory to the declaring lanes, require every `still_crashes` false
[P4] [evidence] machineresearch/sley-2.0/machine-summary.json (s20_700_remaining_surface_audit p2_open/p3_open/p4_open) - over-open ledger, changed shape (1/4/37, 0 restated; the under-open leg is CLOSED): each carried finding of this lane is listed once per round with equal keys (e.g. `is_tracked` ×4, V-02 ×5, over-open ×6), and seven entries closed in fact (thinned record db53894, Makefile 7622776, chronology ×5) are still open with no exact-claim entry - closure: exact-claim entries citing the lane's CLOSED status lines for those seven, and one fold rule that recognises this lane's `prior … OPEN` re-statements structurally, or equal-key de-duplication
[P4] [gate] scripts/build_finding_register.py:712 - the parenthetical refusal `(?![^)]*OPEN)` is still case-sensitive, so `| **CLOSED (leg 2 open)** |` is a closure line - closure: refuse `open` case-insensitively and test the lowercase shape
[P4] [gate] scripts/build_finding_register.py:574,588 - `raising_scope` still matches on the first 80 characters (claim-restoration leg CLOSED), and it now also orders fold roots (retire_review_claims.py:376), so a prefix collision can pick the wrong root generation - closure: match the full description (or kind + anchor + first clause) and test a prefix collision
SUMMARY: HEAD 8966da2e is verified as the records-only descendant of 69907ddf; the three section checkers, `check_finding_register.py`, `retire_review_claims.py --check` and 58 review tests pass, and no fuzz, checker or lane path changed since 8f774d0c. The repair fixes prose-word carries, the identifier wildcard for originals, list-order roots and folds kept through regeneration: my 8f774d0 P3 is CLOSED and this section's five mis-folds are back in `p4_open`. The P2 stays OPEN, narrowed: a named re-statement's `None` identifier still matches any same-lane/kind/file claim, the named sha and quoted phrases are not honoured structurally, and the retired-root guard uses the weak read, leaving ten wrong links at HEAD (five into a closed root) that both checkers accept. A latent named-re-statement closure residual (P3) and ten P4s, unchanged or narrowed, remain; REVISE, not closable as PRIOR_P3_P4_CLOSED.
