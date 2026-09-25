<!-- engine: claude-code; observed model: claude-opus-5-5; scope: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e; role: vulcan; field: vulcan_surface_review; dispatched: 2026-09-23T05:43:24Z; duration_s: 740; process_exit_code: 0 -->
# Vulcan Council review — reproducibility_and_independent_conformance

Harness: claude-code
Reviewed checkpoint: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `d158d26bfdda9fc7dc899a1d1a8a141c2da6610e`, so the scope matches. `git log --format='%h %s' 8966da2e..HEAD` lists eight commits: acbc65f0, 883361e3, f7f9af90, 5fd10849, 0f4b8408, 7c51710b (merge), 8d063f00 and d158d26b (records-only mint). The worktree has two untracked Ariadne d158d26 transcripts, which are not mine.
- **Diffs read.**
  - `git diff --stat 8966da2e..HEAD` shows 53 files.
  - I read the full diffs of `scripts/retire_review_claims.py` and `scripts/build_finding_register.py`.
  - None of the reproducibility-owned files appears in the stat, so they are byte-unchanged over the delta: `build_reproducibility_report.py`, `check_reproducibility_and_independent_conformance.py`, `sync_evidence_counters.py`, `REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md`, ADR-0040, `check_exec_package_envelope_v2.py`, `revision-7-retirement-changes.json` and `bench/release/tests/test_reproducibility.py`.
- **Checkers.**
  - `python3 scripts/check_reproducibility_and_independent_conformance.py`: exit 0, `"result": "PASS"`, `"problems": []`.
  - `python3 scripts/check_finding_register.py`, first run: exit 1, `"problems": ["register-tests:fail"]`. At that moment `/tmp` was at 100% inode use (`df -i`: 1048576/1048576).
  - `python3 scripts/check_finding_register.py`, rerun: `"result": "PASS"`, `"problems": []`.
  - `python3 -m unittest discover -s bench/review/tests -t .`: `Ran 170 tests`, `OK`.
  - `python3 scripts/retire_review_claims.py --check`: `"retirable": 0`, `"stale_closures": []`, `"regeneration_divergence": []`, `"split_status": []`, nine report-only `closer_disagreements` (none in this section), `"result": "PASS"`.
  - The rerun, the unittest run and `--check` each reported exit 1. In each case the only extra output was the harness's trailing `/tmp/claude-*-cwd: No space left on device` line. The scripts themselves printed PASS/OK.
- **Environment limit (stated plainly).** After those runs, every Bash call failed before executing (`ENOSPC … /proc/self/fd/28/*.output`), because `/tmp` had no free inodes. The rest of this review is static (Read/Grep). As a result, I did **not** run any of the following:
  - in-memory probes of `fold_restatements`/`open_lines_about`;
  - `bench/release/tests/test_reproducibility.py`;
  - a sha256 check of the batch-14 pins;
  - `git diff 8d063f00..d158d26b`, to confirm that the mint is records-only.
- **Files read.**
  - My prior verdict, `…/vulcan_surface_review-8966da2.md:1-128`.
  - `scripts/retire_review_claims.py:55-63, 228-233, 286-300, 354-497 (fold_restatements), 516-576 (closer_disagreements/split_status_problems), 579-631, 640`.
  - `scripts/build_finding_register.py`:
    - `:160-171`, `:400-409`, `:540-553`, `:571-690`, `:755`, `:791`;
    - `:802-812`, `:943-976` (`open_lines_about`), `:1013-1045`, `:1051-1056`;
    - `:1069-1070`, `:1082-1133`, `:1174-1273` (`line_speaks_about`), `:1305-1321`, `:1444-1447`;
    - `:1457-1582`, `:1622-1690`, `:1707-1733`.
  - `docs/spec/FINDING_REGISTER_V1.md:50-211`.
  - `bench/review/tests/test_retire_review_claims.py:419-478, 640-788` and `bench/review/tests/test_finding_register.py:365-399, 600-618, 981-1010` (by grep), `:1030`.
  - `evidence/review/claim-retirements.json:2020-2169`.
  - `evidence/review/rounds/resumption-8966da2-batch14.json`.
  - `evidence/release/reproducibility-report.json:1-48`.
  - From `machineresearch/sley-2.0/machine-summary.json`: this section's `p2_open`/`p3_open` (`:5760-5785`), the packaging `p4_restated_claims` `[predicate-precision]` items (`:5262-5464`) and the standards `p3`/`p4_restated_claims` (`:7090-7127`).
  - `evidence/review/finding-register.json:43, 8614-8620, 8759-8762`.
  - `evidence/release/ga-acceptance-report.json:247`.
  - Closure lines `ariadne_contract_review-1a9f0aa.md:18-24`, `ariadne_contract_review-6589c6e.md:19`, `vulcan_surface_review-7622776.md:20-28` and `vulcan_surface_review-c67b072.md:20-25`.
  - Grep of `evidence-integrity` status lines across this lane's transcripts.

