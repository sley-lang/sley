<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 8966da2e0a52a6f6e56949e9a5902464c86613c0; role: nabu; field: nabu_architecture_review; dispatched: 2026-09-23T03:06:06Z; duration_s: 442; process_exit_code: 0 -->
# Nabu Council review — reproducibility_and_independent_conformance

Harness: claude-code
Reviewed checkpoint: 8966da2e0a52a6f6e56949e9a5902464c86613c0

What I verified. I made no writes to the tree. `git status --short` before and after shows only the eight untracked `*-8966da2.md` transcripts that were already there at session start. All probes were in-memory `python3 -` imports of the tracked modules, plus one throwaway `tempfile.TemporaryDirectory()` + `git init` fixture outside the tree. I did not run cargo because the owned delta contains no Rust.

- `git rev-parse HEAD` → `8966da2e0a52a6f6e56949e9a5902464c86613c0`, so the scope matches. `git log --format=%s 8f774d0c..HEAD` lists 69907ddf ("a re-statement names its carried root; identifiers match exactly for originals") and 8966da2e (records-only rebind). `git diff --stat 8f774d0c..HEAD` shows 33 files. The following are **byte-unchanged 8f774d0c..HEAD** (empty diff stat): `scripts/build_reproducibility_report.py`, `scripts/check_reproducibility_and_independent_conformance.py`, `docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md`, `bench/release/tests/test_reproducibility.py`, `bench/review/tests/test_finding_register.py` and `docs/adr/`.
- I read the full owned delta: `git diff 8f774d0c..HEAD` of `scripts/build_finding_register.py` (current :799-835, :1296-1362, :1027-1124 re-read in full), `scripts/retire_review_claims.py` (:345-426, :428-437, :480-497), `docs/spec/FINDING_REGISTER_V1.md` (:1-6, :98-104, :119-135) and `bench/review/tests/test_retire_review_claims.py` (:14-73, :263-274, :313-321).
- I also read my `nabu_architecture_review-8f774d0.md` (whole), Ariadne `ariadne_contract_review-8f774d0.md:56` and `ariadne_contract_review-7622776.md:40`, `claim-retirements.json` (150 entries; the 5 new ones [145..149] with their cited lines), this section of `machine-summary.json` (every open, closed and restated list), `finding-register.json` (counts) and `reproducibility-report.json` (result fields).
- `python3 scripts/check_reproducibility_and_independent_conformance.py` → **exit 0**, `"result": "PASS"`, `"problems": []`, `S20_730_MECHANICS_IMPLEMENTED_REVIEW_PENDING`.
- `python3 scripts/check_finding_register.py` → **exit 0**, `"result": "PASS"`, `"problems": []`, `S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING`.
- `python3 scripts/retire_review_claims.py --check` → **exit 0**, `{"retirable": 0, "stale_closures": [], "regeneration_divergence": [], "result": "PASS"}`.
- `python3 -m unittest bench.review.tests.test_finding_register bench.review.tests.test_retire_review_claims` → `Ran 58 tests … OK`.
- `python3 -m unittest bench.release.tests.test_reproducibility` → `Ran 50 tests … OK`.
- In-memory probes:
  - `finding_key`, `same_finding` and `is_carry` over every restated claim ledger-wide (91), with a census of named re-statements whose own first identifier differs from their root's.
  - `line_speaks_about` (weak and strong) of hypothetical root-closing heads against real re-statements.
  - Equal-key duplicate groups in this section's `p4_open`.
  - Totals in `finding-register.json`: 312 closed, 91 restated, `unclaimed_carried_findings []`. Open P0-P2 counts are rw075 (4 P1 + 4 P2), release_candidate_packaging 5, this section 3, s20_700_remaining_surface_audit 1 and standards_sbom_and_provenance 1. That is ten 8f774d0 P2 claims from eight verdicts; the prompt's "eight P2s" counts verdicts.
  - Rebind: the report is `SINGLE_HOST_REPRODUCIBLE`, `distinct_hosts 1`, `required_hosts 2`, `superseded_attestations []`, one attestation at 69907ddf…, 0 hits for `02756fb1`.
- The tempfile git fixture drove `retire.fold_restatements` end-to-end on two scenarios (A and B, described below).

## Evidence checked

**Prior 8f774d0c findings (nabu_architecture_review-8f774d0: 1 P2 + 17 P4):**

