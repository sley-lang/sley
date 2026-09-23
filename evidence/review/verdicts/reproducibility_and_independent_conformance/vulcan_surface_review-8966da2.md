<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 8966da2e0a52a6f6e56949e9a5902464c86613c0; role: vulcan; field: vulcan_surface_review; dispatched: 2026-09-23T03:06:06Z; duration_s: 484; process_exit_code: 0 -->
# Vulcan Council review — reproducibility_and_independent_conformance

Harness: claude-code
Reviewed checkpoint: 8966da2e0a52a6f6e56949e9a5902464c86613c0

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `8966da2e0a52a6f6e56949e9a5902464c86613c0`, so the scope matches. `git log --format=%s 8f774d0c..HEAD` lists two commits: 69907ddf (fix) and 8966da2e (records-only mint).
- **Diffs read.** `git diff --stat 8f774d0c..HEAD` shows 33 files. I read the full diffs of `scripts/build_finding_register.py`, `scripts/retire_review_claims.py`, `docs/spec/FINDING_REGISTER_V1.md`, `bench/review/tests/test_retire_review_claims.py` and `evidence/review/claim-retirements.json` (5 new entries). `git diff --quiet 8f774d0c..HEAD` over the reproducibility-owned code, spec, ADR and tests showed them unchanged: `build_reproducibility_report.py`, `check_reproducibility_and_independent_conformance.py`, `sync_evidence_counters.py`, `REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md`, ADR-0040, `check_exec_package_envelope_v2.py`, `revision-7-retirement-changes.json`, `test_finding_register.py`, `check_finding_register.py` and `test_reproducibility.py`.
- **Checkers.** All exited 0:
  - `python3 scripts/check_reproducibility_and_independent_conformance.py`: `"result": "PASS"`, `"problems": []`.
  - `python3 scripts/check_finding_register.py`: `"result": "PASS"`, `"problems": []`.
  - `python3 scripts/retire_review_claims.py --check`: `"retirable": 0`, `"stale_closures": []`, `"regeneration_divergence": []`, `"result": "PASS"`.
- **Tests.** `python3 -m unittest bench.review.tests.test_finding_register bench.review.tests.test_retire_review_claims` ran 58 tests, OK. `python3 -m unittest bench.release.tests.test_reproducibility` ran 50 tests, OK.
- **Files read.**
  - My prior verdict, `…/vulcan_surface_review-8f774d0.md:1-76`. Its sha256 `1069d885…` equals the batch-13 index pin, and the register binds it as `vulcan_surface_review_revision_12` with disposition `REVISE_0_P0_0_P1_1_P2_0_P3_17_P4`.
  - `scripts/retire_review_claims.py:345-446`.
  - `scripts/build_finding_register.py:540-560`, `:799-835`, `:924-932`, `:1027-1135`, `:1151-1156`, `:1296-1362`.
  - `docs/spec/FINDING_REGISTER_V1.md:3`, `:116-132`.
  - `evidence/review/rounds/resumption-8f774d0-batch13.json`.
  - The closure lines `release_candidate_packaging/ariadne_contract_review-1a9f0aa.md:20`, `release_candidate_packaging/nabu_architecture_review-1a9f0aa.md:34,36,39,40` and `reproducibility_and_independent_conformance/ariadne_contract_review-1a9f0aa.md:22`.
- **Probes.**
  - A throwaway temp-dir git fixture with three commits, one closed root and one named distinct re-statement, run through `fold_restatements`.
  - Scans of the live `machine-summary.json`: every `pN_restated_claims` item's own identifier and `carried from` sha against its root's; every fold into a closed root, with closer and re-statement ordered by `git rev-list --count`; open-entry key multiplicity.
  - `shared_vocabulary` and `line_speaks_about` evaluated on live claims and lines.
  - A 8f774d0c-vs-HEAD ledger count comparison via `git show`.

## Evidence checked

**Status of my 8f774d0c findings (one P2, seventeen P4):**