## Evidence checked

**Status of my 8966da2e findings (one P3, seventeen P4):**

- **[P3] fail-open/identity-discipline `finding_key`/`same_finding`/`fold_restatements` named-restatement wildcard — CLOSED.**
  - Mechanism:
    - `same_finding` is now strict tuple equality (`build_finding_register.py:1530-1536`).
    - A re-statement keys on its own identifier through `_own_identifier` (`:1457-1470`, `:1505`, `:1519`).
    - `CARRIED_FROM` is read only from `unquoted(body)` (`:1540-1546`, `:1512`, `:1561`).
    - A sha-less leading clause is no carry (`LEADING_CARRY` now requires `from <sha>`).
    - `fold_restatements` resolves the root by `named_carry_sha` at exactly that scope and refuses `len(matching) != 1` as ambiguous (`retire_review_claims.py:438-444`).
    - A retired root needs `closing_lines_name`: the strong read via `speaking_lines`, including `open_lines_about`, and a closing transcript that strictly postdates the entry (`:397-427`).
    - The builder replays the named-sha rule (`build_finding_register.py:1651-1689`).
  - Tests: my requested shapes are pinned. The closed-sibling case is `test_a_named_distinct_restatement_never_folds_into_a_closed_sibling` (`:674-687`) and the quoted-carry case is `test_a_quoted_carried_from_is_not_the_claims_carry` (`:652-672`). Ambiguity, named-round and postdating are also covered (`:754-788`, `:362`).
  - Live data:
    - The five Ariadne `[predicate-precision]` re-statements now restate their true roots `@79fdcc6` `scoped_before`/`is_lane_field_name` and `@b58ac1e` `claim_finding_ids` (`machine-summary.json:5323-5464`).
    - No `restates` points at `is_closure_line` any more (Grep count 0).
    - The standards `raising_severity` chain ends at `@79fdcc6` (`:7100-7101`), not at the 6589c6e-closed root.
- **[P4] record-accuracy `p4_open_count` is a claim count — CLOSED.** The spec states it (`FINDING_REGISTER_V1.md:193-199`). The register emits `package_open_findings` (this section: P4 271 claims / 162 distinct, P3 15/12, P2 5/4; `finding-register.json:8614-8620, 8759-8762`). The GA row states both counts ("24 per-package open claims, 23 distinct findings", `ga-acceptance-report.json:247`). It is tested in `test_open_findings_count_distinct_identities` (`:1030`).
- **[P4] fail-open `regeneration_divergence` compares the regeneration with the ledger it produced — OPEN.** It still regenerates from the tracked ledger (`retire_review_claims.py:579-600`); only the key widened to the full `verified_by`. This round shows the gap again: this section's `p4_restated_claims` (4 at 8966da2e) is now absent, and the packaging and standards chains were re-linked. `resumption-8966da2-batch14.json` is an output index with no change record, and `--check` reports `[]`.
- **[P4] identity-precision `raising_scope` 80-char prefix / `raising_severity` — CLOSED.** The full carry-stripped description is matched first, and the 80-char prefix is used only as a fallback for truncated rounds (`:584-600`, `_finding_line_match` `:655-672`). A tagged claim with no raising line is now refused in all three lists (`tagged_claim_unraised` `:675-686`, applied in open, closed and restated). `raising_severity` is tested (`test_finding_register.py:981-1010`).
- **[P4] closure-grammar `_own_status` cell parenthetical — OPEN, narrowed.** Lowercase `(leg 2 open)` is now refused (`:791`). `(in part)` and `(partial)` still pass as closure cells, and there is no `(P[0-4])` allow-list.
- **[P4] contract-vs-mechanics `ledger_words`/`is_lane_field_name` — OPEN, narrowed.** The `package_*` fields and the lane-bearing verdict fields are now excluded (`:1090-1094`, `verdict_field_names` `:1128-1146`). Still read as identifiers:
  - `current_delta_review` (present at `machine-summary.json:932, 1057, …`);
  - lane-less `*_closure_note` fields (`epoch1_reanchor_review_closure_note`, `legacy_adapter_contract_review_closure_note`, `independent_security_review_closure_note`);
  - the new register field `package_open_findings`.
