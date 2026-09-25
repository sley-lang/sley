<!-- engine: claude-code; observed model: claude-opus-5-5; scope: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e; role: nabu; field: nabu_architecture_review; dispatched: 2026-09-23T05:43:00Z; duration_s: 1157; process_exit_code: 0 -->
# Nabu Council review — reproducibility_and_independent_conformance

Harness: claude-code
Reviewed checkpoint: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e

What I verified. I made no writes to the tree.
- **Scope.** `git rev-parse HEAD` returns `d158d26bfdda9fc7dc899a1d1a8a141c2da6610e`, so the scope matches.
- **Delta.** `git log --format='%h %s' 8966da2e..HEAD` lists acbc65f0, 883361e3, f7f9af90, 5fd10849, 0f4b8408, 7c51710b, 8d063f00 and d158d26b. `git diff --stat 8966da2e..HEAD` shows 53 files.
- **Section files.** Four of my section's own mechanics files are **byte-unchanged** 8966da2e..HEAD (empty diff): `scripts/build_reproducibility_report.py`, `scripts/check_reproducibility_and_independent_conformance.py`, `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md` and `bench/release/tests/test_reproducibility.py`. The only change to section evidence is the rebind of `evidence/release/reproducibility-report.json`.
- **Worktree.** At session start `git status --short` showed one untracked transcript. At the end it shows six untracked `*-d158d26.md` transcripts, filed concurrently by other lanes; none is mine.
- **Environment note.** My first checker and unittest runs failed spuriously. `df -i /tmp` showed `/tmp` at 100% inode use (1048576/1048576), and the failures were ENOSPC tempfile errors: 82 unittest errors, and `check_finding_register` reported `register-tests:fail`. I re-ran everything through a `python3 -` subprocess with `TMPDIR=/dev/shm`. Those results are the authoritative ones:
  - `python3 scripts/check_reproducibility_and_independent_conformance.py` → **exit 0**, `"result": "PASS"`, `"problems": []`, `S20_730_MECHANICS_IMPLEMENTED_REVIEW_PENDING`.
  - `python3 scripts/check_finding_register.py` → **exit 0**, `"result": "PASS"`, `"problems": []`, `S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING`.
  - `python3 scripts/retire_review_claims.py --check` → **exit 0**, `retirable 0`, `stale_closures []`, `regeneration_divergence []`, `split_status []`, `"result": "PASS"`. It also lists 9 report-only `closer_disagreements`; one is in this section (`nabu_architecture_review_revision_10`: the token closes P3, while the b58ac1e transcript records P4).
  - `python3 -m unittest discover -s bench/review/tests -t .` → `Ran 170 tests … OK`.
  - `python3 -m unittest bench.release.tests.test_reproducibility` → `Ran 50 tests … OK`.
- **Files read:**
  - `nabu_architecture_review-8966da2.md` (whole).
  - `git diff 8966da2e..HEAD` of `docs/spec/FINDING_REGISTER_V1.md` (current :1-6, :105-106, :110-205) and ADR-0042 (:1-6, :66-70).
  - `scripts/build_finding_register.py`, current line spans: :35, :152-209, :546-750, :800-1008, :1174-1343, :1450-1735.
  - `scripts/retire_review_claims.py`, current line spans: :216-351, :354-631.
  - `bench/review/tests/test_retire_review_claims.py` (:1-75, :560-793) and `bench/review/tests/test_finding_register.py` (:975-1019).
  - The `build_ga_acceptance_report.py` diff, and the `reproducibility-report.json` diff and fields.
- **In-memory probes (`python3 -`):**
  - The ledger's 72 re-statements: keys, named shas, and identifier-less re-statements that were folded into retired roots.
  - This section's open, closed and restated lists, plus `package_open_findings`.
  - The key of Ariadne's `[128]` finding.
  - Carry parsing of four shapes: `(prior from <sha>)` against the ledger, leading clause vs later phrase, a single-quoted carry and a stray backtick.
  - Mixed-status closure heads.
  - `line_speaks_about`, weak vs strong.