- **[P2] [fail-open/identity-discipline] `finding_key` carried-identifier wildcard / `fold_restatements` binding — OPEN, narrowed.**
  - **What the repair fixed:**
    - Regeneration now runs reopen → retire → fold (retire :435-436, main :492-497).
    - The `:846-852` closure is now recorded: `p4_closed_claims` holds `nabu_architecture_review@79fdcc6: [contract-vs-mechanics] scripts/build_finding_register.py:846-852` verified by `nabu_architecture_review-b58ac1e.md#L32`.
    - The 15-claim chain under the c67b072 `verified_by` claim is undone (0 Nabu restated claims in this section).
    - Prose words no longer make a re-statement.
  - **What it did not fix — the rest of my stated closure:**
    - A named re-statement still keys `identifier = None` even when it carries its own identifier (build_finding_register.py:1330, :1342).
    - `None` still matches any identifier (:1352).
    - The fold still takes one root among several same-file candidates with different identifiers (retire :373-378). The ordering changed from list position to `scope_generation`, but no exact-identifier preference exists and ambiguity is not refused.
    - The requested test (two originals with distinct identifiers, plus a carried claim naming the second) was not added. The fold test at :313-321 only rewrote its strings to the named form.
  - **Mis-binding in the real ledger:** three Nabu `release_candidate_packaging` `[record-precision] (carried from b58ac1e0)` re-statements are each bound to the 1a9f0aa `line_speaks_about` finding. They name `is_open_line`, `raising_scope` and the `:794,1065` identifier rule. Each one's own b58ac1e original is open with a matching identifier (keys `is_open_line`, `raising_scope`, `p4_open`). Ariadne `[predicate-precision]` ×3 and Vulcan `[fail-closed-gap]` ×3 show the same pattern.
  - **Fixture scenario B:** two open originals `alpha_function` and `beta_function` against one lane/kind/file, plus `(carried from <first>, OPEN) … (\`beta_function\`)`. The re-statement folds into **alpha**.
  - **The fail-open leg survives through the new retired-root guard.** retire :388 calls `line_speaks_about(text[n - 1], entry)` with the default weak read. Every fold candidate shares the entry's kind by construction, so a specific kind phrase in the root's closing line satisfies the guard on its own.
    - Probe: `- **[P4] [record-precision] scripts/build_finding_register.py:713-742 (\`line_speaks_about\`) — CLOSED.**` returns weak True / strong False for all three mis-bound Nabu re-statements.
    - Probe: `- **[P4] [record-accuracy] … (\`p4_closed_claims[4]\`) — CLOSED.**` returns weak True / strong False for Ariadne's 8f774d0 `[128]` claim in this section.
    - **Fixture scenario A:** the root `alpha_function` is closed by a line naming only alpha, and the only open instance is `(carried from <first>, OPEN) … (\`beta_function\`)`. The claim is folded (1) and leaves `p4_open` with no line about `beta_function`. `--check` would reproduce this, because regeneration runs the same fold.
- **[P4] [contract-vs-mechanics] FINDING_REGISTER_V1.md:100-102 path-with-tag-word sentence — OPEN.** :100-102 still reads "or a path with a tag word standing there; the OPEN-line refusal reads the head without the path rule". The new paragraph (:121-133) does not amend it.
- **[P4] [fail-open] `tag_words` per-word, unshared-kind path rule — OPEN.** Untouched by the diff (now :1083-1086, :1124).
- **[P4] [fail-open/identity-discipline] `description[:80]` — OPEN.** Untouched; still present (:621).
- **[P4] [test-coverage] `raising_severity` / `paths_ok` — OPEN.** `grep -c raising_severity` returns 0 in both suites.
- **[P4] [fail-open/contract-precision] `open_lines_about` vs the unshared phrase rule; the CLOSED+OPEN test — OPEN.** Code untouched. `OPEN` in the retire test appears only in claim strings (:316, :318).
- **[P4] [contract-vs-mechanics] `round_scope` / test :392 `or True` — OPEN.** test_finding_register.py is byte-unchanged; :392 still asserts `… or True`.
- **[P4] [identity-discipline/records] ledger-anchored `finding_key` origin-round key — OPEN.** The c67b072 pair still shares the key `('nabu','identity-discipline/records','c67b072','p3_open')`, and the 76ae15a pair shares `(…,'76ae15a','p3_open')`.
- **[P4] [contract-vs-mechanics] `raising_severity` → None untagged — OPEN.** Unchanged.
- **[P4] [duplicated-authority] `identity_tokens` vs `line_speaks_about` — OPEN.** Unchanged (:785 vs :1027-1124).
- **[P4] [contract-vs-mechanics] `cited_closure_lines` PRIOR fallback — OPEN.** Unchanged (:872).
- **[P4] [contract-vs-mechanics] `is_tracked` fail-open / replay exact-claim branch — OPEN.** `return _tracked is None or path in _tracked` (:1162) is unchanged.
- **[P4] [duplicated-authority] `role`/`reviewer_of`, `strictly_later`/`strictly_later_scope`, `scope_generation` — OPEN, extended.** The fold now also orders roots by `scope_generation` (`rev-list --count`) at retire :376, and calls that "git ancestry" (retire comment :371-372; spec :131).
- **[P4] [contract-vs-mechanics] `STATUS_MARK` mixed status — OPEN.** Unchanged (:677).
- **[P4] [identity-discipline/records] `is_carry` vs the 400-character cut — OPEN, changed in form.** Prose markers now never fold, by design. That fails closed, but the lane's own "Carried unchanged:" re-statements now all stay open: this section's `p4_open` has 219 entries, 105 distinct keys and 47 equal-key groups. My `role` claims read `is_carry` False (446/434/446 chars). A `carried from <sha>` past the recorded cut is still unseen.
- **[P4] [identity-discipline/records] `nabu_architecture_review_revision_8: [note]` — OPEN.** Still in `p4_open`. None of the 5 new exact-claim entries names it.
- **[P4] [contract-precision] REPRO spec :149-151 vs builder :472-480 — OPEN.** Byte-unchanged.
- **[P4] [identity-discipline/contract-revision] `CONTRACT_REVISION = 11` — OPEN.** check script :35 unchanged; the spec is byte-unchanged.

