<!-- engine: claude-code; observed model: claude-opus-5-5; scope: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e; role: ariadne; field: ariadne_contract_review; dispatched: 2026-09-23T05:26:58Z; duration_s: 986; process_exit_code: 0 -->
# Ariadne Council review — release_candidate_packaging

Harness: claude-code
Reviewed checkpoint: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e

What I verified myself. This review was read-only: I created, edited and committed nothing, and `git status --porcelain` shows 0 lines after the review. I wrote no fixtures to disk. All probes ran in memory against the repository's own modules and the tracked ledgers.

- `git rev-parse HEAD` → `d158d26bfdda9fc7dc899a1d1a8a141c2da6610e`, so the scope matches.
- `git log --format='%h %s' 8966da2e..HEAD` lists eight commits: acbc65f0, 883361e3, f7f9af90, 5fd10849, 0f4b8408, 7c51710b, 8d063f00 and d158d26b.
- `git diff --stat 8966da2e..HEAD` → 53 files. `git diff --name-only 8d063f00..d158d26b` outside `evidence/` and `machineresearch/` → none, so the mint is records-only.
- Diffs read in full over 8966da2e..HEAD: `docs/spec/FINDING_REGISTER_V1.md`, `docs/spec/RELEASE_CANDIDATE_PACKAGING_V1.md` and the ADR-0042 note. I also read the added entries of `evidence/review/claim-retirements.json` and the test names added in `bench/review/tests/test_retire_review_claims.py` and `test_finding_register.py`.
- Read at HEAD:
  - `FINDING_REGISTER_V1.md` :20-219
  - `scripts/retire_review_claims.py` :246-675 (`retire`, `fold_restatements`, `reopen_all`, `closer_disagreements`, `split_status_problems`, `regeneration_divergence`, `replay_problems`, `main`)
  - `scripts/build_finding_register.py` :1080-1150 (`ledger_words`, `is_lane_field_name`, `verdict_field_names`) and :1440-1575 (`_own_identifier`, `CARRIED_FROM`, `LEADING_CARRY`, `finding_key`, `same_finding`, `unquoted`, `is_carry`, `named_carry_sha`)
  - `test_finding_register.py` :393-396, :499-502, :542-555, :981-1010
- Transcripts read: my `ariadne_contract_review-8966da2.md` (whole); `nabu_architecture_review-8f774d0.md` :38; `nabu_architecture_review-1a9f0aa.md` :28; `vulcan_surface_review-b58ac1e.md` :45-48; `nabu_architecture_review-8966da2.md` :52 and :92 (grep).
- Checkers (each exit 0):
  - `python3 scripts/check_release_candidate_packaging.py` → `"result": "PASS"`, `"problems": []`, `"revision": 7`.
  - `python3 scripts/check_finding_register.py` → `"result": "PASS"`, `"problems": []`.
  - `python3 scripts/retire_review_claims.py --check` → `"retirable": 0`, `"stale_closures": []`, `"regeneration_divergence": []`, `"split_status": []`, nine report-only `closer_disagreements`, `"result": "PASS"`.
  - `python3 -m unittest discover -s bench/review/tests -t .` → `Ran 170 tests … OK`.
  - `python3 -m unittest discover -s bench/release/tests -t .` → `Ran 156 tests … OK`.
  - Both counts equal the test inventory: 170 and 156.
- In-memory probes:
  - `scoped_before`, `strictly_later_scope` and `strictly_later` on equal scopes.
  - `is_lane_field_name` on `final_vulcan_disposition`, `implementation_ariadne_review` and `package_closed_claims`.
  - `claim_finding_ids` on `S20-700-PACK-001`, `RW090-DEV-01` and `V-02`.
  - `LEADING_CARRY` on sha-less clauses.
  - `open_lines_about(nabu-1a9f0aa.md, nabu@c67b072 [revision-identity])`.
  - `finding_key`, `is_carry` and `named_carry_sha` over the section's split families.
  - `speaking_lines`, `relation_problem` and `open_lines_about` for the reopened nabu `[verification-depth]` root and the nabu `[ledger-duplication]` root.
  - A census of all 72 re-statements register-wide: root status, named sha vs the root's scope, and key equality.
  - A census of named-carry vs named-root status pairs, matched on lane, kind and file/origin at the named scope.
  - A status-transition diff of every claim between the ledger at 8966da2e and at HEAD.
  - `unquoted`/`is_carry` edge probes.
  - `package_open_findings` vs `package_open_claims` for this section, and the GA open row.
  - Candidate identity across the reproducibility report, content checks, provenance and GA report.

## Evidence checked