- **[P2] fail-open/identity-discipline "a carried claim folds into any same-lane/kind/file claim" (`finding_key`/`same_finding`/`package_restated_claims`/`fold_restatements`) — OPEN, narrowed and downgraded to the P3 below.**
  - Repaired:
    - A prose carry word no longer makes a claim a re-statement (`is_carry` :1355-1362).
    - An identifier-less original keys `""` and matches only `""` (:1330, :1342, :1352).
    - The packaging `pN_open_count` lineage is back in `release_candidate_packaging.p4_open` (six entries, @1a9f0aa through `_revision_6@8f774d0`).
    - My lane's three `[fail-open]` P4s here are no longer chained.
    - `reopen_all` now returns re-statements (:415).
    - The order is reopen → retire → fold (:433-435, :494-497).
    - The spec states the rule (:121-132).
  - Not repaired:
    - A named re-statement still keys `None` (:1330, :1342), and `same_finding` treats `None` as a wildcard (:1352).
    - The root is the earliest same-key claim by scope (:373-378), not the claim at the named `carried from <sha>`.
    - Ambiguity is not refused.
    - The retired-root gate reads `line_speaks_about(text[n-1], entry)` in its default weak form (:388), where the kind phrase alone relates. It applies no `open_lines_about` and does not require the closer to postdate the re-statement.
    - Neither of the tests I asked for was added (the live shape, and a `_count`-absent fixture).
  - Probe: a closed `alpha_one` root plus `(carried from c2, OPEN) … beta_two` gives `folded 1`, where the closure asked for 0. On the closer line the weak read is True and the strong read is False, and `open_lines_about` is not consulted.
  - Live effect, part 1: seven re-statements raised after their root's closer are folded into that closed, different finding. Five are Ariadne `[predicate-precision]` re-statements of `scoped_before`, `is_lane_field_name` and `claim_finding_ids` at b58ac1e/8f774d0. They fold into `@76ae15a is_closure_line`, closed by `ariadne-1a9f0aa.md#L20`, which names only `is_closure_line` (weak True, strong False). The other two are Vulcan standards P3 re-statements (`raising_severity`; closer 6589c6e).
  - Live effect, part 2: of 91 restated items, 23 carry a different own identifier from their root and 22 name a `carried from` sha other than the root's scope.
  - Live effect, part 3: Ariadne's @8f774d0 `[record-accuracy] … entry [128]` claim is keyed through a **quoted** Nabu claim's `carried from 76227765`, because `CARRIED_FROM.search(body)` scans the whole body (:1322). A fresh original that quotes such a claim becomes a wildcard re-statement (probe: `is_carry True`, key `(…, None)`).
  - No live count loss was found: the true roots stay open. The remaining effect is mislinked records, plus a stuck-closure side effect covered in the P3.
