<!-- engine: claude-code; observed model: claude-opus-5-5; scope: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e; role: vulcan; field: vulcan_surface_review; dispatched: 2026-09-23T05:26:58Z; duration_s: 1645; process_exit_code: 0 -->
# Vulcan Council review — release_candidate_packaging
Harness: claude-code
Reviewed checkpoint: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e

This review was read-only. I made no edits in the tree. Probes ran in memory or in `tempfile` git fixtures outside the tree. The only file I wrote was my own memory note, outside the repository. Here is what I verified:

- **Scope.** `git rev-parse HEAD` → `d158d26bfdda9fc7dc899a1d1a8a141c2da6610e`, so the scope is correct. `git log --format='%h %s' 8966da2e..HEAD` lists acbc65f0, 883361e3, f7f9af90, 5fd10849, 0f4b8408, 7c51710b, 8d063f00 and d158d26b.
- **Diff.** `git diff --stat 8966da2e..HEAD` → 53 files. I read these deltas in full: `scripts/retire_review_claims.py` and `scripts/build_finding_register.py` (`git diff 8966da2e..HEAD -- …`), plus the commit messages of acbc65f0, 0f4b8408 and d158d26b.
- **Files read at HEAD, with line ranges:**
  - `scripts/build_finding_register.py`: :571-705, :749-842, :860-976, :1160-1294, :1452-1540, :1596-1735
  - `scripts/retire_review_claims.py`: :80-349, :354-500, :516-600
  - `bench/review/tests/test_retire_review_claims.py`: :1-95, :318-487, :579-793
  - `bench/review/tests/test_finding_register.py`: :376-397
  - `bench/review/tests/test_fuzz_proof_record.py`: :82-100
  - `scripts/fuzz_proof_record.py`: :120-169
  - `docs/spec/FINDING_REGISTER_V1.md`: :95-209
  - `evidence/review/claim-retirements.json`: :2020-2079
  - `evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-b58ac1e.md`: :27, :49-50
  - `nabu_architecture_review-8966da2.md`: :28
  - my own 8966da2e verdict, whole
  - `machineresearch/sley-2.0/machine-summary.json`: :5320-5340, :6425-6449, :7891-7950, :8098-8109; Grep over the restated, closed and note fields
  - `evidence/review/rounds/resumption-8966da2-batch14.json` and `revision-7-retirement-changes.json` (Grep)
- **Checkers:**
  - `python3 -B scripts/check_release_candidate_packaging.py` → `"result": "PASS"`, `"problems": []`, `"revision": 7`. I did not capture the exit code separately.
  - `python3 -B scripts/check_finding_register.py` → exit 0, `"result": "PASS"`, `"problems": []`.
  - `python3 -B scripts/retire_review_claims.py --check` → exit 0, `"result": "PASS"`. It reports retirable 0, stale_closures [], regeneration_divergence [], split_status [], and 9 report-only closer_disagreements.
- **Unit tests.** `python3 -B -m unittest discover -s bench/review/tests -t .` first returned `Ran 170 … FAILED (errors=26)`, then `errors=45` on a rerun. I traced the cause:
  - Captured stderr: `fatal: cannot copy '/usr/share/git-core/templates/…' … No space left on device`.
  - `df -i /tmp` showed 1048561/1048576 inodes in use, later 100%. The space is taken by roughly 166k `inv-probe-*` files from another process, which I left alone.
  - Each failing test passes on its own.
  - Rerun in-process with `tempfile.tempdir = "/dev/shm"`: the review suite ran 170 with 0 errors and 0 failures; the release suite ran 156 with 0 errors and 0 failures.
  - The `/tmp` exhaustion later also broke the harness's own output capture (ENOSPC). The last few probes and a final `git status` could not be captured; this is noted where it applies. The last `git status --porcelain` I captured, mid-review after the test runs, was empty.
- **Mint recomputation.** I recomputed the 8d063f00 mint from `/home/gfarch/Work/checkpoints/sley2-candidate-8d063f0/dist/sley-2.0.0-linux-x86_64.tar.gz`:
  - sha256 `ecf51188d52bd673800c7b91068f2ad58d93068a23e1f37e60320c58a398c1f7`, 2,508,082 bytes.
  - 26 members, all with zero mtime/uid/gid/uname/gname.
  - MANIFEST `commit` is 8d063f00…; `ga_claimed` and `publication_authorized` are false; `working_tree_clean` is true; `member_count` is 15.
  - `manifest_digest` recomputes to `f0bd4072…e800` (match). 15/15 `files` entries match. 0 members contain a host path.
  - `ecf51188…` appears in the reproducibility report, provenance, GA report, dossier, content checks, both SBOMs and the summary.
  - The only 69907ddf mention in the summary is the attestation-history list at :6439, which is legitimate.
