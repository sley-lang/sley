<!-- engine: claude-code; observed model: claude-opus-5-5; scope: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e; role: nabu; field: nabu_architecture_review; dispatched: 2026-09-23T05:26:58Z; duration_s: 1146; process_exit_code: 0 -->
# Nabu Council review — release_candidate_packaging

Harness: claude-code
Reviewed checkpoint: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e

**What I verified.** I worked read-only and wrote nothing to the tree. Probes ran from stdin against the imported modules, against deep copies of the on-disk ledger, or in scratch git fixtures under `~/.cache`. `/tmp` was at 100% inode use machine-wide during the session, so fixtures could not use it.

Git commands run:
- `git rev-parse HEAD` → `d158d26bfdda9fc7dc899a1d1a8a141c2da6610e`, so the scope matches. The working tree is clean.
- `git log --format='%h %s' 8966da2e..HEAD` → acbc65f0, 883361e3, f7f9af90, 5fd10849, 0f4b8408, 7c51710b, 8d063f00, d158d26b.
- `git diff --stat 8966da2e..HEAD`: 53 files, +4691/−785. `git diff --name-only 8966da2e..HEAD` outside evidence/ and machineresearch/ gives 16 files.
- I read the full diffs of `scripts/build_finding_register.py`, `scripts/retire_review_claims.py`, `scripts/build_ga_acceptance_report.py`, `scripts/check_release_candidate_packaging.py`, `scripts/check_finding_register.py`, `bench/release/tests/test_packaging.py`, `bench/review/tests/test_finding_register.py`, `docs/spec/FINDING_REGISTER_V1.md`, `docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md` and ADR-0042.
- `git diff --name-only 8d063f00..HEAD` lists only evidence/ and machineresearch/ records, so the mint is records-only.
- `git show b58ac1e0:scripts/retire_review_claims.py` gave me the pre-union `closers` body.
- Per-commit `git ls-tree -r` counts compared with T54 `candidate_files_scanned`: 2189 tracked, minus the two T52/T54 outputs, = 2187 scanned.
- `sha256sum dist/sley-2.0.0-linux-x86_64.tar.gz`.

Checkers run (all exit 0):
- `python3 scripts/retire_review_claims.py --check` → `retirable 0`, `stale_closures []`, `regeneration_divergence []`, `split_status []`, nine report-only `closer_disagreements`, `"result": "PASS"`.
- `python3 scripts/check_release_candidate_packaging.py` → `"result": "PASS"`, `"problems": []`, `revision 7`, `S20_720_MECHANICS_IMPLEMENTED_REVIEW_PENDING`.
- `python3 scripts/check_finding_register.py` → `"result": "PASS"`, `"problems": []`, `S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING`.
- `python3 -m unittest discover -s bench/review/tests -t .` → `Ran 170 tests`, `OK`, rc 0.
- `python3 -m unittest discover -s bench/release/tests -t .` → `Ran 156 tests`, `OK`, rc 0.
- I did not run `cargo test`. The only Rust change is REQ-11, which is out of lane and PASSed by its own lanes; the packaging checker PASSes against the re-minted candidate.

Files read: `nabu_architecture_review-8966da2.md:1-94` (my prior verdict); `scripts/retire_review_claims.py:85-351` (`strictly_later`, `transcript_closers`, `closers`, `speaking_lines`, `retire`) and the new `fold_restatements` :354-492, `split_status_problems`, `closer_disagreements`; `bench/review/tests/test_retire_review_claims.py:1-600, 674-793`; `docs/spec/FINDING_REGISTER_V1.md:146-206` (the revision 8 and 9 paragraphs); `evidence/review/rounds/resumption-8966da2-batch14.json`; `scripts/generate_supply_chain_evidence.py:300-340`.

Records read: `machineresearch/sley-2.0/machine-summary.json` (packaging `candidate_*`, P0–P4 open/closed/restated lists, the live nabu field and `current_delta_review`); `evidence/review/finding-register.json` (`result`, `contract_revision`, `obligation_count`, `package_open_claims`, `package_open_findings`, `package_closed_claims`, `package_restated_claims`, `unclaimed_carried_findings`); `evidence/release/ga-acceptance-report.json` criterion 37; `evidence/release/reproducibility-report.json`; `evidence/security/T54/secret-scan.json`.

In-memory probes:
1. **Every re-statement in the ledger.** I classified all 72 `pN_restated_claims` entries:
   - root states: 60 open, 12 chained through carries to open roots, 0 retired, 0 dangling;
   - 72 of 72 name a sha equal to their root's scope;
   - 0 have a key that differs from their root's.