- **[P4] fail-open `regeneration_divergence` compares the regeneration with the ledger it produced — OPEN.** The code is now at :428-446 and still compares against itself. This round shows the gap again: this section's P4 restated count went 86 → 4 and open went 92 → 219; packaging P4 closed went 38 → 54 and restated 88 → 60. `--check` reports `[]`, and batch-13 is an output index with no change record.
- **[P4] identity-precision `raising_scope` 80-char prefix / `raising_severity` — OPEN.** Unchanged (:559-623).
- **[P4] closure-grammar :712 cell parenthetical — OPEN.** Unchanged.
- **[P4] contract-vs-mechanics `ledger_words`/`is_lane_field_name` — OPEN.** Unchanged; the functions moved to :974-1001.
- **[P4] test-integrity `test_finding_register.py:392` `or True` — OPEN.** Byte-unchanged. Zero `raising_severity` references remain in the tests.
- **[P4] contract-precision `scoped_before(None, x)` / `strictly_later_scope(x, x)` — OPEN.** Unchanged (:160-168, :944-955).
- **[P4] record-note `revision-7-retirement-changes.json` keying/counts — OPEN.** The file is unchanged.
- **[P4] record-note `machine-summary.json` p3_open/p4_open mis-listed closures, with templated reasons — OPEN.** Still present: 4 `[evidence-integrity]` and 4 `[record-provenance]` @db53894/@c67b072 entries in p3/p4_open, plus the `:4629` record-note. The five new exact-claim entries do bind the reviewers' own item lines, and each closer (gen 1099) postdates its claim (gen 1086-1095). But the template reason again says "names it by kind and file in its head" for `reproducibility…/ariadne-1a9f0aa.md:22`, and that head names no file.
- **[P4] fail-closed-gap `build_reproducibility_report.py:472-480` — OPEN.** Byte-unchanged.
- **[P4] contract-vs-checker `check_…:189-196`, section 2 — OPEN.** Byte-unchanged.
- **[P4] contract-discipline REPRODUCIBILITY spec revision 11 / `attestation_chain` — OPEN.** Byte-unchanged.
- **[P4] note `check_exec_package_envelope_v2.py:26-74` — OPEN (optional).** Unchanged. It now has nine open ledger entries.
- **[P4] fail-open/derivation-degradation `sync_evidence_counters.py` `report_history` — OPEN.** Byte-unchanged.
- **[P4] fail-open `fold_restatements` closer ordering / speaking line (`retire_review_claims.py:345-383`) — OPEN, merged into the P3 below and not counted separately.** A speaking check now exists (:382-390), but it uses the weak read and does not require the closer to postdate the re-statement; the seven post-closer folds above are live.
- **[P4] fail-open replay exact-claim branch without `open_lines_about`; qualified CLOSED heads — OPEN.** Unchanged (branch now at :469-472; :695-712).
- **[P4] fail-open `claim_relation_problem` scope-less / `is_tracked` / `NOTE_SCOPE` — OPEN.** Unchanged: `if scope:` at :929; `is_tracked` at :1151-1154 says "Without git every path counts as tracked"; :540.
- **[P4] contract-discipline FINDING_REGISTER revision 7 — OPEN.** A sixth batch of rule changes (:121-132) ships under `revision 7` (spec :3, builder :35, checker :163, register :43).

**Repair (2), shared vocabulary.** `restates.get(other) == claim or restates.get(claim) == other` (:833) holds for correctly linked chains. The pinned generic-head shapes return 0 and 1 as stated (tests green). But because of the mislinks, the true root `ariadne_contract_review_revision_8@79fdcc6 … (scoped_before)` now finds `scoped_before` and `strictly_later_scope` in its shared vocabulary. A synthetic closer line `[P4] [predicate-precision] \`scoped_before\` … — CLOSED` then relates to it under the strong read: **False**. This fails closed, but a genuine closure of that finding would be refused.

**Reproducibility mechanics.** Byte-identical over the delta. The section checker has zero problems, and all 50 tests pass.

## Findings