- **[P4] test-integrity `test_finding_register.py:392` `or True` — CLOSED.** Line 392 is now a real `assertNotEqual`, and `raising_severity` has tests (`:988-1010`).
- **[P4] contract-precision `scoped_before(None, x)` / `strictly_later_scope(x, x)` — CLOSED.** `strictly_later_scope` returns False on equal or empty scopes (`:1051-1056`). The spec states the equal-scope rule (`:81-83`). A scope-less PASS falls back to the documented `dated_before`/`supersedes` order (`:404-406`).
- **[P4] record-note `revision-7-retirement-changes.json` keying/counts — OPEN.** The file is unchanged.
- **[P4] record-note `machine-summary.json` mis-listed closures and templated reasons — OPEN, narrowed.**
  - The new bindings in this section carry head-specific reasons, and each cited line records CLOSED on its own item (I read them): `ariadne-1a9f0aa.md` :18/:20/:22/:24, `vulcan-7622776.md` :26, `ariadne-76ae15a.md` :18 and `ariadne-6589c6e.md` :19.
  - Still listed open: `vulcan_surface_review@db53894: [evidence-integrity]` (`p3_open`, `machine-summary.json:5769`), although `vulcan_surface_review-6589c6e.md:26` records that finding CLOSED.
- **[P4] fail-closed-gap `build_reproducibility_report.py:472-480` — OPEN.** Byte-unchanged.
- **[P4] contract-vs-checker `check_…:189-196` / section 2 — OPEN.** Byte-unchanged.
- **[P4] contract-discipline REPRODUCIBILITY spec revision 11 / `attestation_chain` — OPEN.** Byte-unchanged.
- **[P4] note `check_exec_package_envelope_v2.py:26-74` — OPEN (optional).** Byte-unchanged.
- **[P4] fail-open/derivation-degradation `sync_evidence_counters.py` `report_history` — OPEN.** Byte-unchanged.
- **[P4] fail-open replay exact-claim branch without `open_lines_about`; qualified CLOSED heads — OPEN.** `replay_problems` (`retire_review_claims.py:623-626`) still checks exact-claim entries without `open_lines_about`. `STATUS_CLOSED` still admits `CLOSED as to …`.
- **[P4] fail-open `claim_relation_problem` scope-less / `is_tracked` / `NOTE_SCOPE` — OPEN, narrowed.**
  - A failed `git ls-files` now refuses every path (`:1305-1321`).
  - An `OSError` (no git) still returns True for every path, uncached.
  - The `if scope:` skip is unchanged (`:1036`).
  - `NOTE_SCOPE` is still 40-hex only (`:540`).
- **[P4] contract-discipline FINDING_REGISTER revision — CLOSED.** Revision 9 appears in spec `:3`, builder `CONTRACT_REVISION = 9` (`:35`), checker `:163` and register `:43`.

**New in the delta — the OPEN-line refusal was narrowed below the contract.** This trace is derived from the code and was not executed, because the shell was unavailable.
- `open_lines_about` now replaces the OPEN item's head with `" ".join(CATEGORY.findall(head) + PATH_TOKEN.findall(head))` (`build_finding_register.py:970`).
- That keeps only bracketed tags and file tokens. A backticked identifier, a quoted phrase or a bare finding id standing in the head is dropped before `line_speaks_about(…, strong=True, paths_ok=False)` runs.
- Consider the common transcript shape `- **[P4] [record] \`alpha_guard\` second leg — STILL OPEN.**`. It reduces to `P4 record`, so it no longer holds `…: [record] scripts/a.py:1 - \`alpha_guard\` stale` open. Before this delta the full head contained `alpha_guard` and the claim was refused.
- The contract still says the refusal fires on any strong identity in the OPEN item's head, identifier and quoted phrase included (`FINDING_REGISTER_V1.md:59-67, :104-106`).
- The only tests put the identifier inside brackets (`[\`alpha_guard\`]`, `test_finding_register.py:610-618`), which hides the gap.
- Every closure path uses this function:
  - automatic retirement via `speaking_lines` (`retire_review_claims.py:233`);
  - exact-claim retirement (`:292`);
  - the builder's exact-claim refusal (`build_finding_register.py:1444`);
  - exact-key inheritance via `closing_lines_name`.