2. **Regenerate vs incremental reads.** `shares_its_kind` for all 1129 ledger claims, tracked ledger vs `reopen_all` copy: 0 flips (48 at 8966da2e).
3. **`closers` union.** The review suite with `closers` replaced in-process by its b58ac1e0 body: 39 run, 1 failure (`test_closers_union_closes_both_severities`).
4. **Closer ancestry sort.**
   - Sort reversed: 1 failure (`test_closers_bind_the_earliest_transcript_and_prefer_own_head_lines`).
   - Sort by path only, 8 runs: pass ×4, fail ×4. The outcome depends on the fixture's commit hashes.
5. **Three-round fixture** (the one from my last review).
   - (A) `alpha_fn` closed at round 2, `beta_fn` raised at round 2, a round-3 `(carried from <r2>, OPEN) … beta_fn`: retire 1, fold 1. The carry restates the open `beta_fn` root.
   - (B) a carry naming round 1 but carrying `beta_fn` stays open.
   - (C) a round-3 carry of `alpha_fn` whose closer (round 2) predates it stays open. `split []`, as the spec intends.
6. **Exact-key inheritance fixture** (`closing_lines_name` + pass 2). Two distinct `alpha_fn` findings: `scripts/a.py:10` "drops the sha" at r1 and `scripts/a.py:50` "leaks a handle on error" at r2.
   - Generic r3 line `` `alpha_fn` sha handling — CLOSED ``: retire 0, fold 0. Fail-closed.
   - r3 line specific to the first finding, `` scripts/a.py:10 `alpha_fn` drops the sha — CLOSED ``: `retired=1 folded=1 open=[]`. The r2 finding is filed as a re-statement of the retired r1 claim; `split []`.
7. **Identifier rule.** Across all 1129 claims, 11 have a first backticked identifier that is a real `def` in scripts/ but not the claim's key: `package_open_claims`/`package_closed_claims`/`package_restated_claims` (now ledger vocabulary) and `retire` (a substring of the file stem). Keys also land on non-identifiers: `nabu`, `b9e2382f`, `_1_P3_9_P4`, `bench`, `assertTrue`.
8. **Candidate identity.**
   - `candidate_commit` = `candidate_evidence_commit` = 8d063f00133f369599032191708fa7efabb3e2fa; artifact ecf51188…c1f7, 2508082 bytes, manifest f0bd4072…, 15 members, `REPRODUCIBLE`.
   - The artifact and/or commit appear in candidate-content-checks, provenance, CycloneDX, SPDX (by artifact), dossier, GA report, reproducibility report (one attestation, `distinct_hosts 1`, `SINGLE_HOST_REPRODUCIBLE`, `superseded_attestations []`), lint report and the T15/T47/T56 matrices.
   - Superseded identities 69907ddf, ca7317a9, 8966da2e, 8f774d0c, b58ac1e0, 4e128def: 0 occurrences in any of these records.
9. **Register.** `FINDING_REGISTER_OPEN`, contract revision 9, 491 obligations, `unclaimed_carried_findings []`.
   - Packaging: `p2_open` 7, `p3_open` 5, `p4_open` 167 claims / 131 distinct findings.
   - GA criterion 37: "24 open P0/P1/P2 findings … (24 of them per-package open claims, 23 distinct findings by finding key)", with `ga_claimed false`.

## Evidence checked

**My 8966da2e findings, each checked at HEAD:**

- **[P2] [fold-identity] scripts/build_finding_register.py:1330,1342 (`finding_key`, `same_finding` wildcard) with scripts/retire_review_claims.py:373-390 (`fold_restatements` root and retired-root guard) — CLOSED.**
  - `finding_key` now keys a named re-statement on its own identifier (`_own_identifier`, :1505-1506), and `same_finding` is plain equality (:1536).
  - `fold_restatements` resolves the root by `named_carry_sha` among same-key claims at that scope. Two same-key claims at the named round are ambiguous and stay open.
  - A fold into a retired root requires the root's cited closing lines to speak about the re-statement through `speaking_lines`, which includes `open_lines_about` and the strong read. The closing transcript must also strictly postdate the re-statement (:397-423).
  - On disk:
    - Ariadne's three 8f774d0c P4 `[predicate-precision]` statements and their b58ac1e re-statements now fold into the open 79fdcc6/b58ac1e roots they name (`scoped_before`, `is_lane_field_name`, `claim_finding_ids`).
    - The `vulcan@79fdcc6 [fail-closed-gap]` `raising_severity` standards P3 is back in `p3_open`.
    - No re-statement restates a retired root.
  - My fixtures A/B/C behave as required. Tests pin the named root, the absent root, the closed twin, the postdating closer and the ambiguity case.
  - This line also covers the 8f774d0c statement of the same finding (`nabu_architecture_review_revision_6@8f774d0` [fold-identity] :1307,1317,1330).
