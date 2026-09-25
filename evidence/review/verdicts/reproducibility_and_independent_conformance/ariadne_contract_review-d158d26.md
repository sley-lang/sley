<!-- engine: claude-code; observed model: claude-opus-5-5; scope: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e; role: ariadne; field: ariadne_contract_review; dispatched: 2026-09-23T05:26:58Z; duration_s: 962; process_exit_code: 0 -->
The harness can no longer capture command output: `/tmp` is down to 14 free inodes. I'll report the first-run results I captured before it filled.

# Ariadne Council review — reproducibility_and_independent_conformance
Harness: claude-code
Reviewed checkpoint: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e

What I verified. I made no edits to the tree and wrote no files in it. Every probe ran in memory (plain `python3` heredocs) or in `tempfile` git fixtures built from `bench.review.tests.test_retire_review_claims.RetireReviewClaimsTests.setUp` and removed with `doCleanups`.

- **Scope.**
  - `git rev-parse HEAD` returned `d158d26bfdda9fc7dc899a1d1a8a141c2da6610e`, so the scope matches.
  - `git log --format='%h %s' 8966da2e..HEAD` lists acbc65f0, 883361e3, f7f9af90, 5fd10849, 0f4b8408, 7c51710b, 8d063f00 and d158d26b.
  - `git diff --name-only 8d063f00..d158d26b` lists 19 files, all under `evidence/` or `machineresearch/`, so the mint is records-only.
  - `git diff --stat 8966da2e..HEAD` shows 53 files. I read the diffs of `docs/spec/FINDING_REGISTER_V1.md`, ADR-0042 and the test-name list of both register test files.
  - `git diff --stat 8966da2e..HEAD` shows no change to `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md`, `scripts/check_reproducibility_and_independent_conformance.py`, `scripts/build_reproducibility_report.py`, `bench/release/tests/test_reproducibility.py` or `evidence/review/rounds/revision-7-retirement-changes.json`.
  - I read the commit messages of acbc65f0, 0f4b8408 and d158d26b.
- **Checkers and suites:**
  - `python3 scripts/check_reproducibility_and_independent_conformance.py`
    - First run: `problems: []`, `result: PASS`, `S20_730_MECHANICS_IMPLEMENTED_REVIEW_PENDING`.
    - A later rerun gave `result: FAIL`, `problems: ["release-tests:fail"]`. At that point `/tmp` (tmpfs) was at 100% inode use (61 free). Running `unittest discover -s bench/release/tests` then gave `Ran 156 tests … FAILED (errors=2)`, both `OSError: [Errno 28] No space left on device` under `/tmp`. That failure is environmental, not the tree.
  - `python3 scripts/check_finding_register.py`: `problems: []`, `result: PASS`, `S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING`. That run was piped through `tail`, so the exit status was not captured; a later capture attempt was lost to the harness's ENOSPC.
  - `python3 scripts/retire_review_claims.py --check`: exit 0, and a second run reproduced it. `retirable 0`, `stale_closures []`, `regeneration_divergence []`, `split_status []`, 9 report-only `closer_disagreements` (one in this section: `nabu_architecture_review_revision_10` token P3 vs `nabu_architecture_review-b58ac1e.md` P4), `result: PASS`.
  - `python3 scripts/build_reproducibility_report.py --check`: exit 1, a bare `result: FAIL`, `SINGLE_HOST_REPRODUCIBLE`, `distinct_hosts 1`.
    - Cause: `evidence/runtime/` is gitignored (`git status --ignored`), and this worktree's local S20-720 record is still the 69907ddf/ca7317a9 one.
    - In memory, `local_attestation("primary", <sley2-candidate-8d063f0 worktree>/evidence/runtime/s20-720-release-candidate/evidence.json)` → `carried_attestations` → `build_report` equals the tracked report byte for byte.
    - `sha256(sley2-candidate-8d063f0/dist/sley-2.0.0-linux-x86_64.tar.gz)` = `ecf51188…c1f7`, 2,508,082 B.
  - `python3 -m unittest discover -s bench/review/tests -t .`: `Ran 170 tests … OK`.
  - `python3 -m unittest bench.release.tests.test_reproducibility`: `Ran 50 tests … OK`, run twice.