- Because `--check` regenerates under the same rule and compares only against itself, a closure newly admitted by this narrowing would enter the ledger silently.
- I could not run a live scan, so no live mis-closure is demonstrated.

**Other delta checks.**
- `split_status_problems` (`:543-576`) refuses identifier-bearing open/closed splits that the closer postdates. Pinned in `:419-448`.
- `package_open_claims` refuses a list with no `_count` (`:698-705`).
- `strictly_later_scope` is guarded.
- There is a stale contract text: spec `:153-155` (revision-8 paragraph) says a fold picks the root "nearest by git ancestry when several match", and `--fold-restatements` help (`retire_review_claims.py:640`) says "into its earliest claim". Both contradict the code's exact-scope, ambiguity-refusing rule (`:438-444`) and the revision-9 text (`:189-190`).
- Reproducibility record at the mint: `reproducibility-report.json` binds commit `8d063f00…`, artifact `ecf51188d52b…c1f7`, `SINGLE_HOST_REPRODUCIBLE`, 1 of 2 required hosts, `ga_claimed: false`. The section checker verified it with zero problems. I did not rebuild the artifact.

## Findings

[P3] [fail-open/refusal-regression] scripts/build_finding_register.py:965-975 (`open_lines_about` reduces the OPEN head to `CATEGORY`+`PATH_TOKEN` at :970) vs docs/spec/FINDING_REGISTER_V1.md:59-67,104-106; consumers scripts/retire_review_claims.py:233,292,397-427 and scripts/build_finding_register.py:1444; bench/review/tests/test_finding_register.py:608-618 - The OPEN-line refusal no longer sees a backticked identifier, quoted phrase or bare finding id in the OPEN item's head. So a transcript line `- **[P4] [record] \`alpha_guard\` … — STILL OPEN.**` no longer stops that transcript from closing the `alpha_guard` claim on another line. This applies to automatic, exact-claim, builder and exact-key-inheritance closures alike, contrary to the contract's strong-identity rule. The trace is derived, not executed (the shell was unavailable), and no live mis-closure is shown - Closure: bound the head by stripping only parenthetical carry/quoted foreign clauses while keeping the identifier, quoted-phrase and finding-id tokens (or amend the contract and state the narrowing); add a test with an unbracketed identifier OPEN line that refuses the closure; report the closures the old and new rules admit differently at HEAD.
[P4] [fail-open] scripts/retire_review_claims.py:579-600 (`regeneration_divergence` still compares the regeneration with the tracked ledger it produced); evidence/review/rounds/resumption-8966da2-batch14.json (index only) - Carried. This round dropped this section's `p4_restated_claims` and re-linked the packaging and standards chains with no change record, and `--check` reports `[]` - Closure: diff against `git show <previous tracked>:machineresearch/sley-2.0/machine-summary.json`, list each dropped, re-bound or re-classified item with a reason, and test it with a mutated previous version.
[P4] [closure-grammar] scripts/build_finding_register.py:791 vs docs/spec/FINDING_REGISTER_V1.md:106-108 - Carried, narrowed: lowercase `open` is now refused, but `(in part)` and `(partial)` still count as closure cells - Closure: allow-list `(P[0-4])` or refuse partial qualifiers case-insensitively, and pin the shapes.
[P4] [contract-vs-mechanics] scripts/build_finding_register.py:1085-1146 (`ledger_words`, `verdict_field_names` requires a lane name) vs docs/spec/FINDING_REGISTER_V1.md:61-63 - Carried, narrowed: `current_delta_review`, lane-less `*_closure_note` fields (`epoch1_reanchor_review_closure_note`, `independent_security_review_closure_note`, …) and the new `package_open_findings` still read as identifiers - Closure: add every `*_review*`/`*_note` summary field and every register field to the vocabulary, with a test.
[P4] [record-note] evidence/review/rounds/revision-7-retirement-changes.json `keying` (omits `severities`), `counts` 45/127 list lengths vs 43/125 set sizes - Carried; the file is unchanged - Closure: add `severities` to the keying or label the counts as list lengths.
[P4] [record-note] machineresearch/sley-2.0/machine-summary.json:5769 (`reproducibility_and_independent_conformance.p3_open`: `vulcan_surface_review@db53894: [evidence-integrity]`) vs evidence/review/verdicts/reproducibility_and_independent_conformance/vulcan_surface_review-6589c6e.md:26 (CLOSED) - Carried, narrowed: the new exact-claim bindings carry head-specific reasons, but this claim, recorded CLOSED, is still listed open - Closure: bind it exact-claim to 6589c6e#L26 (or state why the line cannot bind) and rebind the counts.
[P4] [fail-closed-gap] scripts/build_reproducibility_report.py:472-480 vs docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:149-151 - Carried, byte-identical: the re-attested-host skip runs before the current-commit refusal - Closure: refuse before the skip (or state the exemption) and add the `primary@current` test case.
[P4] [contract-vs-checker] scripts/check_reproducibility_and_independent_conformance.py:189-196; docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:130-156 - Carried, byte-identical - Closure: add a `superseded-host-currently-attested:<label>` rule with a test, and state it in section 2.
[P4] [contract-discipline] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:3,146-151,562-565; scripts/check_reproducibility_and_independent_conformance.py:35,109-121,454-456; docs/adr/ADR-0040-reproducibility-and-independent-conformance-boundary.md:75 - Carried, byte-identical: `attestation_chain:not-derived` is enforced under revision 11 while the spec is silent on it - Closure: bump to 12 and state the chain-derivation rule.
[P4] [note] scripts/check_exec_package_envelope_v2.py:26-74 - Carried optional note, byte-identical - Closure: optional.
[P4] [fail-open/derivation-degradation] scripts/sync_evidence_counters.py:59,62-68,78-82,124-129 - Carried, byte-identical: `report_history` returns only the working-tree report when `git log` fails - Closure: fail (or skip the chain update) on git failure, and add a real-git-history test.
[P4] [fail-open] scripts/retire_review_claims.py:623-626 (replay exact-claim branch: no `open_lines_about`); scripts/build_finding_register.py:754 (`STATUS_CLOSED` admits qualified `CLOSED as to/for/in substance` heads) - Carried, unchanged - Closure: apply `open_lines_about` in the replay branch; refuse a qualified CLOSED head (or require a reason for it); add tests.
[P4] [fail-open] scripts/build_finding_register.py:1036 (no round check when the scope is None), :1305-1321 (`is_tracked`: an `OSError` still treats every path as tracked), :540 (`NOTE_SCOPE` accepts 40-hex only, silently) - Carried, narrowed: a failed `git ls-files` now refuses everything - Closure: refuse without a floor, fail closed when git is absent, refuse a short or `at <sha>` note, with tests.
[P4] [contract-precision] docs/spec/FINDING_REGISTER_V1.md:153-155 ("nearest by git ancestry when several match"); scripts/retire_review_claims.py:640 (`--fold-restatements` help: "into its earliest claim") vs scripts/retire_review_claims.py:438-444 and docs/spec/FINDING_REGISTER_V1.md:189-190 - New: the revision-8 paragraph and the CLI help describe root selection the code no longer performs (exact named scope; two matches are ambiguous and the carry stays open) - Closure: correct or mark as superseded the revision-8 sentence and the help text.