- **One tempfile git fixture (scenario C).** I created it under `/dev/shm`, outside the tree, and drove `retire.fold_restatements` end to end on it.

## Evidence checked

**Prior 8966da2e findings (nabu_architecture_review-8966da2: 1 P2 + 1 P3 + 18 P4).**

- **[P2] [fail-open/identity-discipline] scripts/build_finding_register.py:1485-1537 (`finding_key`, `same_finding`) — CLOSED.**
  - A named re-statement now keys on its own identifier (`_own_identifier(rest)`, :1514/:1525-1526), and `same_finding` is plain equality (:1537). No wildcard remains.
  - The fold resolves its root at the named sha and refuses ambiguity: `len(matching) != 1` → keep (retire :434-444).
  - Tests A and B exist: `test_nabu_fixtures_fold_only_into_the_carried_identifier` (test_retire_review_claims.py:708-725). The ambiguity test is at :778-788.
  - Regeneration reproduces the tracked ledger.
  - The nine ledger mis-bindings are gone. The Nabu `release_candidate_packaging` `(carried from b58ac1e0)` re-statements now restate their own b58ac1e originals: `is_open_line`→`is_open_line`, `raising_scope`→`raising_scope`. Ariadne `[predicate-precision]` carries bind `scoped_before`/`is_lane_field_name`/`claim_finding_ids` to their own originals.
  - The carried 8f774d0 version of this finding ("an absent identifier matches any") is closed by the same equality rule.
  - The one closure element not delivered as specified: the retired-root guard still reads weak when the kind is unshared. It now only matters for identifier-less keys, so I raise it separately below as a new, narrower P3.
- **[P3] [identity-discipline] scripts/build_finding_register.py:1481-1482,1540-1559 (`CARRIED_FROM`, `LEADING_CARRY`, `unquoted`, `is_carry`) — CLOSED.**
  - `CARRIED_FROM` is now searched over `unquoted(body)` (:1520, :1559, :1570).
  - `LEADING_CARRY` requires `from <sha>` (:1482).
  - Ariadne's `[128]` finding is back in this section's `p4_open`: `is_carry` False, `named_carry_sha` None, key origin `8f774d0` (its own round).
  - The test is `test_a_quoted_carried_from_is_not_the_claims_carry` (:652-672).
  - This section's `pN_restated_claims` are empty.
- **[P4] [identity-discipline/contract-revision] docs/spec/FINDING_REGISTER_V1.md:3; scripts/build_finding_register.py:35 — CLOSED.**
  - Revision 8 is at acbc65f0 and revision 9 at 0f4b8408 (checked with `git show`).
  - The checker expects 9 (check_finding_register.py:163).
  - ADR-0042 records both (:71).