**Disposition of my 8966da2e findings (`ariadne_contract_review-8966da2.md`: 2 P2, 1 P3, 6 P4):**

- **[P2] [regeneration-order] scripts/retire_review_claims.py:494-497 `fold_restatements` / retire-before-link — CLOSED as a P2; the residual live splits are re-raised as the P3 [split-status] below.**
  - Repaired where it acts:
    - Shared vocabulary now exempts a same-finding carry derived from the claims themselves (spec :133-134, :166-170), so an original with a live re-statement keeps its identifier.
    - A second fold pass gives an open copy the status of a retired claim with the same identifier-bearing key, provided the closing line names that copy and strictly postdates it (`fold_restatements` :467-487, `closing_lines_name` :397-425).
    - `split_status_problems` (:543-576) runs under `--check` (:663-665) and fails the gate on an identifier-bearing split.
    - The fixture is pinned: `test_one_finding_receives_one_status`, `test_split_status_refusal` and `test_equal_key_open_claim_inherits_a_later_closure` all pass.
    - The docstring order is now reopen → retire → fold (:581-582).
  - Of the four live instances I named, only nabu `[revision-identity]` is reconciled: `@c67b072` and `@76ae15a` are both CLOSED via nabu-1a9f0aa.md#L31, and `open_lines_about(…#L67)` → `[]`.
  - The other three are still split, as listed under the P3 [split-status] below.

- **[P2] [fold-identity] scripts/build_finding_register.py:1322, :1330, :1342 (`identifier = None`, `same_finding` wildcard) — CLOSED.**
  - `same_finding` is now equality (:1530-1537), and a named carry keys on its own identifier (:1498-1515).
  - The fold resolves its root by `named_carry_sha` at the scope the carry names. It stays open when zero or two roots match (:434-444).
  - A fold into a retired root requires the root's cited closing lines to speak about the carry under the retirement read, with a closer that strictly postdates the carry (:397-425, :446-449).
  - Census: 72 re-statements register-wide, 0 folded into a closed root, 0 whose named sha differs from the root's scope, 0 whose key differs from the root's.
  - My five `[predicate-precision]` re-statements and one `[relation-precision]` re-statement now restate their open `@79fdcc6`/`@b58ac1e` roots, not `is_closure_line`@76ae15a.
  - Pinned tests: `test_runbook_tracking_carry_folds_into_its_named_root`, `test_fold_prefers_the_open_named_root_over_a_closed_twin`, `test_restatement_naming_an_absent_root_stays_open`, `test_an_ambiguous_named_root_stays_open`.

- **[P3] [contract-text] docs/spec/FINDING_REGISTER_V1.md:80-81 ("reopen → fold → retire") — CLOSED.**
  - :84 now reads "reopen → retire → fold".
  - :32-35 now reads "naming its carried root … folds into the claim at the round it names".
  - :111-121 now reads "keys on the identifier it carries, never a wildcard … never by a bare carry word".
  - The old :96-97 is :100-102 and is consistent.
  - The docstring (:581-582) is corrected.
  - `CONTRACT_REVISION = 9` in the builder (:35), in the checker (:163) and in `finding-register.json` `contract_revision`.
  - Two leftover wordings are filed as a new P4 [help-text].

- **[P4] [contract-precision] scripts/build_finding_register.py:1300 `LEADING_CARRY` — CLOSED.** :1482 requires `from <sha>` inside the parentheses. The probes `(prior)` and `(carried, worse)` return None. A grep for `\bCARRY\b` over scripts finds 0. Pinned by `test_sha_less_carry_clauses_are_originals`.
- **[P4] [test-coverage] `raising_severity` / `_own_status` / exact-claim OPEN refusal / own-head preference — CLOSED.** Covered by test_finding_register.py:981-1010, `test_exact_claim_open_refusal` and `test_own_head_lines_are_preferred_over_trailing_prose`. A grep for `or True` over bench/review/tests finds 0.
- **[P4] [predicate-precision] `scoped_before` vs `strictly_later_scope` — CLOSED.** The contract now decides the equal-scope case (spec :81-83). Probes: `scoped_before(x,x)` True, `strictly_later_scope(x,x)` False, `strictly_later(x,x)` False. All agree with the spec and are tested at :542-545.
- **[P4] [predicate-precision] `is_lane_field_name` — CLOSED.** It is derived from the summary's verdict-bearing fields (`verdict_field_names`, :1125-1147). `final_vulcan_disposition` → True, tested at :555.
- **[P4] [relation-precision] `is_open_line` / `open_lines_about` — CLOSED.** nabu-1a9f0aa.md:67 no longer holds `nabu@c67b072 [revision-identity]` open (`[]`). Pinned by `test_open_check_bounded_head_and_shared`.
- **[P4] [predicate-precision] `claim_finding_ids` (S20-700-PACK-001) — CLOSED.** Probe: `{'S20-700-PACK-001'}` and `{'RW090-DEV-01'}`. Tested at :548-549.