**Repair (4): five exact-claim re-bindings.** Entries [145..149] each cite their own lane's item line at the stated position. I checked the four at `nabu_architecture_review-1a9f0aa.md` #L40/#L39/#L36/#L34 and the one at `ariadne_contract_review-1a9f0aa.md#L22`. Each line is a `[P4] [<same kind>] … — CLOSED.` head naming that claim's file. I accept these.

**New observation: carry naming.** `CARRIED_FROM` (:1299) searches the whole body, including backticked quotations of other claims. `LEADING_CARRY` (:1300) accepts `(carried …)`, `(prior …)` and `(residual …)` clauses without a sha. Both contradict spec :121-125 ("a leading `(carried from <sha>, …)` clause or a `carried from <sha>` phrase").

Ariadne's 8f774d0 finding in this section, `[record-accuracy] evidence/review/claim-retirements.json entry [128] (carried from the [85] finding, …)`, shows the effect:
- By its own text it is a new finding about entry [128]. Its predecessor, "[85]", is said to be closed at 7622776#L28.
- It keys origin `7622776` only because it quotes a Nabu claim containing "carried from 76227765".
- It is therefore recorded as restating the unrelated Ariadne 7622776 `p4_closed_claims[4]` finding (the revision_10 blanket retirement, 7622776.md:40).
- Scenario A's weak guard would carry it away with that root's closure.

## Findings

[P2] [fail-open/identity-discipline] scripts/build_finding_register.py:1322,1330,1342 (`finding_key`: `identifier = None if named`, even when the claim has its own identifier), :1352 (`same_finding`: None matches any); scripts/retire_review_claims.py:373-378 (root = earliest same_finding candidate by `scope_generation`, no exact-identifier preference, no ambiguity refusal), :388 (retired-root guard `line_speaks_about(text[n-1], entry)` on the weak read); docs/spec/FINDING_REGISTER_V1.md:121-133 - Carried from 8f774d0c, OPEN, narrowed. Fixed: retire now precedes fold, the `:846-852` closure is recorded at b58ac1e#L32, the 15-claim chain is undone and prose words no longer fold. Still open: a re-statement that names its carried root still wildcards over every finding of its lane/kind/file. In the ledger, three Nabu `(carried from b58ac1e0)` `[record-precision]` re-statements (`is_open_line`, `raising_scope`, identifier rule) are bound to the 1a9f0aa `line_speaks_about` finding while their own originals are open; Ariadne `[predicate-precision]` ×3 and Vulcan `[fail-closed-gap]` ×3 are bound the same way. The new retired-root guard reads weak, so a root-closing head naming only its kind satisfies it for every candidate (probe: weak True / strong False for all three Nabu re-statements and for Ariadne [128]). Fixture A: a carried `beta_function` claim folds into a retired `alpha_function` root closed by a line naming only alpha and leaves `p4_open` with no line about it. Fixture B: it folds into the open alpha instead of the open beta. Every checker PASSes - Closure: key a named re-statement on its own first identifier when it has one (wildcard only when absent); prefer an exact-identifier root and refuse a wildcard matching two distinct identifiers; read the retired-root guard strongly (head only, identifier/finding id/anchor/quoted phrase, with the lane's shared vocabulary); add tests for scenarios A and B; regenerate.