- **[P4] [contract-vs-mechanics] FINDING_REGISTER_V1.md path-with-tag-word sentence — OPEN.** The sentence has moved to :105-106 and is unamended.
- **[P4] [fail-open] `tag_words` per-word path rule — OPEN.** Unchanged, now :1233-1236 and :1281.
- **[P4] [fail-open/identity-discipline] `description[:80]` — OPEN.** Still at :592 and :639.
- **[P4] [test-coverage] `paths_ok` / severity-mismatch refusal — OPEN, narrowed.** Direct `raising_severity` tests now exist (test_finding_register.py:981-1010). `paths_ok` and the "raising transcript records Pn" refusal still have no assertion (0 grep hits in both suites).
- **[P4] [fail-open/contract-precision] `open_lines_about` asymmetry — OPEN.** The OPEN refusal reads strong (:974), while closure reads weak under an unshared kind (:1214, :1281). A CLOSED+OPEN single-transcript test exists only for the exact-claim branch (:592-605).
- **[P4] [contract-vs-mechanics] `round_scope` / `scoped_before` — OPEN, narrowed.** The `or True` assertion was fixed. `round_scope` still reads 40-hex only (:156), and a scope-less PASS is still not refused (`scoped_before` → False at :166-167).
- **[P4] [identity-discipline/records] ledger-anchored key — OPEN, narrowed.** The c67b072 and 76ae15a pairs still share keys, now with `""` identifiers. Shared vocabulary, inheritance, split-status and `package_open_findings` no longer treat `""` as an identity. Identifier-bearing ledger findings from one round (for example three `accepted.hex` keys) still collide, and no anchor span was added.
- **[P4] [contract-vs-mechanics] untagged `raising_severity` → None — OPEN, narrowed.** Tagged claims are refused (:675-685, applied at :710, :1377 and :1630). The untagged exemption is still not stated in the spec.
- **[P4] [duplicated-authority] `identity_tokens` vs `line_speaks_about` — OPEN.** Both still derive phrases and identifiers separately (:871-882 vs :1202-1222).
- **[P4] [contract-vs-mechanics] `cited_closure_lines` PRIOR fallback — OPEN.** Unchanged (:979-1008).
- **[P4] [contract-vs-mechanics] `is_tracked` / replay exact-claim — OPEN, narrowed.** A non-zero git exit now refuses everything (:1319-1320). An `OSError` still counts every path as tracked (:1317-1318). The replay exact-claim branch (retire :623-626) still applies no `open_lines_about`.
- **[P4] [duplicated-authority] `role`/`reviewer_of`, `strictly_later`/`strictly_later_scope`/`scope_generation` — OPEN.** Unchanged (retire :63, :85; register :99, :546, :1051).
- **[P4] [contract-vs-mechanics] `STATUS_MARK` mixed status — OPEN, narrowed.** `STATUS_OPEN` now also takes `still`/`remains` (:755). My probe shows that `- **[P3] \`alpha_x\` — CLOSED, remains OPEN in part.**` and `… — CLOSED (leg 2 open).**` both still return `is_closure_line` True.
- **[P4] [identity-discipline/records] `is_carry` vs the 400-character cut — OPEN.** Of this section's 271 `p4_open` claims, 147 have descriptions of 395 characters or more.
- **[P4] [identity-discipline/records] `nabu_architecture_review_revision_8: [note]` — OPEN.** Still in `p4_open`.
- **[P4] [contract-precision] REPRO spec :149-151 vs builder :472-480 — OPEN.** Byte-unchanged.
- **[P4] [identity-discipline/contract-revision] REPRO `CONTRACT_REVISION = 11` — OPEN.** Byte-unchanged (check script :35).

**Rebind (d158d26b).** `reproducibility-report.json` has one attestation, at commit `8d063f00…`, with artifact `ecf51188…`, 15 members, `REPRODUCIBLE` and `differing_members []`. The report is `SINGLE_HOST_REPRODUCIBLE`, with `distinct_hosts 1`, `required_hosts 2` and `superseded_attestations []`. There are 0 hits for `69907ddf`. The rebind is clean. The REQ-11 product change (`crates/sley-policy`) touches none of this section's owned paths.

**Other round-9 mechanics checked:**
- The GA row now states both counts, with the gate still reading the claim count (build_ga_acceptance_report.py:323-330, :602).
- `package_open_findings` for this section: p2 4, p3 12, p4 162.
- The replay rule requires the named sha (register :1657-1674).

**Probe: scenario C (fixture).**
- The root `nabu@<first>: [record-precision] scripts/y.py:1 - the guard reads the wrong span` is retired by the line `- **[P4] [record-precision] scripts/y.py:1 guard span — CLOSED.**` at `<third>`.
- The carry is `nabu@<second>: [record-precision] (carried from <first>, OPEN) scripts/y.py:90 - an unrelated bound is missing`.
- Both key to `('nabu','record-precision','scripts/y.py','')`. `fold_restatements` folds the carry (1), which leaves `p4_open` under a line that says nothing about y.py:90.
- That line reads weak True and strong False against the carry. `closing_lines_name` calls `speaking_lines`, which reads strong only when `shares_its_kind`. With the root removed from the view, the kind is unshared.
- In the ledger, 17 of the 72 re-statements are identifier-less, and none of them restates a retired root. The gap is latent.