## Assessment

The 0f4b8408/acbc65f0 repair closes my 8966da2e P3:
- A named re-statement keys on its own identifier.
- `same_finding` is equality.
- The fold root is the unique claim at the named sha, and ambiguity stays open.
- A quoted `carried from` no longer counts.
- A retired root needs the strong read, the OPEN-line refusal and a postdating closer.
- The builder replays the named-sha rule, and tests pin the probe shapes I asked for.

The live mislinks are re-linked to their true roots, and nothing restates `is_closure_line` any more. Five of my seventeen P4s are also closed:
- claim-count labelling, with `package_open_findings` and the GA row stating both counts;
- raising-line identity;
- the `or True` test;
- the equal-scope predicate;
- the register revision bump.

The section checker, the register checker (on rerun), the 170 review tests and `--check` all report PASS/OK. Every non-PASS exit traces to the host's `/tmp` inode exhaustion, and that same exhaustion kept me from running probes. The reproducibility mechanics are byte-identical over the delta. The mint records `SINGLE_HOST_REPRODUCIBLE` (1 of 2 hosts) for `8d063f00`/`ecf51188…`.

The same batch that closed the identity gap narrowed the OPEN-line refusal below its contract. Stripping the OPEN head down to tags and paths means an unbracketed identifier or quoted phrase no longer holds a claim open, and the only test uses a bracketed identifier. That is a reachable fail-open in a refusal that guards every closure path; I derived it statically and show no live effect, so I rate it P3. Eleven P4s are carried, four of them narrowed, plus one new P4 for stale contract text. This is REVISE with no P0, P1 or P2. I make no GA or release-readiness claim.

VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3_13_P4_PRIOR_P3_CLOSED
SECTION: reproducibility_and_independent_conformance
FIELD: vulcan_surface_review
SCOPE_SHA: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e
FINDINGS: [P3] [fail-open/refusal-regression] build_finding_register.py:965-975 (:970 head reduced to CATEGORY+PATH_TOKEN) vs FINDING_REGISTER_V1.md:59-67,104-106; consumers retire_review_claims.py:233,292,397-427, build_finding_register.py:1444; test_finding_register.py:608-618 - an unbracketed identifier/quoted phrase/finding id in an OPEN head no longer refuses a closure on another line (derived, not executed) - closure: keep strong-identity tokens in the bounded head or amend the contract, add an unbracketed-identifier test, report the admitted-set difference at HEAD. [P4] [fail-open] retire_review_claims.py:579-600; batch14 index only - self-comparing regeneration; this round's re-links and dropped restated list are unrecorded - closure: diff against the previous tracked ledger with reasons, test. [P4] [closure-grammar] build_finding_register.py:791 - `(in part)`/`(partial)` still close - carried narrowed. [P4] [contract-vs-mechanics] build_finding_register.py:1085-1146 - `current_delta_review`, lane-less `*_closure_note`, `package_open_findings` still identifiers - carried narrowed. [P4] [record-note] revision-7-retirement-changes.json keying/counts - carried. [P4] [record-note] machine-summary.json:5769 `@db53894 [evidence-integrity]` open vs 6589c6e.md:26 CLOSED - carried narrowed. [P4] [fail-closed-gap] build_reproducibility_report.py:472-480 - carried. [P4] [contract-vs-checker] check_reproducibility…py:189-196; spec:130-156 - carried. [P4] [contract-discipline] REPRODUCIBILITY spec:3,146-151,562-565; checker:35,109-121,454-456; ADR-0040:75 - carried. [P4] [note] check_exec_package_envelope_v2.py:26-74 - carried optional. [P4] [fail-open/derivation-degradation] sync_evidence_counters.py:59,62-68,78-82,124-129 - carried. [P4] [fail-open] retire_review_claims.py:623-626; build_finding_register.py:754 - carried. [P4] [fail-open] build_finding_register.py:1036,1305-1321,540 - carried narrowed. [P4] [contract-precision] FINDING_REGISTER_V1.md:153-155; retire_review_claims.py:640 vs :438-444 - revision-8 "nearest by git ancestry" and "earliest claim" help contradict the exact-scope/ambiguity rule - new.
SUMMARY: At d158d26b the fold/identity repair closes my 8966da2e P3: identity is strict equality on the claim's own identifier, the root is the unique claim at the named sha, a quoted carry is ignored, retired roots need the strong read and a postdating closer, and the live mislinks are re-linked. Five of my P4s also close. The same delta cut the OPEN-line refusal's head down to tags and paths, so an unbracketed identifier or quoted phrase recorded OPEN no longer blocks a closure, contrary to the contract (one P3, derived statically because host `/tmp` inode exhaustion stopped the shell). Eleven carried P4s and one new P4 remain; checkers and tests report PASS and the reproducibility mechanics are byte-identical; no GA or release-readiness claim.