- **[P4] [ledger-duplication] machine-summary.json packaging `p4_open` "114 listed / 97 distinct" — CLOSED.** `package_open_findings` publishes the distinct count beside the claim count (`test_open_findings_count_distinct_identities`), and the GA row states both.
- **[P4] [duplicated-authority] `shared_vocabulary` `restates` exemption, build_finding_register.py:815-818,833 — CLOSED.** The exemption is now derived from the claims (equal non-empty key plus a carry). I measured 0 flips across 1129 claims, and `test_duplicated_authority_reads_agree_with_and_without_the_link` covers it.
- **[P4] [contract-vs-mechanics] `LEADING_CARRY` without a sha and the unreferenced `CARRY`, build_finding_register.py:1296,1300 — CLOSED.** The leading clause now requires `from <sha>`, `CARRY` is deleted, and `test_sha_less_carry_clauses_are_originals` covers it.
- **[P4] [test-coverage] retire_review_claims.py:190-200 `closers` union — CLOSED.** The b58ac1e0 body fails `test_closers_union_closes_both_severities`.
- **[P4] [record-precision] build_finding_register.py:754-758 `is_open_line` bare `OPEN` — CLOSED.** A bare OPEN now counts only when followed by `(`, `:` or end of line; both of my probe shapes are tested, and `nabu@6589c6e [fail-open-exemption]` is now retired.
- **[P4] [test-coverage] bench/review/tests/test_retire_review_claims.py:231-245 (now :235-249) `test_closers_bind_the_earliest_transcript_and_prefer_own_head_lines` — OPEN.** The test is unchanged. It catches a reversed sort but passes 4 of 8 runs with the sort removed.
- **[P4] [record-precision] build_finding_register.py:574,593 `raising_scope` 80-character prefix — CLOSED.** The full carry-stripped description is matched first and the prefix is only a fallback. `_finding_line_match` has a collision test.
- **[P4] [record-precision] build_finding_register.py:794,1065 identifier rule with claim truncation — CLOSED.** The truncated stem no longer keys (`p4_open` @79fdcc6 now keys `assertTrue`), and the standards 6589c6e and 7622776 `[fail-closed-gap]` keys now differ. The rule's over-reach is filed below as a new P4.
- **[P4] [test-coverage] test_finding_register.py:392 `or True`, `is_tracked` failure branch, `replay_problems` exact-claim branch — CLOSED.** `or True` is removed, the rc=1 mock refuses, and `test_replay_exact_claim_branch` exists.
- **[P4] [check-precision] retire_review_claims.py:428-446 `regeneration_divergence` on `verified_by.split("#")[0]` — CLOSED.** It now compares the full reference, and `test_regeneration_keys_on_the_full_reference` covers it.
- **[P4] [unbound-record] evidence/review/rounds/revision-7-retirement-changes.json — CLOSED.** `test_retirement_change_record_is_keyed` now reads it and pins its keying. Its content records a superseded revision's changes, so I no longer require it to be regenerated.
- **[P4] [fail-open-default] build_finding_register.py:1151-1163 `is_tracked` git rc≠0 — CLOSED.** rc≠0 now yields an empty set, and a test covers it. An `OSError` (no git binary) still returns True, but that is documented, and every other ordering call in both tools would raise without git.
- **[P4] [record-consistency] nabu_architecture_review-7622776.md:81 `_1_P3_9_P4` — OPEN.** The ledger still records 1 P3 + 8 P4 `nabu_architecture_review@7622776:` claims, with no note correction and no token-vs-claims check.
- **[P4] [duplicated-predicate] scripts/build_ga_acceptance_report.py:316-322,592 — CLOSED.** An open list without its `_count` is now `SUMMARY_INVALID` at register build (`package_open_claims`, with a test). A list-only section can no longer reach the GA sum, and the row now states claims and distinct findings.
- **[P4] [local-state] dist/sley-2.0.0-linux-x86_64.tar.gz — OPEN.** The archive is still `b9e2382f…`/2507941 bytes, while the records name `ecf51188…`/2508082.