## Findings

[P3] [fail-open/contract-vs-mechanics] scripts/retire_review_claims.py:397-425 (`closing_lines_name` via `speaking_lines` :228-243, strong only when `shares_its_kind`), :434-449 (pass-1 fold accepts an equal `""` key when it is unique at the named round); scripts/build_finding_register.py:936-940; docs/spec/FINDING_REGISTER_V1.md:157,184 ("under the retirement read (strong identity …)") - The spec says a fold into a retired root needs the root's closing lines to name the re-statement by a strong identity. The code reads weak whenever the kind is unshared once the root is excluded. That is exactly the case for an identifier-less named carry, which pass 1 still treats as the same finding by an absent identifier. Fixture C: a carry about `scripts/y.py:90` folds into a retired root closed by a `scripts/y.py:1` line (weak True, strong False) and leaves the open list. There are no ledger instances today - Closure: read the retired-root guard (both passes) with `strong=True`, or refuse a pass-1 fold of a `""` key into a retired root; add fixture C as a test; regenerate and show no ledger change.

[P4] [identity-discipline] scripts/build_finding_register.py:1546 (`unquoted` strips backtick and double quotes only), :1570-1579 (`named_carry_sha` prefers any unquoted `carried from` over the claim's own leading `(prior|residual from <sha>)` clause), :1520 (ledger origin reads `CARRIED_FROM` only) - Three carry-sha readers disagree. A single-quoted `'carried from <sha>'` counts as the claim's own carry. A leading `(prior from S, …)` plus later prose `carried from T` names T. A ledger-anchored `(prior from S)` carry keys on its own round rather than S. All three shapes mostly fail closed (they stay open) - Closure: one carry-sha reader used by `is_carry`, `named_carry_sha` and `finding_key`; leading clause first; single and curly-single quotes treated as quoted; a test per shape.

[P4] [contract-vs-mechanics] docs/spec/FINDING_REGISTER_V1.md:105-106 ("or a path with a tag word standing there; the OPEN-line refusal reads the head without the path rule") vs scripts/build_finding_register.py:1281 - Carried unchanged - Closure: amend the sentence and state where the exclusion applies.

[P4] [fail-open] scripts/build_finding_register.py:1233-1236 (`tag_words` per 5+-letter word), :1281 (unshared-kind path rule) - Carried unchanged - Closure: match the whole kind phrase in the path rule; a test with two kinds sharing a word.

[P4] [fail-open/identity-discipline] scripts/build_finding_register.py:592, :639 (`description[:80]` prefix fallback in `raising_scope` / `_finding_line_match`) - Carried unchanged - Closure: match through the anchor's line span, refuse a severity mismatch, test two findings sharing an 80-character prefix.

[P4] [test-coverage] scripts/build_finding_register.py:1174,1281 (`paths_ok`), :711-715,:1620-1624 (raising-severity mismatch refusal); bench/review/tests/ - Carried, narrowed: `raising_severity` now has direct tests (test_finding_register.py:981-1010); `paths_ok` and the mismatch refusal have no assertion - Closure: direct tests.

[P4] [fail-open/contract-precision] scripts/build_finding_register.py:943-976 (`open_lines_about`, strong) vs :1214,:1281 (weak closure under an unshared kind) - Carried unchanged; the CLOSED+OPEN single-transcript test covers only the exact-claim branch - Closure: one symmetric read, plus an automatic-path test.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:152-157 (`round_scope`, 40-hex only), :166-167 (scope-less `scoped_before` → no refusal) - Carried, narrowed (the `or True` assertion is fixed) - Closure: read 7-40-hex scopes and refuse a scope-less PASS fold.

[P4] [identity-discipline/records] scripts/build_finding_register.py:1516-1527 (ledger-anchored key: lane, kind, origin round, first identifier) - Carried, narrowed: `""` keys are no longer an identity downstream, but identifier-bearing ledger findings of one round still share a key (for example `accepted.hex`) and count once in `package_open_findings` - Closure: add the anchor's line span to the ledger key; a test with two ledger findings from one round.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:657-659 (`raising_severity` → None untagged); docs/spec/FINDING_REGISTER_V1.md - Carried, narrowed: tagged claims are now refused, but the untagged exemption is still unstated - Closure: state the exemption or derive the severity from the matched raising line.

[P4] [duplicated-authority] scripts/build_finding_register.py:871-882 (`identity_tokens`) vs :1202-1222 (`line_speaks_about`) - Carried unchanged - Closure: make `line_speaks_about` consume `identity_tokens`; a test that both agree.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:979-1008 (`cited_closure_lines`, PRIOR fallback) - Carried unchanged - Closure: restrict the fallback to closure lines whose head names no severity; a test.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:1314-1318 (`OSError` → tracked); scripts/retire_review_claims.py:623-626 (replay exact-claim branch without `open_lines_about`) - Carried, narrowed (a non-zero git exit now refuses) - Closure: fail closed on `OSError`; apply `open_lines_about` in replay; tests.

[P4] [duplicated-authority] scripts/retire_review_claims.py:63 (`role`), :85 (`strictly_later`) vs scripts/build_finding_register.py:99 (`reviewer_of`), :546 (`scope_generation`), :1051 (`strictly_later_scope`) - Carried unchanged - Closure: one resolver and one ancestry predicate imported by both scripts.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:750-755 (`STATUS_MARK`/`STATUS_OPEN`), :802-812 (`is_closure_line`) - Carried, narrowed: `— CLOSED, remains OPEN in part.` and `— CLOSED (leg 2 open).` heads are still closure lines - Closure: treat any unquoted open qualifier in the item's own status span as mixed; tests.

[P4] [identity-discipline/records] scripts/build_finding_register.py:1549-1559 (`is_carry`) vs the records' 400-character description cut; machineresearch/sley-2.0/machine-summary.json (this section's `p4_open`: 147 of 271 descriptions ≥395 characters) - Carried unchanged - Closure: record the whole description or the carry marker as a field; a test with a marker beyond 400.