**Older claims of my lane that this delta closes.** Each is verified against the same code and census:

- **[P2] [identity-precision] scripts/build_finding_register.py:1334-1339 `same_finding` ("equal or absent on either side") with `CARRY` over the body — CLOSED.** `same_finding` is equality and `CARRY` is removed.
- **[P3] [fold-identity] docs/spec/FINDING_REGISTER_V1.md:73-75 with `finding_key` ledger-file branch and `is_carry` 160-character window — CLOSED.** `is_carry` reads `CARRIED_FROM` over `unquoted(body)` or a sha-bearing `LEADING_CARRY` (:1549-1559). The ledger-file branch keys on the named or raising origin plus the identifier (:1516-1527).
- **[P3] [fold-identity] (8f774d0c) docs/spec/FINDING_REGISTER_V1.md:107-116, :398-400 `CARRIED_FROM` file-anchored branch — CLOSED.** There are 0 wrong-target folds (census above), `--check` has a split refusal, and a runbook-tracking fold test exists. The named-carry split residue is tracked in the P3 [split-status].
- **[P4] [contract-text] docs/spec/FINDING_REGISTER_V1.md:32-33 and :96-97 (8f774d0c) — CLOSED.** Rewritten as above.

**What the delta breaks, verified at HEAD:**

1. **Named carries that no fold pass links.**
   - Fold pass 1 needs `same_finding`, which requires equal identifiers (:440).
   - Fold pass 2 needs a non-empty identifier (:475).
   - The split refusal counts only identifier-bearing keys (:565).
   - So a named carry whose identifier is absent, or differs from its named root's, is never linked and never refused.
   - Census: 18 named-carry/named-root pairs in this section have different statuses, and none exist elsewhere. `--check` still reports `split_status []`.
   - Between 8966da2e and HEAD, 29 P4 and 1 P3 claims in this section moved from restated to open.
   - That movement includes the nabu lint-branch family that I verified as consistent at 8966da2e:
     - `[test-coverage]` @7622776/@c67b072/@76ae15a (carried from c04539b9) are OPEN. Their root keys on `lint`, taken from "grep `lint`".
     - `[runbook-tracking]` @db53894/@7622776/@c67b072 are OPEN.
     - `[cleanliness-standard]` @c67b072/@76ae15a are OPEN. The root keys on `workin`.
     - `[contract-wording]` @76ae15a is OPEN.
     - All of their roots are CLOSED by the lane's own lines (nabu-1a9f0aa.md #L34/#L36/#L39/#L40).
   - nabu `[ledger-duplication]`:
     - The delta bound three more carries (@1a9f0aa/@6589c6e/@79fdcc6) CLOSED to nabu-8f774d0.md#L38 as a "split-status reconciliation".
     - The root they name, @7622776, stays OPEN (`speaking_lines []`, `relation_problem` None, `open_lines_about []`, so an exact-claim binding was available). The carry @76ae15a also stays OPEN.
     - nabu-8966da2.md:52 itself records "the 76227765 finding closed at 8f774d0c".
   - vulcan `[robustness]` record_lint_report.py: vulcan-b58ac1e.md#L45 says it "Closes the untagged claim and its `@db53894` carry". The @7622776 copy "(carried from c04539b9/db53894e, unchanged)" and the @c67b072 and @76ae15a copies remain OPEN.
   - vulcan `[robustness]` retire_review_claims.py: the @7622776 copy is CLOSED via #L48, while @c67b072 "(carried from 76227765…)" is OPEN.

2. **A lane-closed P3 reopened.**
   - `ledger_words` (:1088-1092) now includes `package_closed_claims`, which is also the function the finding is about (build_finding_register.py:1343).
   - That word was the only identity of nabu@7622776 `[verification-depth]`.
   - At 8966da2e this claim was CLOSED via nabu-1a9f0aa.md#L28, the lane's own `[P3] [verification-depth] … (package_closed_claims) — CLOSED` item line. At HEAD it is OPEN in `p3_open`.
   - Probe: `speaking_lines []`, shared vocabulary `['verification-depth']`, `relation_problem` None, `open_lines_about []`.
   - Both named carries, @c67b072 and @76ae15a, stay CLOSED via the same #L28.
   - Register-wide, 8 closures reopened in this delta: 1 P3 here and 7 P4s in reproducibility. There is no per-claim change record for revisions 8/9 of the kind `revision-7-retirement-changes.json` gives for revision 7.
   - Every split and the reopen are conservative: open over-counts and nothing is retired falsely. P0–P2 are unaffected. For this section, `package_open_findings` P2 7 equals the claim count 7, and the GA row states "24 … per-package open claims, 23 distinct findings".