**Delta review: ownership, direction, fail-closed structure, identity.**
- **Direction holds.** `retire_review_claims.py` imports `named_carry_sha`, `finding_key`, `same_finding`, `is_carry`, `shared_vocabulary` and `open_lines_about` from the builder. The builder imports nothing from the tool.
- **The builder's replay of folds is deliberately partial.** Exact-key inheritance skips the carry, later-scope and closing-line checks (build_finding_register.py:611-650). The full rule is enforced only by `--check` regeneration, which is wired into the Makefile (:91, :284) beside `check_finding_register.py`. The spec documents this exemption; it is acceptable because `--check` gates.
- **The new fail-closed refusals are delivered and tested:**
  - `tagged_claim_unraised` in the open, closed and restated lists;
  - a missing `_count`;
  - `split_status_problems`;
  - equal-scope `strictly_later_scope`.
- **Where it fails open** is the second fold pass (P2 below).

## Findings

[P2] [fail-open] scripts/retire_review_claims.py:397-423 (`closing_lines_name` drops the retired twin from the vocabulary view at :413-416 before `speaking_lines` at :417) with :462-490 (the second, exact-key inheritance pass) vs docs/spec/FINDING_REGISTER_V1.md:182-195 - The inheritance treats an equal identifier-bearing key as one finding once the retired claim's closing line names the identifier, so a distinct finding that shares an identifier inherits a closure written for another finding. The spec's own premise, "a lane may raise a distinct finding on an identifier it used before", says this is wrong. The read is weaker than `retire` itself: in my fixture, `` `alpha_fn` `` at `scripts/a.py:10` "drops the sha" (r1) and `` `alpha_fn` `` at `scripts/a.py:50` "leaks a handle on error" (r2), with the r3 line `` scripts/a.py:10 `alpha_fn` drops the sha — CLOSED ``, give `retire` 1 (the first finding only), then `fold_restatements` 1. The second finding leaves `p4_open` as a re-statement of the retired claim, although no line judged it. `split_status` is [] and `--check` reproduces the fold. The same path would drop a P0–P2 claim from the GA gate count. It is latent: 0 exact-key inheritances on disk at HEAD, and test_retire_review_claims.py:746-752 pins only a line that names the copy's own span - Closure: inherit only when a cited closing line speaks about the inheriting claim under the unreduced retirement read (the read `retire` applies), or when the line's head names the inheriting claim's own anchor or distinguishing phrase; otherwise leave it open or refuse it as a split. Add a test with two distinct same-identifier findings and a line specific to one of them, asserting the other stays open; regenerate.

[P4] [identity-discipline] scripts/build_finding_register.py:1457-1475 (`_own_identifier`: `word.lower() in stem` drops any backticked word that is a substring of a cited file's stem), :1085-1094 with :1109-1122 (the register's own functions `package_open_claims`/`package_closed_claims`/`package_restated_claims` are now ledger vocabulary) - 11 ledger claims lose their function identifier. For example, the packaging `p3_open` `nabu@7622776 [verification-depth]` `package_closed_claims` claim keys "". `retire` in retire_review_claims.py keys on `binding`, `fold_restatements` or `bench`. Keys also land on non-identifiers such as lane names (`nabu`), shas (`b9e2382f`) and verdict tokens (`_1_P3_9_P4`). As a result, carries do not meet their roots: my `[local-state]` finding is listed six times in `p4_open`, as the untagged root keyed "" plus five carries keyed `b9e2382f` - Closure: exclude only a word equal to a cited path's stem (or its truncated tail); read a ledger word as vocabulary only outside a function-call or code context; refuse hex-only, lane-name and verdict-token words as identifiers. Add tests with `retire` in retire_review_claims.py and `package_closed_claims` in build_finding_register.py.

[P4] [test-coverage] (carried from b58ac1e0, OPEN) bench/review/tests/test_retire_review_claims.py:235-249 (`test_closers_bind_the_earliest_transcript_and_prefer_own_head_lines`) - unchanged; it fails when the ancestry sort is reversed, but with a filename-only sort it passed 4 of 8 runs, because the outcome depends on the fixture's commit hashes - Closure: fixed transcript names, or closers built in-memory, so filename order and ancestry order disagree on every run; assert the binding.

[P4] [record-consistency] (carried from c67b0729, OPEN) evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-7622776.md:81 count token `_1_P3_9_P4` vs 1 P3 + 8 P4 recorded `nabu_architecture_review@7622776:` claims - no note correction - Closure: record the reconciliation in the note, and add a register check that a scope-tagged lane's token equals its recorded claims.

[P4] [local-state] (carried from c04539b9, OPEN) dist/sley-2.0.0-linux-x86_64.tar.gz (gitignored) - the local archive is still the superseded 2507941-byte build while the records name ecf51188…/2508082 - Closure: replace it with the current archive or remove it.

## Assessment