[P4] [identity-discipline/records] machineresearch/sley-2.0/machine-summary.json (this section's `p4_open`, `nabu_architecture_review_revision_8: [note]`); evidence/review/verdicts/reproducibility_and_independent_conformance/nabu_architecture_review-92fa664.md:25 - Carried unchanged - Closure: one exact-claim entry.

[P4] [contract-precision] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:149-151 vs scripts/build_reproducibility_report.py:472-480 - Carried unchanged (byte-unchanged 8966da2e..HEAD) - Closure: qualify the sentence or reorder the check, plus a test.

[P4] [identity-discipline/contract-revision] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:3; scripts/check_reproducibility_and_independent_conformance.py:35 - Carried unchanged: `CONTRACT_REVISION = 11` was amended in place - Closure: revision 12 or an explicit amendment record.

## Assessment

My section's reproducibility mechanics are byte-unchanged, and the rebind to 8d063f00 is clean (single host, reproducible, no stale attestation). With a working TMPDIR, all three checkers and both suites pass (170 + 50 tests). The inode-exhausted `/tmp` produces spurious failures, which I set aside after re-running.

The 8966da2e-round repair closes my P2:
- A named re-statement keys on its own identifier, and `same_finding` is equality.
- The fold binds only to the unique same-key claim at the named round, and refuses ambiguity.
- Fixtures A and B are pinned as tests, and the nine ledger mis-bindings now restate their own originals.

It also closes my P3: quoted and sha-less carries no longer make a re-statement, and Ariadne's `[128]` finding is unfolded. Finding Register revisions 8 and 9 are recorded, which closes the in-place-revision P4.

One narrow gap remains. The retired-root guard is documented as a strong read but reads weak under an unshared kind, so an identifier-less named carry can still leave the open list under a closing line that does not name it. The fixture reproduces this; the ledger has no instance today. I raise it as a P3. I also add one P4 (the three carry-sha readers disagree) and carry 16 prior P4s, 7 of them narrowed. With no P0-P2 open, the verdict is PASS. No gate moves, and I make no GA or release-readiness claim.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_18_P4_PRIOR_P2_P3_CLOSED
SECTION: reproducibility_and_independent_conformance
FIELD: nabu_architecture_review
SCOPE_SHA: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e
FINDINGS: [P3] [fail-open/contract-vs-mechanics] scripts/retire_review_claims.py:397-425,434-449; scripts/build_finding_register.py:936-940; FINDING_REGISTER_V1.md:157,184 - the retired-root guard reads weak under an unshared kind, so an identifier-less named carry folds into a retired root whose closing line does not name it (fixture C; 0 ledger instances) - Closure: strong read or refuse `""` pass-1 folds into retired roots, fixture test, regenerate. | [P4] [identity-discipline] build_finding_register.py:1546,1570-1579,1520 - the three carry-sha readers disagree (single quotes, a leading prior/residual clause vs a later phrase, the ledger origin) - Closure: one reader, a test per shape. | [P4] [contract-vs-mechanics] FINDING_REGISTER_V1.md:105-106 - carried unchanged. | [P4] [fail-open] build_finding_register.py:1233-1236,1281 - carried unchanged. | [P4] [fail-open/identity-discipline] build_finding_register.py:592,639 - carried unchanged. | [P4] [test-coverage] build_finding_register.py:1174,1281,711-715,1620-1624 - carried, narrowed. | [P4] [fail-open/contract-precision] build_finding_register.py:943-976 vs :1214,1281 - carried unchanged. | [P4] [contract-vs-mechanics] build_finding_register.py:152-157,166-167 - carried, narrowed. | [P4] [identity-discipline/records] build_finding_register.py:1516-1527 - carried, narrowed. | [P4] [contract-vs-mechanics] build_finding_register.py:657-659 - carried, narrowed. | [P4] [duplicated-authority] build_finding_register.py:871-882 vs :1202-1222 - carried unchanged. | [P4] [contract-vs-mechanics] build_finding_register.py:979-1008 - carried unchanged. | [P4] [contract-vs-mechanics] build_finding_register.py:1314-1318; retire_review_claims.py:623-626 - carried, narrowed. | [P4] [duplicated-authority] retire_review_claims.py:63,85 vs build_finding_register.py:99,546,1051 - carried unchanged. | [P4] [contract-vs-mechanics] build_finding_register.py:750-755,802-812 - carried, narrowed. | [P4] [identity-discipline/records] build_finding_register.py:1549-1559; this section's p4_open (147 of 271 at or over 395 characters) - carried unchanged. | [P4] [identity-discipline/records] this section's p4_open revision_8 [note]; nabu_architecture_review-92fa664.md:25 - carried unchanged. | [P4] [contract-precision] REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:149-151 vs build_reproducibility_report.py:472-480 - carried unchanged. | [P4] [identity-discipline/contract-revision] REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:3; check script :35 - carried unchanged.
SUMMARY: All three checkers and both suites pass (170 + 50 tests) with TMPDIR off the inode-exhausted /tmp, and the rebind to 8d063f00 is clean and single-host reproducible. My 8966da2e P2 (identifier wildcard in fold identity) and P3 (quoted and sha-less carries) are verified closed: re-statements key on their own identifier, folds bind only to the unique claim at the named round, fixtures A/B are tested, the nine ledger mis-bindings and Ariadne's [128] are corrected, and revisions 8/9 are recorded. I raise one new P3 (the retired-root guard still reads weak for identifier-less named carries; fixture C, no ledger instance) and one new P4 (carry-sha readers disagree), and I carry 16 prior P4s, 7 of them narrowed. No GA or release-readiness claim is made.