- **Probes I ran** (fixtures built from the suite's own `setUp`):
  - The OPEN-refusal shapes, at HEAD and against the 8966da2e code loaded from `git show`.
  - Equal-key inheritance of a distinct same-identifier finding.
  - A live audit of all 320 closures and 72 re-statements against the unreduced OPEN head.

## Evidence checked

**Prior 8966da2e findings (my vulcan_surface_review-8966da2: 1 P2 + 18 P4):**

- **[P2] fold chains to the oldest sibling (`finding_key`/`same_finding` wildcard, `fold_restatements` earliest-by-scope, weak closing read) — CLOSED.**
  - `finding_key` keys on the claim's own identifier (`_own_identifier`, build_finding_register.py:1457), and `same_finding` is now equality (:1530).
  - The fold resolves its root by `named_carry_sha` and folds only when exactly one match exists (retire_review_claims.py:434-444).
  - A fold into a retired root must pass `closing_lines_name` (:397-425): `speaking_lines` (strong read plus OPEN refusal) must intersect the cited lines, and `strictly_later(closer_scope, entry_scope)` must hold.
  - The builder replays the named-sha rule (:1657-1674).
  - Fixture tests pin these rules (test_retire_review_claims.py:342-387, :674-726).
  - Live, the six false folds I named are undone. The Ariadne `scoped_before`, `is_lane_field_name` and `claim_finding_ids` re-statements restate open roots at 79fdcc6 and b58ac1e (summary :5323-5336). The standards `raising_severity` re-statement restates its open 79fdcc6 root (:7100-7101). 0 of 72 re-statements fold into a closed root.
  - `retire --check` runs in the Makefile gates (:91, :284).
  - Residual: the test data at :388-389 were changed rather than restored. The two claims now first-name different identifiers (`note` and `records`), so the original same-first-identifier shape is no longer pinned. That shape feeds new finding 2.
- **[P4] `closers` union (retire_review_claims.py:190-200) — CLOSED.** `closer_disagreements` (:516-540) reports these cases in `--check`, and test_closer_disagreement_is_reported pins it. 9 are reported live. Of the live P0-P2 closures in rw090, none cites any of the nine reported transcripts. The P2 at summary :8107 binds through Ariadne 92fa664, whose token has no PRIOR group. That is the line-derived closer rule, not the union.
- **[P4] spec :99-102,115-116 vs mechanics — OPEN.** The carry-anywhere sentence is gone (:119-121). The strong read still admits "a path with a tag word" (:104-106), while code refuses it (:1281). Carried below together with the next point.
- **[P4] `LEADING_CARRY` accepts a sha-less clause — CLOSED.** :1482 requires `from <sha>`, `CARRY` is deleted, and tests :641 and :671-672 pin it.
- **[P4] `""` key merges identifier-less originals — CLOSED.** `shared_vocabulary` counts equal-key originals (:927-929), and `""` is excluded from inheritance and split (retire :475, :565-569). Tests :625-650 and :727-752 pin it. The residual for non-empty identifiers is new finding 2.
- **[P4] `raising_scope` 14 rw075 claims scope-less — OPEN, unchanged.** `ariadne_round13`, `nabu_round13` and `premium_delta_r2` are still untagged, and there is no raising transcript under verdicts/rw075_correction (summary :7925-7950).
- **[P4] span rule across files (:1113-1121) — CLOSED.** The basename rule is at :1265-1275.
- **[P4] revision-7-retirement-changes.json `counts` — OPEN, unchanged.** It still reads 127/250/78 against 320 tracked closures.
- **[P4] OPEN-head vs weak-read asymmetry, `STATUS_OPEN` — merged into new finding 1, not counted separately.** The grammar half is fixed: `STILL/still/remains OPEN` now reads as OPEN (:755). The asymmetry half got worse and is new finding 1.
- **[P4] line-scoping (weak read searches the whole line; `closure_head` untested) — OPEN, unchanged.**
- **[P4] `raising_severity` → `None` accepted — CLOSED.** `tagged_claim_unraised` (:675) is applied in the open, closed and restated lists (:707-710, :1377, :1630).
- **[P4] record-staleness: three ledger-path-only claims — CLOSED.** claim-retirements.json:2023-2050 binds them exactly to my b58ac1e :27, :49 and :50. I read each line; each one's own head closes the named finding.
- **[P4] grammar-precision, six shapes — OPEN, narrowed.**
  - Two shapes are fixed: `— CLOSED.** leg 2 — still OPEN` and `**CLOSED (leg 2 open)**` (:755, :788-791).
  - By reading :749-755 and :802-812, four still close: `remains OPEN — CLOSED` (no status mark before "remains"), `(residual — REOPENED) — CLOSED`, `— All NOT CLOSED` (`All(?:\s+\w+)?` consumes NOT) and `— CLOSED for the rule half:`.
  - My empirical re-probe of these could not be captured because of ENOSPC.
- **[P4] fuzz `regression_records`/`still_crashes` binding — OPEN, narrowed.** `still_crashes` is now refused (fuzz_proof_record.py:155-159). A record is bound only by `(root / record).is_file()` (:152-154), not to the tracked fixture list.
- **[P4] ledger-shape (open/closed twins; the gate compares transcripts without `#L`) — CLOSED.** `split_status_problems` (:543-577) handles twins, and the gate now compares the full reference (:590-597). Both are tested; `--check` reports split_status [].
- **[P4] `pN_open` without `_count` — CLOSED.** It is refused at :698-706.
- **[P4] `scoped_before(…) or True` — CLOSED.** test_finding_register.py:393-394 now asserts the real outcomes.
- **[P4] RCP spec/checker pinned at revision 6 — CLOSED.** The checker prints revision 7 and pins `contract_revision` 7 and the spec line (check_release_candidate_packaging.py:203-210).
- **[P4] 69907ddf commit message "eight P2s" — CLOSED.** No tree change was needed. The batch-14 record carries per-verdict tokens: 2+1+1+1+1+0+1+1 = 8 P2 claims.

**Delta and record consistency:**
- **P0-P2 direction.** The register shows 24 open P0-P2 claims: 7 packaging + 5 reproducibility + 4 rw075 P1 + 4 rw075 P2 + 2 s20_700 + 2 standards. That is 23 distinct findings (reproducibility 5 → 4). The GA row states both numbers. My arithmetic: 18 − 2 + 8 = 24. The two closures are Nabu 8966da2 L28 and my 8966da2 L22, both genuine CLOSED lines that I read; the 8 are batch 14's P2 claims. Nothing else in P0-P2 moved.
- **Mint.** Recomputed identical (see above). The REQ-11 product change is outside this lane and does not touch this section's owned paths.

## Findings
[P2] [fail-closed-regression] scripts/build_finding_register.py:970 (`open_lines_about`: the OPEN item's head is reduced to `CATEGORY.findall(head) + PATH_TOKEN.findall(head)`) with :974, scripts/retire_review_claims.py:233 (`speaking_lines`), :292 (exact-claim OPEN refusal), :397-425 (`closing_lines_name`) and build_finding_register.py:1444 (`package_closed_claims`) - the OPEN refusal ("a transcript that records the claim OPEN cannot close it on another line") now sees only a file:line anchor or an identifier written inside brackets; backticked identifiers, finding ids and quoted phrases are dropped from the head. Reproduction: a transcript whose L1 reads `- **[P4] record `alpha_guard` stale — CLOSED.**` and whose L2 reads `- **[P4] record `alpha_guard` stale — OPEN.**` (or `[P4] [record] `alpha_guard` stale - still OPEN`) gives `open_lines_about` → `[]` and `speaking_lines` → `[1]`. The claim auto-retires, and an exact-claim binding to L1 is accepted. The same probe against the 8966da2e code gives `[2]` and `[]` for both shapes. The only pinned shape is the bracketed `[`alpha_guard`]` (test_retire_review_claims.py:592-605). The spec (FINDING_REGISTER_V1.md:104-106, :158, :162-163, :184-186) promises the strong-read OPEN refusal. Live effect: 0 of 320 closures and 0 of 72 folds rest on a transcript that records the claim OPEN under the unreduced head. The regression affects every severity, including the P0-P2 count behind GA row 26.6 - closure evidence needed: keep the head's backticked identifiers, finding ids and quoted phrases (drop only foreign carry parentheticals, e.g. via `unquoted` plus parenthetical stripping), tests for the plain-identifier and finding-line OPEN shapes on the retire, exact-claim and fold paths, and a ledger re-derivation.
[P3] [identity-coarseness] scripts/retire_review_claims.py:467-500 (exact-key inheritance pass), :413-418 (`closing_lines_name` drops the retired twin from the shared vocabulary), scripts/build_finding_register.py:1457 (`_own_identifier`: the key is lane + kind + file + first backticked identifier), :1661-1689 (builder `exact_inherit` skips the carry and chronology checks and has no closing-line replay) vs docs/spec/FINDING_REGISTER_V1.md:193-194 ("an uncarried copy never folds by key alone — a lane may raise a distinct finding on an identifier it used before") - two distinct originals of one round that share lane, kind, file and first identifier (A: `scripts/a.py:1 - `alpha_guard` leaks the session token`; B: `scripts/a.py:40 - `alpha_guard` has no bound on the retry count`) key equal. When a later transcript closes A by a line naming `alpha_guard`, B inherits A's closure. The probe gives `r=1 f=1 open=0 restated=1 split=0`, and the same result when the transcript also records `- **[P4] [record] `alpha_guard` retry bound in scripts/a.py — still OPEN.**` (via finding 1). Only an OPEN line carrying B's own `a.py:40` anchor keeps B open, and `--check` then fails on split status for a legitimately distinct finding. Tests :450-465 and :727-752 pin inheritance across different anchors; :388-389 no longer pin the same-first-identifier shape. Live: 0 exact-key inheritances. Builder acceptance of the inheriting ledger is from reading :1661-1689; my first fixture was refused by the builder, most likely because it had no raising line, and the re-probe could not be captured - closure evidence needed: require the closing head to name the inheriting claim by something beyond the key identifier (its own anchor or span, or a finding id), or restrict inheritance to named carries; replay the closing-line and chronology gate for `exact_inherit` in `package_restated_claims`; a fixture test with two distinct same-identifier originals, one closed.
[P4] [contract-vs-mechanics] docs/spec/FINDING_REGISTER_V1.md:21,104-106,463 vs scripts/build_finding_register.py:1281, and :153-154 vs :189-190 with scripts/retire_review_claims.py:442 - carried, narrowed: the carry-anywhere sentence is gone, but the strong read still admits "a path with a tag word" (code refuses it), and the revision-8 paragraph's "nearest by git ancestry when several match" stands beside revision 9's "ambiguous and stays open", which the code implements - closure evidence needed: one sentence stating the symmetric strong read; strike or date-supersede the "nearest" clause.
[P4] [relation-precision] scripts/build_finding_register.py:559-609 (`raising_scope`) - carried, unchanged: the rw075 lane-less claims (`ariadne_round13`, `nabu_round13`, `premium_delta_r2`; summary :7925-7950) still have no raising scope, and no coverage is reported - closure evidence needed: tag or derive their scopes, refuse scope-less explicit retirement, report coverage.
[P4] [records] evidence/review/rounds/revision-7-retirement-changes.json (`counts`) - carried, unchanged: `entries_now` 127, `closures_now` 250 and `exact_claim_entries_now` 78 against 320 tracked closures - closure evidence needed: regenerate or date the `_now` fields.
[P4] [line-scoping] scripts/build_finding_register.py:1214-1281 (the weak read searches the whole line) - carried, unchanged: under an unshared kind the identifier, quoted-phrase, finding-id, anchor and span rules read trailing prose; `closure_head` has no direct test - closure evidence needed: apply the head to every rule except the kind phrase; a test.
[P4] [grammar-precision] scripts/build_finding_register.py:749-755,802-812 - carried, narrowed (2 of 6 fixed): `remains OPEN — CLOSED`, `(residual — REOPENED) — CLOSED`, `— All NOT CLOSED` and `— CLOSED for the rule half:` still read as closures (by reading; the re-probe was lost to ENOSPC) - closure evidence needed: treat `remains OPEN` without a status mark, `REOPENED` and `NOT CLOSED` as non-closures, and a partial-scope refusal; tests.
[P4] [binding] scripts/fuzz_proof_record.py:145-159 - carried, narrowed: `still_crashes` is refused, but `regression_records` is bound only by `(root / record).is_file()`, so absolute paths, `..` paths and untracked files pass; non-list dict values (:148) and a non-list `retested_regressions`/`retested_prior_crashes` (:157) are silently dropped; the test (test_fuzz_proof_record.py:82-100) pins existence only - closure evidence needed: require a tracked path under `fuzz/regressions/` (no absolute path, no `..`), refuse non-list shapes; tests.

## Assessment
The 8d063f00 mint is sound, and every identity I recomputed agrees. It hashes to ecf51188… at 2,508,082 bytes with zeroed metadata and no host paths. The manifest digest recomputes to f0bd4072…, all 15 member files match, and GA and publication are both false. Every binding record names the new artifact, and no superseded identity remains outside history lists. The three checkers PASS. The 170 + 156 tests pass once the fixtures use a temp directory with free inodes; the first-run errors were environmental (`/tmp` inode exhaustion from another process), not code failures. The register is open with 24 P0-P2 claims (23 distinct findings), which I re-summed.

My 8966da2e P2 is closed as asked:
- Identity is equality on the claim's own identifier.
- The fold resolves its root by the named sha, and an ambiguous root stays open.
- A fold into a retired root must pass the strong-read closing-line check and closer chronology.
- The builder replays the named-sha rule, and fixture tests pin these rules.
- All six live false folds are undone, and no re-statement folds into a closed root today.

Ten of the eighteen P4s are closed. Five are carried or narrowed as findings, and the asymmetry one is merged into new finding 1.

The repair round did introduce a regression in the section's own fail-closed machinery. To fix an Ariadne false positive, `open_lines_about` now keeps only bracket and path tokens from the OPEN head. As a result, an OPEN line that names a finding by its backticked identifier (the ordinary shape) no longer stops that transcript from closing the finding. This affects automatic retirement, exact-claim bindings and the new inheritance pass alike. I reproduced it against both the HEAD code and the 8966da2e code.

The new exact-key inheritance also treats two distinct findings that share a first identifier as one finding. It closes the second one from the first one's closer, even when that transcript says the second is still OPEN. Neither defect affects a live record today (0 of 320 closures, 0 of 72 folds, 0 inheritances), and no P0-P2 count is understated. The OPEN-refusal regression is still a P2, because it re-opens a guarded closure path at every severity and its only test pins a shape that hides the defect.

GA and release readiness are outside this scope and not claimed.

VERDICT: REVISE_0_P0_0_P1_1_P2_1_P3_6_P4_PRIOR_P2_CLOSED
SECTION: release_candidate_packaging
FIELD: vulcan_surface_review
SCOPE_SHA: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e
FINDINGS: [P2] [fail-closed-regression] build_finding_register.py:970 with :974, retire_review_claims.py:233,292,397-425 and build_finding_register.py:1444 - the OPEN head is reduced to bracket and path tokens, so an OPEN line naming the claim by a backticked identifier, finding id or quoted phrase no longer blocks its closure (HEAD: open_lines [] / speaking [1] / exact-claim accepted / auto-retired; 8966da2e: [2] / []); only the bracketed shape is tested; 0 live effect - keep identifiers, finding ids and quoted phrases in the head, test the plain and finding-line shapes, re-derive; [P3] [identity-coarseness] retire_review_claims.py:467-500,413-418 with build_finding_register.py:1457,1661-1689 vs FINDING_REGISTER_V1.md:193-194 - exact-key inheritance folds a distinct finding sharing lane, kind, file and first identifier into its sibling's closure, even when the transcript records it still OPEN (via the P2); the builder does not replay the closing-line or chronology gate; 0 live inheritances - require an identity beyond the key identifier or restrict inheritance to carries, replay the gate in the builder, add a fixture test; [P4] [contract-vs-mechanics] FINDING_REGISTER_V1.md:21,104-106,463 vs build_finding_register.py:1281, and :153-154 vs :189-190 - carried, narrowed; [P4] [relation-precision] build_finding_register.py:559-609 - carried, unchanged: rw075 lane-less claims have no raising scope; [P4] [records] revision-7-retirement-changes.json counts - carried, unchanged (127/250/78 vs 320); [P4] [line-scoping] build_finding_register.py:1214-1281 - carried, unchanged; [P4] [grammar-precision] build_finding_register.py:749-755,802-812 - carried, narrowed: 4 of 6 shapes still close; [P4] [binding] fuzz_proof_record.py:145-159 - carried, narrowed: existence-only binding, path escapes and non-list shapes accepted.
SUMMARY: The 8d063f00 mint recomputes and binds consistently (ecf51188…, manifest f0bd4072…, 15/15 files, GA and publication false). All three checkers PASS, the 170 + 156 tests pass once fixtures avoid the inode-exhausted /tmp, and the register's 24 open P0-P2 claims (23 distinct) re-sum correctly. My 8966da2e fold P2 is closed as asked (named-sha roots, equality identity, a strong-read and chronology gate for retired roots, six live false folds undone), and ten of eighteen P4s are closed. The round introduced a P2 regression: `open_lines_about` now drops identifiers from the OPEN head, so a transcript that records a claim OPEN by identifier can still close it. The new exact-key inheritance can also close a distinct same-identifier finding (P3). Neither has a live effect today, but the section's fail-closed guard is weaker than at 8966da2e, which warrants REVISE.