- **Code read at HEAD:**
  - `scripts/build_finding_register.py`: :871-940 (`identity_tokens`, `shared_vocabulary`, `shares_its_kind`), :1051-1066 (`strictly_later_scope`), :1073-1075 (`STOP_WORDS`), :1174-1281 (`line_speaks_about`), :1284-1320 (`transcript_path`, `is_tracked`), :1453-1733 (`_own_identifier`, `CARRIED_FROM`/`LEADING_CARRY`, `finding_key`, `same_finding`, `unquoted`, `is_carry`, `named_carry_sha`, `package_restated_claims`, `package_open_findings`).
  - `scripts/retire_review_claims.py`: :216-645.
  - `scripts/build_reproducibility_report.py`: :25-30, :80-120, :440-576.
  - `scripts/build_ga_acceptance_report.py`: :300-345.
  - `docs/spec/FINDING_REGISTER_V1.md`: :1-230, :455-494.
  - `bench/review/tests/test_retire_review_claims.py`: :1-80, :611-793.
  - `bench/review/tests/test_finding_register.py`: :394, :536-545, :585-596, :981-1030.
- **Records read:**
  - `machine-summary.json`: this section, all lane fields, plus a field-by-field diff against `git show 8966da2e:…`.
  - `claim-retirements.json`: 157 entries; [40], [142], [149], [152]-[155].
  - `finding-register.json`: `package_open_claims` / `package_open_findings`.
  - `reproducibility-report.json`, `provenance.json`, `candidate-content-checks.json`.
  - The GA report's criteria[37] row.
  - `resumption-8966da2-batch14.json`.
  - Transcript lines `ariadne_contract_review-1a9f0aa.md:18,20,22,24`, `-76ae15a.md:18`, `-6589c6e.md:19`, `-7622776.md:28`, `nabu_architecture_review-7622776.md:27`, `-b58ac1e.md:32`, `vulcan_surface_review-c04539b.md:25`, `-1a9f0aa.md:28`.
- **Probes:**
  1. Every `pN_restated_claims` entry ledger-wide (72) replayed against the named-sha, key and ambiguity rules.
  2. The 17 identifier-less named folds read one by one.
  3. The packaging `[predicate-precision]` carries.
  4. The standards live instance from my prior P2.
  5. Every closure present at 8966da2e and open at HEAD (8), each re-read under the retirement read, with and without the file basename in the cited line.
  6. Equal-key exemption pairs with no one-hop round link (21).
  7. Generic-head fixtures: an original plus a foreign-round carry sharing one identifier; two identifier-less carries naming different roots; quoted and backticked leading clauses.

## Evidence checked

**Status of each finding in my 8966da2e verdict (`ariadne_contract_review-8966da2.md`):**

- **[P2] [fail-open/identity] `other_key == own_key` equal-key exemption (build_finding_register.py:833 at 8966da2e; also the @8f774d0 `shared_vocabulary`/`same_finding` P2 at build_finding_register.py:824 it carried) — CLOSED.**
  - `shared_vocabulary` :927-929 now exempts only `other_key == own_key and own_key[3] and (is_carry(other) or is_carry(claim))`.
  - `same_finding` :1530-1537 is plain equality, and a named carry keys on its own identifier (:1508-1527). So neither `None == None` nor `"" == ""` is an identity.
  - `test_generic_head_under_carried_and_identifier_less_shapes` :625-650 pins my six shapes: leading and trailing carries → 1 retired at `#L2`; identifier-less originals, leading and trailing carries → 0; sha-less clauses → 1.
  - My own replay of two identifier-less carries naming different roots gives 0.
  - The standards live instance `vulcan_surface_review@6589c6e [record] claim-retirements.json … carried from 1a9f0aab` now keys `('vulcan','record','1a9f0aa','')`, so the exemption cannot apply to it.
  - Spec :126-134 and :173-179 are amended. A narrower latent residual is raised below at P4.