[P3] [fail-open/identity-discipline] scripts/build_finding_register.py:1322,1330,1342 (a named re-statement keys `None`; `CARRIED_FROM.search(body)` also matches a quoted claim's `carried from <sha>`), :1346-1352 (`same_finding`: `None` is a wildcard); scripts/retire_review_claims.py:373-378 (root = earliest same-key claim by scope, not the named sha), :382-390 (retired-root gate uses the weak `line_speaks_about` with no `open_lines_about` and no closer-postdates check); docs/spec/FINDING_REGISTER_V1.md:121-132 - This is the residual of my 8f774d0c P2: the prose-word folds are undone and the packaging lineage is restored, but a named re-statement of a distinct finding still folds into any earlier same-lane/kind/file claim, including a closed one whose closing line only shares the kind phrase. Probe: closed `alpha_one` root plus `(carried from c2, OPEN) … beta_two` → `folded 1`. Live: seven post-closure OPEN re-statements are linked to closed different findings (five Ariadne `scoped_before`/`is_lane_field_name`/`claim_finding_ids` → `@76ae15a is_closure_line` via `ariadne-1a9f0aa.md#L20`, weak True / strong False; two Vulcan standards P3s). 23/91 restated items differ in identifier and 22/91 in named sha; Ariadne's `entry [128]` claim is keyed by a quoted sha. The mislinks also make `scoped_before` "shared" for its true root, so a genuine closer line naming it relates to nothing. No live count loss was found because the true roots stay open; the loss is reachable when the true root is itself a named/quoted carry or absent - Closure: select the fold root by the named `carried from <sha>` (the lane's claim at that scope, or its chain) for file-anchored claims too; keep a named re-statement's own first identifier and require equality when it has one; refuse a re-statement that is `same_finding` with two distinct keys; read `CARRIED_FROM` only from the leading clause or the claim's own head, not quoted text; for a retired root require the strong read with the lane's shared vocabulary, `open_lines_about`, and a closer that postdates the re-statement; re-link the seven live folds; add tests for the live shape (closed sibling + named distinct re-statement → 0 folds) and the quoted-carry shape.
[P4] [record-accuracy] machineresearch/sley-2.0/machine-summary.json `reproducibility_and_independent_conformance.p4_open` / `p4_open_count` (219) - Since the prose-word folds were withdrawn, later statements of one finding in the lanes' `Carried, byte-identical:` style no longer fold, even with equal non-empty keys. 219 open P4 entries collapse to 105 keys; 38 equal non-empty keys cover 120 entries (for example, 5 × `report_history` and 9 envelope-note entries), so the count the GA row sums is a claim count, not a finding count. This over-counts, so it fails safe - Closure: fold strictly later claims whose non-empty keys are equal (identity by equality, not by carry marker), or state and label `pN_open_count` as a claim count in the spec and GA row, with a test.
[P4] [fail-open] scripts/retire_review_claims.py:428-446 (`regeneration_divergence` compares the regeneration with the tracked ledger it produced); evidence/review/rounds/resumption-8f774d0-batch13.json (index only) - This round re-classified 82 restated P4s here and 16 packaging P4 closures with no change record, and `--check` reports `[]` - Closure: under `--regenerate`/`--check`, diff against `git show <previous tracked>:machineresearch/sley-2.0/machine-summary.json`, list every dropped, re-bound or re-classified item with a required reason, and test it with a mutated previous version.
[P4] [identity-precision] scripts/build_finding_register.py:559-599,602-623 (`raising_scope` 80-char prefix; `raising_severity` first-prefix match) - Carried, unchanged - Closure: identify the raising line by kind plus anchor with line numbers, prefer the frozen note, refuse an ambiguous prefix, test.
[P4] [closure-grammar] scripts/build_finding_register.py:712 vs docs/spec/FINDING_REGISTER_V1.md:101-103 - Carried, unchanged: `(in part)`, `(partial)` and lowercase `(leg 2 open)` count as closure cells - Closure: allow-list `(P[0-4])`, exclude case-insensitively, pin the shapes.
[P4] [contract-vs-mechanics] scripts/build_finding_register.py:974-1001 (`ledger_words`, `is_lane_field_name`) vs docs/spec/FINDING_REGISTER_V1.md:58-62 - Carried, unchanged: `current_delta_review` and the `*_closure_note` fields are accepted as identifiers - Closure: add every `*_review*`/`*_note` field to `ledger_words`, test.
[P4] [test-integrity] bench/review/tests/test_finding_register.py:392 (`or True`, comment inverted); bench/review/tests (no `raising_severity` test) - Carried, byte-unchanged - Closure: assert the real value with a correct comment; add a `raising_severity` mismatch test.
[P4] [contract-precision] scripts/build_finding_register.py:160-168 (`scoped_before(None, x)` → False), :944-955 (`strictly_later_scope(x, x)` → True) vs docs/spec/FINDING_REGISTER_V1.md:77-79 - Carried, unchanged - Closure: state the fallback or require a scope, guard the same-commit case, test.
[P4] [record-note] evidence/review/rounds/revision-7-retirement-changes.json `keying` (omits `severities`), `counts` 45/127 list lengths vs 43/125 set sizes - Carried, file unchanged - Closure: add `severities` to the keying or label the counts as list lengths.
[P4] [record-note] machineresearch/sley-2.0/machine-summary.json `reproducibility_and_independent_conformance.p3_open`/`p4_open` (the @db53894/@c67b072 `[evidence-integrity]`/`[record-provenance]`/`:4629` record-note entries recorded CLOSED in the cited transcripts); evidence/review/claim-retirements.json (templated reasons, e.g. the new entry citing `reproducibility…/ariadne_contract_review-1a9f0aa.md:22` says "kind and file" though that head names no file) - Carried - Closure: exact-claim bindings with reasons that describe the cited head, then rebind the counts.
[P4] [fail-closed-gap] scripts/build_reproducibility_report.py:472-480 vs docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:149-151 - Carried, byte-identical: the re-attested-host skip runs before the current-commit refusal - Closure: refuse before the skip (or state the exemption) and add the `primary@current` test case.
[P4] [contract-vs-checker] scripts/check_reproducibility_and_independent_conformance.py:189-196; docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:130-156 - Carried, byte-identical - Closure: a `superseded-host-currently-attested:<label>` rule with a test, stated in section 2.
[P4] [contract-discipline] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:3,146-151,562-565; scripts/check_reproducibility_and_independent_conformance.py:35,109-121,454-456; docs/adr/ADR-0040-reproducibility-and-independent-conformance-boundary.md:75 - Carried, byte-identical: `attestation_chain:not-derived` is enforced under revision 11 while the spec is silent - Closure: bump to 12 and state the chain-derivation rule.
[P4] [note] scripts/check_exec_package_envelope_v2.py:26-74 - Carried optional note, byte-identical - Closure: optional.
[P4] [fail-open/derivation-degradation] scripts/sync_evidence_counters.py:59,62-68,78-82,124-129 - Carried, byte-identical: `report_history` returns only the working-tree report when `git log` fails - Closure: fail (or skip the chain update) on git failure, add a real-git-history test.
[P4] [fail-open] scripts/retire_review_claims.py:469-472 (replay exact-claim branch: no `open_lines_about`); scripts/build_finding_register.py:695-712 (qualified `CLOSED as to/for/in substance` heads count as closures) - Carried, unchanged - Closure: apply `open_lines_about` in the replay branch; refuse a qualified CLOSED head (or require a reason for it); tests.
[P4] [fail-open] scripts/build_finding_register.py:927-929 (no round check when `raising_scope` is None), :1151-1154 (`is_tracked`: every path counts as tracked without git), :540 (`NOTE_SCOPE` accepts 40-hex only, silently) - Carried, unchanged - Closure: refuse without a floor, fail closed on `git ls-files` failure, refuse a short or `at <sha>` note, tests.
[P4] [contract-discipline] docs/spec/FINDING_REGISTER_V1.md:3 (`revision 7`) vs :121-132; scripts/build_finding_register.py:35; scripts/check_finding_register.py:163; evidence/review/finding-register.json:43 - Carried: a sixth batch of rule changes ships under the same revision - Closure: bump to 8 in the spec header, builder, checker and register.

## Assessment

The delta does repair the concrete harm my P2 named:
- Prose carry words no longer create re-statements.
- An identifier-less original no longer matches everything.
- The packaging `pN_open_count` lineage my packaging reviewer recorded OPEN is back in the open list.
- Recorded folds are re-derived on regeneration.
- Retirement now runs before folding.

I checked every section checker, the register checker, `--check` and both test suites: all exit 0 or pass. The five new exact-claim bindings cite reviewers' own item lines that postdate their claims, and the reproducibility mechanics are byte-identical.

The mechanism the P2 targeted is only narrowed:
- A named re-statement is still a wildcard over its lane, kind and file.
- The root is picked by earliest scope, not by the sha the claim names.
- The new retired-root gate is the weak read, so the shared kind phrase alone passes it. It has no OPEN-line check and no ordering check.

Consequences:
- Seven live re-statements, raised after their root's closure and asserting OPEN, are recorded as restating closed, different findings whose closing lines don't name them.
- A quoted `carried from <sha>` turns an original into a wildcard.
- The mislinks make one true root un-closable by its own identifier.
- My probe folds exactly the shape my closure asked to be refused.

I found no live count loss, since the true roots remain open. So I downgrade the finding to P3 and do not keep it at P2.

Withdrawing prose folds also means the open count now counts claims, not findings (219 entries over 105 keys). That over-counts and fails safe, but it is not stated anywhere. The sixteen other carried P4s stand unchanged, and one, regeneration visibility, is shown again by this round's silent re-classification.

This is REVISE with no P0, P1 or P2. I make no GA or release-readiness claim.

VERDICT: REVISE_0_P0_0_P1_0_P2_1_P3_17_P4
SECTION: reproducibility_and_independent_conformance
FIELD: vulcan_surface_review
SCOPE_SHA: 8966da2e0a52a6f6e56949e9a5902464c86613c0
FINDINGS: [P3] [fail-open/identity-discipline] build_finding_register.py:1322,1330,1342,1346-1352; retire_review_claims.py:373-378,382-390; FINDING_REGISTER_V1.md:121-132 - named re-statement still keys None (wildcard), root chosen by earliest scope not the named sha, CARRIED_FROM matches quoted text, retired-root gate is the weak read without open_lines_about or closer ordering; probe folds a distinct finding into a closed sibling (1, expected 0); 7 live post-closure OPEN re-statements linked to closed different findings (ariadne-1a9f0aa.md#L20 weak True/strong False); 23/91 identifier and 22/91 sha mismatches; mislinks make `scoped_before` un-closable for its true root - closure: select by named sha, keep own identifier and require equality, refuse ambiguity, leading-clause CARRIED_FROM only, strong read + OPEN-line + ordering for retired roots, re-link, tests. [P4] [record-accuracy] machine-summary.json repro p4_open/p4_open_count 219 (105 keys; 38 equal non-empty keys cover 120 entries) - closure: fold equal non-empty keys or label the count as a claim count, test. [P4] [fail-open] retire_review_claims.py:428-446; batch13 index only - 82 restated here and 16 packaging closures re-classified silently - closure: diff against the previous tracked ledger with reasons, test. [P4] [identity-precision] build_finding_register.py:559-599,602-623 - carried. [P4] [closure-grammar] build_finding_register.py:712 vs spec:101-103 - carried. [P4] [contract-vs-mechanics] build_finding_register.py:974-1001 vs spec:58-62 - carried. [P4] [test-integrity] test_finding_register.py:392 `or True`; no raising_severity test - carried. [P4] [contract-precision] build_finding_register.py:160-168,944-955 vs spec:77-79 - carried. [P4] [record-note] revision-7-retirement-changes.json keying/counts - carried. [P4] [record-note] machine-summary.json repro p3_open/p4_open mis-listed closures; templated claim-retirements reasons - carried. [P4] [fail-closed-gap] build_reproducibility_report.py:472-480 vs spec:149-151 - carried. [P4] [contract-vs-checker] check_reproducibility…py:189-196; spec:130-156 - carried. [P4] [contract-discipline] REPRODUCIBILITY spec:3,146-151,562-565; checker:35,109-121,454-456; ADR-0040:75 - carried. [P4] [note] check_exec_package_envelope_v2.py:26-74 - carried optional. [P4] [fail-open/derivation-degradation] sync_evidence_counters.py:59,62-68,78-82,124-129 - carried. [P4] [fail-open] retire_review_claims.py:469-472; build_finding_register.py:695-712 - carried. [P4] [fail-open] build_finding_register.py:927-929,1151-1154,540 - carried. [P4] [contract-discipline] FINDING_REGISTER_V1.md:3 vs :121-132; build_finding_register.py:35; check_finding_register.py:163; finding-register.json:43 - carried.
SUMMARY: At 8966da2e all section checkers, the register checker and `--check` pass, both test suites are green, and the reproducibility mechanics are byte-identical. The 69907ddf repair undoes the prose-word folds and restores the packaging `pN_open_count` lineage my P2 named, but a named re-statement is still a wildcard over its lane, kind and file, is folded by earliest scope rather than its named sha, and passes a weak-read retired-root gate. Seven live post-closure OPEN re-statements are therefore linked to closed, different findings, and a probe folds exactly the shape the closure asked to refuse. I found no live count loss, so this is REVISE with the P2 downgraded to one P3, one new P4 on open-count inflation, and sixteen carried P4s; no GA or release-readiness claim.