The acbc65f0 + 0f4b8408 repair closes my `[fold-identity]` P2, which I checked against the code and a from-scratch fixture rather than against the note. A re-statement now keys on its own identifier. `same_finding` is equality. The fold resolves its root by the sha the carry names, leaves ambiguous or absent roots open, and folds into a retired root only when a strictly later closing line speaks about the re-statement.

On disk every one of the 72 re-statements names its root's scope, none restates a retired root, and the Ariadne and Vulcan findings I named are back in their open roots. The shared-vocabulary reads agree on both paths (0 of 1129 flips). Twelve of my fifteen prior P4s close on evidence I recomputed.

The candidate re-minted at 8d063f00 (ecf51188…, 2508082 bytes, 15 members, one clean single-host attestation) binds identically across every release record, with no superseded identity. The mint is records-only. Three checkers PASS, 170 + 156 tests are green, the ledger regenerates without divergence or split, and the GA row honestly states 24 claims / 23 distinct P0–P2 findings with `ga_claimed false`.

What is wrong is the round's second fold pass. Exact-key inheritance removes the retired twin from the vocabulary before reading the closing line. A closure line written for one finding then absorbs a distinct finding that shares only its identifier, even though `retire` refused that finding on the same line, and `--check` accepts the result. The pass is latent today but fails open in the mechanism that decides whether P0–P2 findings remain open, so it is a P2.

The new identifier rule drops real function names and keys on shas, lane names and verdict tokens (a new P4). Three prior P4s remain. Nothing here claims GA or release readiness; the second host remains an operator-gated lane.

VERDICT: REVISE_0_P0_0_P1_1_P2_0_P3_4_P4_PRIOR_P2_CLOSED
SECTION: release_candidate_packaging
FIELD: nabu_architecture_review
SCOPE_SHA: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e
FINDINGS: [P2] [fail-open] scripts/retire_review_claims.py:397-423 (`closing_lines_name` drops the retired twin from the vocabulary view) with :462-490 (the exact-key inheritance pass) vs docs/spec/FINDING_REGISTER_V1.md:182-195 - a distinct same-identifier finding inherits a closure written for another: in the fixture, `alpha_fn` scripts/a.py:10 closed by a line specific to it, the r2 `alpha_fn` scripts/a.py:50 finding that `retire` refused is filed as a re-statement with split [] and --check clean; latent, 0 on disk - inherit only under the unreduced retirement read or a head naming the inheriting claim's own anchor or phrase, test, regenerate; [P4] [identity-discipline] scripts/build_finding_register.py:1457-1475 (`_own_identifier` substring stem rule) with :1085-1094,1109-1122 (`package_*_claims` as ledger vocabulary) - 11 claims lose their function identifier, keys land on `nabu`/`b9e2382f`/`_1_P3_9_P4`, the `[local-state]` finding is listed six times - exact-stem exclusion only, refuse hex/lane/token words, tests; [P4] [test-coverage] (carried from b58ac1e0, OPEN) bench/review/tests/test_retire_review_claims.py:235-249 (`test_closers_bind_the_earliest_transcript_and_prefer_own_head_lines`) - passes 4 of 8 runs without the ancestry sort - make filename and ancestry order disagree every run, assert; [P4] [record-consistency] (carried from c67b0729, OPEN) evidence/review/verdicts/release_candidate_packaging/nabu_architecture_review-7622776.md:81 `_1_P3_9_P4` vs 1 P3 + 8 P4 recorded - note correction and token-vs-claims check; [P4] [local-state] (carried from c04539b9, OPEN) dist/sley-2.0.0-linux-x86_64.tar.gz - superseded 2507941-byte build vs records ecf51188…/2508082 - replace or remove.
SUMMARY: At d158d26b the 8966da2e-round repairs close my `[fold-identity]` P2 as filed. Re-statements key on their own identifier, fold only into the claim at the round they name, and fold into a retired root only through a strictly later closing line that speaks about them; all 72 re-statements on disk name their root's scope, and twelve of my fifteen prior P4s close on recomputed evidence. Candidate 8d063f00 (ecf51188…, 2508082 bytes, 15 members, single host) binds identically across every release record, the mint is records-only, three checkers PASS, 170 + 156 tests are green, and the GA row honestly reads 24 claims / 23 distinct P0–P2 findings. One new P2 remains: exact-key inheritance reads the closing line with the retired twin removed from the vocabulary, so a line closing one finding also absorbs a distinct same-identifier finding that `retire` refused, with `--check` clean (latent, reproduced in a two-claim fixture). One new P4 on the identifier rule and three carried P4s also remain; nothing here claims GA or release readiness.