**Candidate at d158d26b:**
- Artifact `ecf51188d52bd673800c7b91068f2ad58d93068a23e1f37e60320c58a398c1f7`, 2,508,082 B, commit `8d063f00133f369599032191708fa7efabb3e2fa`, manifest `f0bd4072…`, 15 members.
- These values are equal across the reproducibility report (`REPRODUCIBLE`, single host `primary`, `working_tree_clean` true), the content checks (`PASS`) and provenance (`publication_authorized` false, unsigned).
- GA report: `ga_claimed` false.

## Findings

[P3] [split-status] docs/spec/FINDING_REGISTER_V1.md:37-38, :163-164 vs :175-179, :189-193 with scripts/retire_review_claims.py:434-444, :471-487 (`keys[entry][3]` at :475) and :543-576 (`split_status_problems`, identifier-bearing keys only) - (residual of my 8966da2e P2 [regeneration-order]) a named carry whose identifier is absent or differs from its named root's is linked by neither fold pass and refused by no gate, so one finding keeps two statuses. There are 18 such named-carry/named-root pairs in this section, and `--check` reports `split_status []`. Three families are still split: nabu [ledger-duplication] (four carries CLOSED via nabu-8f774d0.md#L38, three of them bound in this delta as a "split-status reconciliation", while the named root @7622776 and the carry @76ae15a stay OPEN although nabu-8966da2.md:52 records the 76227765 finding closed at 8f774d0c); vulcan [robustness] record_lint_report.py (@7622776/@c67b072/@76ae15a OPEN against vulcan-b58ac1e.md#L45); and vulcan [robustness] retire_review_claims.py (@c67b072 OPEN vs @7622776 CLOSED). The delta also moved the nabu lint-branch family ([test-coverage], [runbook-tracking], [cleanliness-standard], [contract-wording]; 9 named carries of closed roots) from restated to OPEN, because the roots key on grep words (`lint`, `workin`). The contract's headline ":37-38 … `--check` refuses a split" is contradicted by its own later exemptions and by the records - closure evidence needed: a named carry linked to its named root by lane/kind/file and named sha whatever its identifier (or each pair reconciled by a recorded binding or a recorded distinctness reason, the nabu root @7622776 included); `--check` reporting a named-carry split; :37-38 amended to state the scope of the refusal; a fixture with an identifier-less carry of an identifier-bearing root.

[P3] [closure-regression] scripts/build_finding_register.py:1088-1092 (`ledger_words` adds `package_closed_claims`/`package_open_claims`) with :1109-1122 (`is_lane_field_name`) and :1457-1475 (`_own_identifier`) vs docs/spec/FINDING_REGISTER_V1.md:61-63 and :37-38 - a name that is both a register output key and the function the finding is about (build_finding_register.py:1343) is stripped as ledger vocabulary. As a result nabu@7622776 `[verification-depth]` loses the closure recorded at 8966da2e via nabu-1a9f0aa.md#L28, the lane's own `[P3] [verification-depth] … (package_closed_claims) — CLOSED` line. It is now in `p3_open`, while its named carries @c67b072 and @76ae15a remain CLOSED via the same #L28 (probe: `speaking_lines []`, shared `['verification-depth']`, relation None, open lines `[]`). Register-wide, 8 closures reopened in this delta with no per-claim change record - closure evidence needed: a code identifier defined in or anchored to the claim's file kept as an identity (or an exact-claim binding of the root to nabu-1a9f0aa.md#L28); a test with `package_closed_claims`; a per-claim record of the closures revisions 8/9 changed, with the refusing rule.

[P4] [help-text] scripts/retire_review_claims.py:639-640 (`--fold-restatements` help: "into its earliest claim") and :656-657 (comment "reopening every closure, folding and retiring") vs docs/spec/FINDING_REGISTER_V1.md:84 (reopen → retire → fold) and :152-156 ("never by earliest scope") - the tool's user-facing help and gate comment still state the superseded root rule and order - closure evidence needed: both strings corrected.