[P3] [identity-discipline] scripts/build_finding_register.py:1299 (`CARRIED_FROM` over the whole body, including backticked quotations), :1300 (`LEADING_CARRY` accepts sha-less `(carried …)`, `(prior …)`, `(residual …)`), :1336; docs/spec/FINDING_REGISTER_V1.md:121-125; machineresearch/sley-2.0/machine-summary.json (`reproducibility_and_independent_conformance.p4_restated_claims[0]`) - The mechanics accept carry forms the spec excludes. Ariadne's 8f774d0 `[record-accuracy] … entry [128]` finding is new by its own text (its "[85]" predecessor is closed), yet it is recorded as restating the unrelated Ariadne 7622776 `p4_closed_claims[4]` blanket-retirement finding. The only reason is that it quotes a Nabu claim containing "carried from 76227765", which sets its origin to 7622776 - Closure: take the carried sha only from the claim's own leading clause or outside backticked or quoted spans; require a sha in `LEADING_CARRY` as the spec states; unfold `[128]`; add a test with a quoted foreign "carried from <sha>".

[P4] [identity-discipline/contract-revision] docs/spec/FINDING_REGISTER_V1.md:3 ("revision 7 (2026-09-19)"); scripts/build_finding_register.py:35 (`CONTRACT_REVISION = 7`) - Fold identity was redefined in place at 7527d82a, 4e128def, 02756fb1 and 69907ddf (re-statement keying, wildcard semantics, fold order) with revision 7 unchanged. A register built at any of those commits claims the same contract revision - Closure: bump the revision on a semantic change, or keep an explicit in-place amendment record.

[P4] [contract-vs-mechanics] docs/spec/FINDING_REGISTER_V1.md:100-102 ("or a path with a tag word standing there; the OPEN-line refusal reads the head without the path rule"), :116-117 vs scripts/build_finding_register.py:1124, :1079-1086 - Carried unchanged: the removed path rule is still stated for the strong read, and the shared-words sentence still has no effect under it - Closure: amend :100-102 and state where the exclusion applies.

[P4] [fail-open] scripts/build_finding_register.py:1083-1086 (`tag_words` per 5+-letter word), :1124 (unshared-kind path rule) - Carried unchanged: under an unshared kind, a path plus a word two kinds share relates to both findings - Closure: match the whole kind phrase in the path rule; a test with two kinds sharing a word.

[P4] [fail-open/identity-discipline] scripts/build_finding_register.py:575-598, :621 (`description[:80]` prefix binding of the raising scope) - Carried unchanged - Closure: match through the anchor's line span, refuse a severity mismatch, test two findings sharing an 80-character prefix.

[P4] [test-coverage] scripts/build_finding_register.py:602-623 (`raising_severity`), :1027 (`paths_ok`); bench/review/tests/test_finding_register.py, test_retire_review_claims.py - Carried unchanged: `raising_severity` appears 0 times in both suites; `paths_ok` and a retired claim whose raising line names another severity have no assertion - Closure: direct tests.

[P4] [fail-open/contract-precision] scripts/build_finding_register.py:847-869 (`open_lines_about`), :1062-1067 (phrase rule, unshared kind); bench/review/tests/test_retire_review_claims.py - Carried unchanged: closure and OPEN refusal still read asymmetrically for an unshared kind; the CLOSED+OPEN single-transcript test does not exist - Closure: one symmetric read, plus the test.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:152-157 (`round_scope`, 40-hex only), :160-168 (`scoped_before`); bench/review/tests/test_finding_register.py:392 (`… or True`) - Carried unchanged - Closure: read 7-40-hex scopes, refuse a scope-less PASS fold, fix the assertion.

[P4] [identity-discipline/records] scripts/build_finding_register.py:1334-1343 (ledger-anchored key: lane, kind, origin round, first identifier) - Carried unchanged: the c67b072 `[identity-discipline/records]` pair and the 76ae15a pair each still share one key (`…,'c67b072','p3_open'` / `…,'76ae15a','p3_open'`) - Closure: add the anchor's line span to the ledger key; a test with two ledger findings from one round.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:602-611 (`raising_severity` → None without a tag); docs/spec/FINDING_REGISTER_V1.md:104-105 - Carried unchanged - Closure: derive the severity from the matched raising line, or state the exemption; a mismatch test.