- **[P3] [record-accuracy/fold] ambiguous or mis-targeted fold root (retire_review_claims.py:373-378 at 8966da2e; the @8f774d0 first-listed-root copy at retire_review_claims.py:366) — CLOSED.**
  - `fold_restatements` :427-458 now resolves the root to the one same-key claim whose scope equals the named sha. It refuses zero or several candidates (:438-444), and `package_restated_claims` :1657-1674 replays the rule.
  - Ledger-wide, 72 of 72 folds are named carries into open roots. There are 0 sha mismatches, 0 key mismatches and 0 unnamed folds. The 17 identifier-less named folds are each true re-statements of their root, which I read one by one.
  - The packaging `[predicate-precision]` carries now fold into `revision_8@79fdcc6` / `revision_9@b58ac1e`, and `@76ae15a is_closure_line` absorbs nothing.
  - Fixtures: `test_an_ambiguous_named_root_stays_open` :778-788 and `test_nabu_fixtures_fold_only_into_the_carried_identifier` :708-725.
- **[P3] [fail-open/fold] closed-root weak whole-line read (retire_review_claims.py:382-390 at 8966da2e) — CLOSED.**
  - `closing_lines_name` :397-425 now requires the root's own cited lines to be in `speaking_lines`, with the strong read, the lane's shared vocabulary less the linked pair, and the OPEN-line refusal. The closing transcript must also strictly postdate the entry.
  - The ledger has 0 folds into closed roots.
  - Fixtures: :674-687, :708-722 and :727-752. The last also shows an identifier-bearing copy folding only once the cited line (`#L2`) names it.
- **[P3] [record-accuracy/fold] per-round copies over-count the open lists (FINDING_REGISTER_V1.md:31-37; section `p4_open` 219 entries, 105 keys at 8966da2e) — CLOSED** under the alternative closure I offered.
  - Contract :193-199 now states that `pN_open_count` is a claim count, and that the register reports the distinct findings as `package_open_findings`.
  - The register gives this section p4 271/162, p3 15/12 and p2 5/4 (claims/distinct).
  - The GA row, criteria[37], reads "24 open P0/P1/P2 findings … (24 of them per-package open claims, 23 distinct findings by finding key)" (`build_ga_acceptance_report.py:322-329`).
  - The stale §3 sentence is carried in the P4 below.
- **[P3] [record-accuracy] own-line closures ariadne_contract_review-1a9f0aa.md:18,20,24 (the copies at @6589c6e, @79fdcc6, @b58ac1e, @8f774d0 and @8966da2) — CLOSED.**
  - Entries [152] (1a9f0aa `#L18`, `#L20`, `#L22`, `#L24`), [154] (76ae15a `#L18`) and [155] (6589c6e `#L19`) bind all five claims I named exact-claim, plus the `@76ae15a [checker-binding]` twin of [149].
  - I read each cited line against its claim. :18 is `record-accuracy/regeneration`. :20 is retirement [10], `p4_closed_claims[1]`. :24 is `p3_closed_claims[4]`, db53894 :28. 76ae15a :18 is `record-provenance`, `p2_closed_claims[5]`. 6589c6e :19 is `record-accuracy/contract-accuracy`. All match.
  - All five now sit in `p3_closed_claims` / `p4_closed_claims` with `binding: exact-claim`.