[P4] [quote-precision] scripts/build_finding_register.py:1540-1546 (`unquoted`) with :1549-1559, :1562-1572 vs docs/spec/FINDING_REGISTER_V1.md:179-182 - an unbalanced straight `"` (for example `5"`) swallows a genuine carry: `is_carry('… 5" gap; carried from abcdef1, OPEN; "q" …')` → False. A single-quoted foreign carry still counts: `'carried from abcdef1'` → True. There are 0 live instances; the one quoted-only carry register-wide is the intended case - closure evidence needed: quoted spans bounded to balanced pairs, single quotes decided in the contract, a test.

## Assessment

On the paths it names, the delta repairs what my 8966da2e review found:
- Identity is no longer a wildcard.
- Folds resolve by the named sha and never land on a closed root whose line does not name them: 72 of 72 are clean, and my five predicate-precision re-statements are re-pointed.
- Retire-before-link no longer strips an original's identifier.
- `--check` has a split refusal.
- The contract text states one order and one keying rule.
- All six of my P4s are closed with tests.
- The candidate is bound consistently at 8d063f00 by a records-only mint. Every checker passes, and the 170 review and 156 release tests pass.

The repair's strictness has a cost the round did not record. Identifier equality became the only link for both fold passes and for the split refusal. Named carries that name their root by sha but carry no identifier, or a different one, are therefore now unlinked. Eighteen such pairs in this section hold two statuses, and `--check` passes. This includes a nabu lint-branch family that was consistent at 8966da2e. It also includes a "split-status reconciliation" for nabu ledger-duplication that closed three more carries but left the root they name open.

Separately, the new `package_*` ledger-word exclusion strips a real function identifier and reopens a P3 that its own lane had closed, with no change record. Both defects are conservative: they over-count open, retire nothing falsely and leave P0–P2 untouched. They are still records non-conformances with FINDING_REGISTER_V1.md :37-38 and :163-164 as written, hence REVISE. Nothing here assesses GA or release readiness.

VERDICT: REVISE_0_P0_0_P1_0_P2_2_P3_2_P4_PRIOR_P3_P4_CLOSED
SECTION: release_candidate_packaging
FIELD: ariadne_contract_review
SCOPE_SHA: d158d26bfdda9fc7dc899a1d1a8a141c2da6610e
FINDINGS: [P3] [split-status] docs/spec/FINDING_REGISTER_V1.md:37-38, :163-164 vs :175-179, :189-193 with scripts/retire_review_claims.py:434-444, :471-487, :543-576 - named carries with an absent or different identifier are linked by no fold pass and refused by no gate: 18 named-carry/named-root status splits in this section with `--check` PASS (nabu [ledger-duplication] root @7622776 OPEN while four carries are CLOSED via nabu-8f774d0.md#L38, three bound in this delta as "reconciliation"; vulcan [robustness] record_lint_report.py and retire_review_claims.py copies OPEN against CLOSED roots; nabu lint-branch family of 9 carries moved restated→OPEN) - closure: carries linked by named sha, or each pair reconciled; a `--check` report; :37-38 amended; a fixture. [P3] [closure-regression] scripts/build_finding_register.py:1088-1092, :1109-1122, :1457-1475 vs docs/spec/FINDING_REGISTER_V1.md:61-63, :37-38 - `package_closed_claims` is excluded as ledger vocabulary although it is the finding's function (build_finding_register.py:1343), which reopens nabu@7622776 [verification-depth] (closed at 8966da2e via nabu-1a9f0aa.md#L28) while its carries stay CLOSED; 8 closures reopened register-wide with no change record - closure: code identifiers kept or an exact-claim binding, a test, a per-claim change record. [P4] [help-text] scripts/retire_review_claims.py:639-640, :656-657 vs docs/spec/FINDING_REGISTER_V1.md:84, :152-156 - help and comment still say "earliest claim" and fold-before-retire - closure: strings corrected. [P4] [quote-precision] scripts/build_finding_register.py:1540-1546 vs docs/spec/FINDING_REGISTER_V1.md:179-182 - an unbalanced `"` hides a real carry and a single-quoted foreign carry counts; 0 live - closure: balanced spans, single quotes decided, a test.
SUMMARY: Every 8966da2e finding is closed at its raised severity: identity is equality, folds resolve by the named sha (72/72 clean, my five predicate-precision re-statements re-pointed), retire-before-link no longer strips identifiers, `--check` has a split refusal, the contract states one order and one keying rule, all six P4s are closed with tests, and the 8d063f00 candidate is bound consistently by a records-only mint with every checker and 170/156 tests passing. The strict identifier rule leaves 18 named-carry/root status splits in this section that `--check` does not see, including a nabu ledger-duplication "reconciliation" that closed carries but not their named root. A new `package_*` ledger-word exclusion reopens a P3 its own lane had closed, with no change record. Both are conservative and leave P0–P2 untouched; nothing here assesses GA or release readiness.