[P4] [duplicated-authority] scripts/build_finding_register.py:785-796 (`identity_tokens`) vs :1027-1124 (`line_speaks_about`) - Carried unchanged: the phrase and identifier rules are still duplicated - Closure: make `line_speaks_about` consume `identity_tokens`; a test that both agree.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:872-900 (`cited_closure_lines`, PRIOR fallback); scripts/retire_review_claims.py:154-155 - Carried unchanged - Closure: restrict the fallback to closure lines whose head names no severity; a test.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:1151-1162 (`is_tracked`, `_tracked is None` → filed); scripts/retire_review_claims.py:469-472 (replay exact-claim branch without `open_lines_about`) - Carried unchanged - Closure: fail closed when `git ls-files` fails; apply `open_lines_about` in replay; tests.

[P4] [duplicated-authority] scripts/retire_review_claims.py:54-60 (`role`) vs scripts/build_finding_register.py:99-104 (`reviewer_of`); retire :76-89 (`strictly_later`) vs register :944 (`strictly_later_scope`) vs :546 (`scope_generation`); retire :203, :376; docs/spec/FINDING_REGISTER_V1.md:131 - Carried, extended: the fold root is now also ordered by a reachable-commit count that the comment and spec call "git ancestry" - Closure: one resolver, one strict ancestry predicate, one ancestry order imported by both scripts; a test with equal scopes.

[P4] [contract-vs-mechanics] scripts/build_finding_register.py:677-679 (`STATUS_MARK`), :723-733 (`is_closure_line`) - Carried unchanged: mixed `CLOSED … OPEN` heads are still P3 closure lines - Closure: treat an unquoted OPEN in the item's own status span as mixed; tests for the four shapes.

[P4] [identity-discipline/records] scripts/build_finding_register.py:1355-1362 (`is_carry`) vs the records' 400-character description cut; machineresearch/sley-2.0/machine-summary.json (this section's `p4_open`: 219 entries / 105 keys / 47 equal-key groups) - Carried, changed in form: prose carries now stay open by design (fail-closed over-count); a `carried from <sha>` past the cut is still unseen - Closure: record the whole description or the carry marker as a field; a test with a marker beyond 400.