- **[P4] [fail-open] `is_tracked` silent degradation (build_finding_register.py:1305-1320) — CLOSED.** A non-zero `git ls-files` now yields an empty set and refuses (:1319). The fixture at `test_finding_register.py:592-596` patches `subprocess.run` to `returncode=1`. The OSError path (no git binary) is documented in the docstring.
- **[P4] [fold-scope] `strictly_later_scope` equal scopes (build_finding_register.py:1051-1066) — CLOSED.** Code :1055 returns False on equal ids, spec :81-83 states the exception, test :542 asserts it, and the `or True` at :394 is gone.
- **[P4] [contract-vs-mechanics] stacked contradictory register paragraphs (FINDING_REGISTER_V1.md) — narrowed, remains open.** Fixed: :31-37 (named root), :84 (reopen → retire → fold), :113-120 (equality), `LEADING_CARRY` needs a sha (:1482), and the `regeneration_divergence` docstring :581-582. Remaining items are in Findings.
- **[P4] [record-accuracy] claim-retirements.json entry [40] — remains open.** It still carries `severities [3]` with four prefixes.
- **[P4] [contract-versioning] REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md revision 11 — remains open.** The spec is unchanged: :3 still reads revision 11, and the checker :35 still sets `CONTRACT_REVISION = 11`.
- **[P4] [contract] `attestation_chain` unnamed in the section contract — remains open.** There are 0 hits for it in the spec, and the chain now has 62 entries.
- **[P4] [compound-kind] per-phrase vs whole-tag shared-kind reading — remains open.** Spec :56-63 / :68-70 and `shares_its_kind` :936-940 are unchanged.
- **[P4] [record-accuracy] revision-7-retirement-changes.json rows [53],[54] — remains open.** The file is not in the diff.
- **[P4] [test-coverage] — narrowed, remains open.**
  - Now fixtured: the cell parenthetical (:996-998), the PRIOR union (`test_closers_union_closes_both_severities` :481), named-root folds and closed-root folds.
  - Still missing: a severity-mismatch closed or restated claim.

**Candidate identity.** The records agree:
- The report's single attestation is `8d063f00…`, artifact `ecf51188…`, host `primary`, with `distinct_hosts 1`, `SINGLE_HOST_REPRODUCIBLE`, second host `GATED_OPERATOR_LANE`, `ga_claimed false`, `publication_authorized false` and `superseded_attestations []`.
- The summary's `attested_commit` is `8d063f00…`, and the chain ends `69907ddf [primary]`, `8d063f00 [primary]`.
- Provenance `externalParameters.commit` is `8d063f00…`, and content checks record `8d063f00…` / `ecf51188…`.
- My 8966da2e verdict is recorded as `ariadne_contract_review_revision_10` with its exact token.

**Out of lane.** The REQ-11 change (`crates/sley-policy`) touches none of this section's owned paths.

## Findings