[P4] [identity-discipline/records] machineresearch/sley-2.0/machine-summary.json (this section's `p4_open`, `nabu_architecture_review_revision_8: [note]`); evidence/review/verdicts/reproducibility_and_independent_conformance/nabu_architecture_review-92fa664.md:25 - Carried unchanged - Closure: one exact-claim entry.

[P4] [contract-precision] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:149-151 vs scripts/build_reproducibility_report.py:472-480 - Carried unchanged (byte-unchanged 8f774d0c..HEAD) - Closure: qualify the sentence or reorder the check, plus a test.

[P4] [identity-discipline/contract-revision] docs/spec/REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:3; scripts/check_reproducibility_and_independent_conformance.py:35 - Carried unchanged: `CONTRACT_REVISION = 11` was amended in place - Closure: revision 12 or an explicit amendment record.

## Assessment

The section's own mechanics (reproducibility builder, checker, spec, tests) are byte-unchanged. The rebind to 69907ddf is clean, and all five checker and test runs exit 0 (58 + 50 tests). The round fixed real parts of my 8f774d0c P2:
- Retire now runs before fold.
- My verified b58ac1e#L32 closure of the `:846-852` finding is recorded.
- The 15-claim mis-chain is gone.
- Prose words no longer turn originals into re-statements.
- The five exact re-bindings cite their own item lines.

The mechanism the P2 named is still there, though. A re-statement that names its carried root still drops its own identifier and matches any finding of its lane, kind and file. In the ledger, nine re-statements are therefore bound to findings they do not restate, three of them in my own lane. The new guard meant to stop folds into retired roots reads weakly, so a closing head that names only the shared kind satisfies it for every candidate. The tempfile fixture shows a carried finding leaving `p4_open` under an unrelated closure while every checker would stay green.

The P2 therefore stays open (narrowed) and the verdict is REVISE. I also add one P3 (a quoted foreign "carried from <sha>" plus sha-less leading clauses mis-fold Ariadne's `[128]` finding in this section) and one P4 (Finding Register revision 7 amended in place across four fix rounds). All seventeen prior P4s remain open (two changed in form, one extended). No gate moves, and I make no GA or release-readiness claim.

VERDICT: REVISE_0_P0_0_P1_1_P2_1_P3_18_P4
SECTION: reproducibility_and_independent_conformance
FIELD: nabu_architecture_review
SCOPE_SHA: 8966da2e0a52a6f6e56949e9a5902464c86613c0
FINDINGS: [P2] [fail-open/identity-discipline] scripts/build_finding_register.py:1322,1330,1342,1352; scripts/retire_review_claims.py:373-378,388; docs/spec/FINDING_REGISTER_V1.md:121-133 - carried 8f774d0c P2, OPEN narrowed: a named re-statement still keys identifier None and wildcards over its lane/kind/file (3 Nabu, 3 Ariadne, 3 Vulcan re-statements bound to findings they do not restate); the retired-root guard reads weak, so a kind-only closing head satisfies it (fixture A: a carried `beta_function` claim folds into a retired alpha root; fixture B: into the open alpha instead of beta) - Closure: keep the own identifier, prefer exact matches, refuse ambiguity, read the guard strongly, tests A/B, regenerate. | [P3] [identity-discipline] scripts/build_finding_register.py:1299,1300,1336; FINDING_REGISTER_V1.md:121-125; machine-summary.json this section's p4_restated_claims[0] - CARRIED_FROM reads quoted foreign shas and LEADING_CARRY accepts sha-less clauses; Ariadne 8f774d0 [128] is mis-folded under the unrelated 7622776 p4_closed_claims[4] finding - Closure: take the sha from the claim's own clause, require a sha, unfold, add a test. | [P4] [identity-discipline/contract-revision] FINDING_REGISTER_V1.md:3; build_finding_register.py:35 - revision 7 amended in place across four rounds - Closure: bump or record the amendment. | [P4] [contract-vs-mechanics] FINDING_REGISTER_V1.md:100-102,116-117 - carried unchanged. | [P4] [fail-open] build_finding_register.py:1083-1086,1124 - carried unchanged. | [P4] [fail-open/identity-discipline] build_finding_register.py:575-598,621 - carried unchanged. | [P4] [test-coverage] build_finding_register.py:602-623,1027 - carried unchanged. | [P4] [fail-open/contract-precision] build_finding_register.py:847-869,1062-1067 - carried unchanged. | [P4] [contract-vs-mechanics] build_finding_register.py:152-168; test_finding_register.py:392 - carried unchanged. | [P4] [identity-discipline/records] build_finding_register.py:1334-1343 - carried unchanged. | [P4] [contract-vs-mechanics] build_finding_register.py:602-611 - carried unchanged. | [P4] [duplicated-authority] build_finding_register.py:785-796 vs :1027-1124 - carried unchanged. | [P4] [contract-vs-mechanics] build_finding_register.py:872-900; retire_review_claims.py:154-155 - carried unchanged. | [P4] [contract-vs-mechanics] build_finding_register.py:1151-1162; retire_review_claims.py:469-472 - carried unchanged. | [P4] [duplicated-authority] retire_review_claims.py:54-60,76-89,203,376 vs build_finding_register.py:99-104,546,944 - carried, extended. | [P4] [contract-vs-mechanics] build_finding_register.py:677-679,723-733 - carried unchanged. | [P4] [identity-discipline/records] build_finding_register.py:1355-1362; this section's p4_open - carried, changed in form. | [P4] [identity-discipline/records] this section's p4_open revision_8 [note]; nabu_architecture_review-92fa664.md:25 - carried unchanged. | [P4] [contract-precision] REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:149-151 vs build_reproducibility_report.py:472-480 - carried unchanged. | [P4] [identity-discipline/contract-revision] REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md:3; check script :35 - carried unchanged.
SUMMARY: Three checkers and two test suites pass (58 + 50 tests), the rebind is clean, and the 69907ddf repair fixes the fold order, records my b58ac1e#L32 closure, undoes the 15-claim chain and stops prose words from folding. The P2 I raised stays open: a named re-statement still matches any finding of its lane/kind/file (nine ledger mis-bindings, three in my lane), and the new retired-root guard reads weakly. A tempfile fixture shows a carried finding folding under an unrelated closure with every checker green. There is one new P3 (a quoted foreign sha and sha-less carry clauses mis-fold Ariadne's [128] finding in this section) and one new P4 (Finding Register revision 7 amended in place); all seventeen prior P4s remain open. No GA or release-readiness claim is made.