[P3] [contract-vs-mechanics/record-accuracy] scripts/build_finding_register.py:1260-1275 (span relates only when the line also names the claim's file basename, introduced at acbc65f0) vs docs/spec/FINDING_REGISTER_V1.md:98-100 ("an anchor's line span (`413-417,424-425`) relates without its file name") and :104-105 - new at acbc65f0. Revisions 8 and 9 never state the added condition. The re-derivation reopened three reviewer-verified closures in this section whose own heads name the claim's span: ariadne_contract_review-7622776.md:28 (":464-465"), vulcan_surface_review-c04539b.md:25 ("spec 501-504") and nabu_architecture_review-b58ac1e.md:32 (":846-852"). Replayed: :28 relates False as written and True once `build_reproducibility_report.py` is added to the line. With four more closures reopened under the equal-key-originals rule (:169-170; vulcan `transcript_for` ×2, nabu `[contract-drift]` ×2), 7 of the 8 closures reopened ledger-wide since 8966da2e are in this section. No round record lists them (resumption-8966da2-batch14.json has 0 hits); the acbc65f0 message mentions only "one span-rule … exact-claim binding" - Closure: state the basename condition in the contract under a revision bump and amend :98-100, or drop it; bind the three span closures exact-claim or record them reopened with the refusing rule; list the round's reopened closures in a change record.

[P4] [contract-vs-mechanics] docs/spec/FINDING_REGISTER_V1.md:104-106 ("or a path with a tag word standing there" under the strong read) vs scripts/build_finding_register.py:1281 (`(not strong) and …`); §3 :475-477 ("folds into the earliest claim") vs :152-156 and scripts/retire_review_claims.py:427-445; scripts/retire_review_claims.py:640 (`--fold-restatements` help "into its earliest claim") - carried, narrowed (the :31-37, :84, :113-120, `LEADING_CARRY` and :581-582 docstring contradictions are fixed) - Closure: amend the three texts to the named-round and strong-read rules.

[P4] [fail-open/identity-residual] scripts/build_finding_register.py:927-929 (the equal-key exemption fires on any carry marker, not on the carry's named round) vs docs/spec/FINDING_REGISTER_V1.md:152-156,187-189 (named-round rule) and :193-195 ("a lane may raise a distinct finding on an identifier it used before") - new. Probe: an original `closure_head` claim at :10, plus `… `closure_head` bound is missing - carried from 0000000` at :90, under one generic `[robustness] scripts/build_finding_register.py — CLOSED` head, retires 2 (`#L1,2`); the same pair without the carry retires 0. It is latent: the 21 ledger pairs exempted with no one-hop round link are all multi-hop carry chains of one identifier - Closure: exempt only when the carry's named sha is the other claim's scope or the same named root (the fold's rule); add this fixture.

[P4] [reproducibility/diagnostics] scripts/build_reproducibility_report.py:533-547 (`--check` prints a bare `FAIL`) with :26,80-120 (derives from gitignored evidence/runtime/s20-720-release-candidate/evidence.json) - new. In this review worktree the local evidence is the 69907ddf/ca7317a9 candidate, while the tracked report attests 8d063f00/ecf51188. The check fails with no detail, indistinguishable from tracked-record drift. The tracked report does regenerate byte-identically from the mint worktree's evidence (in-memory replay) - Closure: name the local evidence commit and the tracked attestation commit in the FAIL, or refuse with a distinct stale-evidence code; add a fixture.

[P4] [test-coverage] scripts/build_finding_register.py:1376 and :1619-1624 (raising-severity binding on `pN_closed_claims` / `pN_restated_claims`) vs bench/review/tests/test_finding_register.py:1001-1028 (the unraised `None` case only) - carried, narrowed (cell parenthetical, PRIOR union, named-root and closed-root folds are now fixtured) - Closure: add a claim raised P3 but listed in `p4_closed_claims`, and one in `p4_restated_claims`, each refused `SUMMARY_INVALID`.

[P4] [record-accuracy] evidence/review/claim-retirements.json entry [40] - carried, unchanged: `severities [3]` with four prefixes, three of them P4s - Closure: trim [40] to its P3 prefix.

[P4] [contract-versioning] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:3 (revision 11) with scripts/check_reproducibility_and_independent_conformance.py:35 - carried, byte-unchanged - Closure: bump to revision 12 with a section 14 entry, or state an erratum policy.

[P4] [contract] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md (0 hits for `attestation_chain`) - carried: the checker binds the now 62-entry chain with refusals no contract sentence names - Closure: name the field, its derivation and its refusals under the revision bump.

[P4] [compound-kind] docs/spec/FINDING_REGISTER_V1.md:56-63 vs :68-70 and scripts/build_finding_register.py:936-940 - carried, unchanged: the per-phrase exclusion and the whole-tag strong mode still disagree, and the fixture is single-phrase - Closure: state the governing reading; align the mechanics or bind by exact-claim; add a compound-kind fixture.

[P4] [record-accuracy] evidence/review/rounds/revision-7-retirement-changes.json:450,458 (rows [53], [54]) - carried, unchanged: "closes a different finding or only half of this one" is false for vulcan c04539b :26 and :24 - Closure: replace the clause, or record the bindings.

## Assessment

Every P2 and P3 in my 8966da2e verdict is CLOSED on verified evidence: the identity-by-absence P2, the three fold/count P3s and the own-line closure P3. Two P4s are also CLOSED (`is_tracked`, the strict scope). The other seven P4s are carried, two of them narrowed. My earlier P2's fixture passes, all 72 ledger folds follow the named-sha rule with none ambiguous, and my five own-line closures are bound exact-claim to lines I re-read.

The candidate records consistently bind one primary, single-host, reproducible attestation of 8d063f00/ecf51188. I confirmed that attestation against the mint worktree's evidence and artifact hash.

The mechanisms held against most of my probes. There are two residuals:
- **P3.** The acbc65f0 span-needs-basename rule was never written into the contract. It reopened three closures in this section whose reviewers' own heads name the claim's span, and no round record lists the reopenings.
- **P4 (latent).** The equal-key vocabulary exemption fires on any carry marker rather than the named round.

In this worktree, `build_reproducibility_report.py --check` fails only because the gitignored local evidence is stale, and it gives no diagnostic (P4). The late `release-tests:fail` came from `/tmp` running out of inodes.

No P0–P2 remains in my lane's delta, so the verdict is PASS carrying one P3 and nine P4s. I make and endorse no GA, release-readiness or out-of-scope claim.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_9_P4
SECTION: reproducibility_and_independent_conformance
FIELD: ariadne_contract_review
SCOPE_SHA: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e
FINDINGS: [P3] [contract-vs-mechanics/record-accuracy] scripts/build_finding_register.py:1260-1275 (span needs the file basename, new at acbc65f0) vs docs/spec/FINDING_REGISTER_V1.md:98-100,104-105 - the rule is unstated in revisions 8/9 and reopened three reviewer-verified span closures in this section (ariadne_contract_review-7622776.md:28 ":464-465", vulcan_surface_review-c04539b.md:25 "501-504", nabu_architecture_review-b58ac1e.md:32 ":846-852"); 7 of the 8 closures reopened ledger-wide since 8966da2e are in this section and no round record lists them - Closure: state or drop the condition (revision bump, amend :98-100); bind or record the three; add a reopened-closure change record; [P4] [contract-vs-mechanics] docs/spec/FINDING_REGISTER_V1.md:104-106 (path+tag under the strong read) vs scripts/build_finding_register.py:1281, §3 :475-477 and scripts/retire_review_claims.py:640 ("earliest claim") - carried, narrowed - Closure: amend the texts; [P4] [fail-open/identity-residual] scripts/build_finding_register.py:927-929 - the exemption fires on any carry marker regardless of the named round; generic-head probe retires 2 vs 0; latent (21 ledger pairs are chains of one identifier) - Closure: require the named round or the same named root; add a fixture; [P4] [reproducibility/diagnostics] scripts/build_reproducibility_report.py:533-547 - bare FAIL when the gitignored local S20-720 evidence is from another candidate (69907ddf vs tracked 8d063f00) - Closure: name both commits or use a distinct stale-evidence code; add a fixture; [P4] [test-coverage] scripts/build_finding_register.py:1376,1619-1624 - no severity-mismatch closed/restated fixture - Closure: add both; [P4] [record-accuracy] evidence/review/claim-retirements.json entry [40] - `severities [3]` with four prefixes - Closure: trim; [P4] [contract-versioning] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:3 with checker :35 - revision 11 unchanged - Closure: revision 12 or an erratum policy; [P4] [contract] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md - `attestation_chain` (62 entries) unnamed - Closure: name its field, derivation and refusals; [P4] [compound-kind] docs/spec/FINDING_REGISTER_V1.md:56-63 vs :68-70, scripts/build_finding_register.py:936-940 - unchanged - Closure: governing reading, alignment, fixture; [P4] [record-accuracy] evidence/review/rounds/revision-7-retirement-changes.json:450,458 - false refusing-rule clause - Closure: replace or bind.
SUMMARY: Every P2/P3 of my 8966da2e verdict is CLOSED on verified evidence: the absence-identity P2 fixture passes, 72 of 72 folds follow the named-sha rule with none ambiguous, closed-root folds use the retirement read, claim and distinct counts are both stated and reported, and my five own-line closures are bound exact-claim; `is_tracked` and strict-scope P4s are also closed. The 8d063f00/ecf51188 single-host attestation is consistent across records, and the tracked report regenerates byte-identically from the mint's evidence; the review checkers and 170 + 50 tests pass. The residuals are one P3 (the acbc65f0 span-basename rule is missing from the contract and reopened three verified closures in this section, unrecorded) and nine P4s, including a latent round-blind vocabulary exemption and a detail-free reproducibility `--check` failure on stale local evidence. No GA or publication claim is made.
