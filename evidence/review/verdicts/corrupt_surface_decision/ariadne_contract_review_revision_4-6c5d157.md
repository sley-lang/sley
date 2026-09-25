<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 6c5d157b717b070944a2832395b8be42ec988b0e; role: ariadne; field: ariadne_contract_review_revision_4; dispatched: 2026-09-23T04:42:37Z; duration_s: 304; process_exit_code: 0 -->
# Ariadne Council review — corrupt_surface_decision

Harness: claude-code
Reviewed checkpoint: 6c5d157b717b070944a2832395b8be42ec988b0e

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `6c5d157b717b070944a2832395b8be42ec988b0e`, which matches. `git status --short` printed nothing. I did not modify the tree.
- **Delta.** I ran `git diff --stat 5d8106f3..HEAD` and `git log --oneline 5d8106f3..HEAD`. There are four commits (db3da6fe, e964bedc, 387e9713, 6c5d157b) touching five files:
  - `bench/live/CORRUPT-SURFACE-DECISION.md`
  - `crates/sley-repo/src/exchange.rs` (test module only: +217 lines, one new `#[test]`, no non-test lines changed)
  - `docs/spec/REPOSITORY_EXCHANGE_V1.md`
  - `evidence/review/rounds/corrupt-r3-5d8106f.json`
  - the revision-3 transcript
- **Full diff read.** I read `git diff 5d8106f3..HEAD` for all five files.
- **Round-index hash.** `sha256sum` of `ariadne_contract_review_revision_3-5d8106f.md` is `c6d5c70c…df89f`. This equals the `sha256` recorded in `corrupt-r3-5d8106f.json`. The verdict string and path recorded there also match the transcript.
- **New unit test.** `cargo test -p sley-repo --lib preflight_precedence` compiled `sley-repo` fresh ("Compiling sley-repo … Finished"). It exited 0: `test exchange::tests::preflight_precedence_early_exits_match_the_import_phase_text ... ok`, `1 passed; 0 failed; … 420 filtered out`.
- **Resealed pin.** `cargo test -p sley-repo --test s3_g2_corrupt -- --list` now lists 5 tests, including `s3_corrupt_exchange_resealed_embedded_pack`. The stale-binary limitation from revision 3 is gone. `cargo test -p sley-repo --test s3_g2_corrupt s3_corrupt_exchange_resealed_embedded_pack` exited 0: `1 passed; 0 failed`.
- **Spec checker.** `python3 scripts/check_repository_exchange_spec.py && echo CHECKER_EXIT_0` printed `"problems": []`, `"result": "PASS"`, `"status": "S20_540_COMPLETE"` and `CHECKER_EXIT_0`.
- **Files read, with line ranges:**
  - `evidence/review/verdicts/corrupt_surface_decision/ariadne_contract_review_revision_3-5d8106f.md` (whole file)
  - `docs/spec/REPOSITORY_EXCHANGE_V1.md:1-30`, `:90-300`, `:352-451`, plus the diffed note `:492-570`
  - `crates/sley-repo/src/exchange.rs:553-562`, `:640-945`, `:1098-1442`, `:1885-1893`, `:2659-2790`, `:2915-3172`
  - `crates/sley-repo/src/lib.rs:1089-1100`, `:138`
  - `crates/sley-txn/src/repository.rs:2500-2620`
  - `crates/sley-id/src/lib.rs:175-179`, `:453-467`
  - `bench/live/CORRUPT-SURFACE-DECISION.md:1-28`
  - grep of `conformance/repository-exchange/v1/rejected.json` for `RESOURCE_LIMIT` (no hits)
  - grep of existing verdicts for PASS-with-open-P3 precedent

## Evidence checked

1. **Prior P2 (precedence contradicted by early exits): CLOSED.**
   - The rewritten precedence text (spec `:356-376`) now names both reachable exits I raised:
     - An empty receipt set is `EXCHANGE_ANCESTRY_OPEN` in step 2 (`:370-371`, `:387-388`). This matches `exchange.rs:813-816` inside `decode_payload`, which runs before `:1381`.
     - Rule 6 for origin and ref records is decided per branch in step 5 (`:375-376`, `:434-442`). This matches `verify_branch_entry` `:1250-1252`, iterated at `:1415-1419`.
   - The tree-record profile and count failures are stated in step 2 (`:372-373`, `:391-395`). This matches `decode_tree` `:760-764` and `:775-777`, and `decode_payload` `:820-822`.
   - Pairwise pins at `exchange.rs:2962-3170` (test executed green):
     - Empty receipts over `EXCHANGE_PACK_INVALID`, using the nested exchange as control.
     - Algorithm tag 2 over `EXCHANGE_PACK_INVALID`.
     - An earlier binding-broken `aa` branch plus a later foreign-workspace `bb` branch gives `EXCHANGE_BRANCH_INVALID`. Both single-defect controls are asserted: `bb` alone gives `EXCHANGE_WORKSPACE_MISMATCH`, and `aa` alone gives `EXCHANGE_BRANCH_INVALID`.
   - The branch pin discriminates. `aa` sorts before `bb`: same length, then lower bytes, per spec `:125-134`.
2. **Prior P3 (step 3.3 overstatement and note sentence): CLOSED.**
   - Spec `:407-417` now says step 3.3 recomputes leaves:
     - section 1 from the step-3.2 `RepositoryPackId`;
     - sections 2 and 4 from the declared ids;
     - section 3 from recomputed name keys, with an ungrammatical name giving `EXCHANGE_BRANCH_INVALID`.
   - It then compares the leaf list and the root. This matches `exchange.rs:1386-1397`, `compute_leaves` `:864-905` and `branch_name_key` `:858-861`.
   - It states that declared receipt and head identities are verified in step 4. This matches `:1399-1406` and `:1168-1172`.
   - The note's "What changed" bullets (spec diff) no longer say "moves them into step 3.3". They move only the tree-content check, and keep the declared identities in step 4 (4.1 and 4.4).
   - Pin: the unnamed-first-branch row gives `EXCHANGE_BRANCH_INVALID` over `EXCHANGE_HEAD_INVALID`. It also beats a stale-leaf `EXCHANGE_DIGEST_TREE_MISMATCH`, because the branch element changed while the leaves were not recomputed.
3. **Prior P4 (packet header lines 16 and 18): CLOSED.**
   - `CORRUPT-SURFACE-DECISION.md:16-17` now reads "the ruling required the spec to be amended … and spec revision 9 does so".
   - `:19-22` now says the revision-2 review "raised" the P3 and P4, that this revision addresses them, and that closure was verified in revision 3. That is accurate: revision 3 did verify both CLOSED.
4. **End-to-end walk of the step text against `preflight` (`exchange.rs:1358-1430`):**

   | Spec step | Code | Status |
   |---|---|---|
   | 1 (`:378-381`) | `decode_envelope` `:646-677`: size bound, magic, version, tag, epoch read, sized payload, trailer, no trailing bytes, trailer vs derive; then the epoch check `:1360-1364` | Match |
   | 2.1 | `:786-812`: closed record, version, pack size, compression, signature | Match |
   | 2.2 | `:813-816` | Match (see F3 on element interleave) |
   | 2.3 | `:817-818` | Match (see F3 on element interleave) |
   | 2.4 | `decode_tree` `:760-777`, then `:820-822` | Match |
   | 2.5 | `:823-837` | Match |
   | 2.6 | `:1366-1379` | Match |
   | 3.1-3.3 | `:1381-1397` | Match |
   | 4.1 | `:1399-1408` | Match |
   | 4.2 | `:1137-1162`: per receipt, parents then kind/shape (`EXCHANGE_ANCESTRY_OPEN`), then workspace (`EXCHANGE_WORKSPACE_MISMATCH`). Iteration is `BTreeMap` order, and `TransactionId` derives `Ord` over `[u8; 32]` (`sley-id/src/lib.rs:177-179`), so it equals canonical order | Match |
   | 4.3 | `:1163-1166` | Match |
   | 4.4 | `:1168-1172` | Match |
   | 4.5 | `:1174-1199` | Match |
   | 5.1 | `verify_branch_entry` `:1235-1270`, in the listed bullet order | Match |
   | 5.2 | `:1420` | Match |
   | 6 | `:1421-1422`, then `:1325-1353`: cumulative bounds, then `verify_receipt_against_objects` | Match on per-receipt structure (see F1 on which topological order) |

5. **The new out-of-order statements the author added are correct:**
   - Rule 5 runs before rule 3 in step 4 (`:1168` before `:1174`). Pinned by the narrow-pack rows: foreign head gives `EXCHANGE_HEAD_INVALID` over `EXCHANGE_ROOT_CLOSURE`, with root-closure-alone as control.
   - The per-receipt workspace check runs with rule 1, ahead of the genesis count (`:1157-1164`). This is shown by the existing `every_reachable_exchange_code_has_an_asserting_rejection` "mixed" row at `exchange.rs:2929-2952`: two geneses in different workspaces give `EXCHANGE_WORKSPACE_MISMATCH`, not `EXCHANGE_ANCESTRY_OPEN`.
   - Rule 1 runs before rule 5. Pinned by open ancestry plus foreign head giving `EXCHANGE_ANCESTRY_OPEN`.
6. **Pin coverage.** The step-2 leaf-count failures (as opposed to the algorithm tag) are not pinned pairwise against step 3.1. Reading the code settles the order: both run inside `decode_payload`, before `:1381`. I raise no finding.
7. **Packet §5 answers (`:383-406`).** The claims about the test rows match the test source row for row. The test row and the spec's "Current pins" list agree.

## Findings

[P3] [spec-accuracy] docs/spec/REPOSITORY_EXCHANGE_V1.md:447-450 (with :356-359) - Step 6 says "per receipt in topological order", and the precedence paragraph claims an exact realized order. But the code uses one specific topological order that the text does not state: `topological_order` makes repeated passes in ascending `TransactionId` order, and a receipt is placed as soon as its parents are placed, including by an earlier placement in the same pass (exchange.rs:1098-1127, used at :1421-1422 and :1325-1353). Failure scenario: genesis G (id 0x50…) has two children, A (0x10…) and B (0x90…); B is a visible branch head, so there is no surplus. A and B each carry a different step-6 defect, for example two different parent-relationship `TXN_*` codes from `verify_receipt_against_objects` (sley-txn/src/repository.rs:2520-2533), or a cumulative bound that trips on whichever receipt comes second. Code order is G, B, A, so the code returns B's failure. An equally valid smallest-ready-id topological order (G, A, B) returns A's failure. The spec's "exact" promise does not determine the code. (Constructed by reading the code; not executed.) - Closure evidence: state the tie-break in step 6 (the pass order above), or define "topological order" once; optionally pin two sibling step-6 defects in a test.

[P3] [spec-conformance] docs/spec/REPOSITORY_EXCHANGE_V1.md:243-245,277-279,386-395 - Pre-existing and outside this delta, but inside the end-to-end walk the task requested. The spec says import "returns `EXCHANGE_RESOURCE_LIMIT` for every exchange-level bound". The receipt (4,096), branch (4,096) and leaf (8,194) list ceilings are enforced by `decode_list` as `PackError::pack(PackErrorCode::ResourceLimit)` (crates/sley-repo/src/lib.rs:1093-1094). That error is passed through unmapped by `pack_to_exchange` (exchange.rs:559-561, :682, :731, :767-771), and `ExchangeError::code` returns its symbol `PACK_RESOURCE_LIMIT` (exchange.rs:276; lib.rs:138). Step 2's rewritten check list does not name these list-count bounds at all. The existing test covers only the export side (`build_exchange`, exchange.rs:2682-2701), and `rejected.json` has no `RESOURCE_LIMIT` row. Failure scenario: import of an exchange whose receipt list declares 4,097 entries returns `PACK_RESOURCE_LIMIT`; the spec requires `EXCHANGE_RESOURCE_LIMIT`. - Closure evidence: one of two routes, then an import-side test asserting the chosen code for a 4,097-receipt list. Route 1 is a code fix plus that test: map exchange-level `decode_list` bounds to `EXCHANGE_RESOURCE_LIMIT`. Route 2 is a separate owner ruling plus a spec amendment in which step 2 names the list-count bounds and their realized code. Route this item to the exchange contract owner; it does not affect the CORRUPT precedence decision.

[P4] [spec-precision] docs/spec/REPOSITORY_EXCHANGE_V1.md:386-390 - Steps 2.2 and 2.3 describe the receipt and branch lists with the same wording, but the code interleaves differently. For receipts, every element's record is decoded before any order check (exchange.rs:684-711). For branches, each element's order check runs before that element's record decode (exchange.rs:734-748). Failure scenario: an out-of-order pair followed by a malformed element gives the SCB decode code for receipts, but `EXCHANGE_CANONICAL_ORDER` for branches. The text does not say which. - Closure evidence: one sentence per list stating the element decode vs order interleave.

[P4] [record-completeness] bench/live/CORRUPT-SURFACE-DECISION.md:23-24,383-406 - The header and the "Revision-3 review answers" list answer only the revision-3 P2 and P3. The revision-3 P4 (the packet header wording) was fixed at `:16-22`, but the packet does not record that answer. - Closure evidence: add a one-line "[P4] packet header wording: addressed at lines 16-22" entry to §5.

## Assessment

- **Prior findings.** All three of my revision-3 findings are verified CLOSED:
  - **P2:** the precedence text now names every out-of-order exit I raised, plus the ones the author found. Each one matches `exchange.rs`, and the pairwise unit test executes green.
  - **P3:** step 3.3 and the note are reworded to match `compute_leaves` and the step-4 identity checks.
  - **P4:** the packet header now attributes the amendment and the closure correctly.
- **Step text vs code.** I walked `preflight` end to end against steps 1 through 6. Every listed check, its code and its relative position match the code, including the step-4 rule 1 → 5 → 3 order and the per-receipt workspace check.
- **Resealed pin.** It now executes green at this scope, which removes the revision-3 environment caveat.
- **Remaining gaps.** None contradicts a stated order:
  - an unstated step-6 tie-break among topological orders (P3);
  - a pre-existing code-identity mismatch for exchange list-count ceilings, outside the delta and to be routed separately (P3);
  - a list-element interleave detail (P4);
  - a missing packet line recording the P4 answer (P4).
- **Verdict.** Council precedent in `evidence/review/verdicts` accepts PASS with open P3 and P4 findings. The CORRUPT packet is complete for the scope decision, and the frozen-vector deferral stays recorded as pending.
- No GA or release claim is made or implied.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_2_P4_PRIOR_P2_P3_P4_CLOSED
SECTION: corrupt_surface_decision
FIELD: ariadne_contract_review_revision_4
SCOPE_SHA: 6c5d157b717b070944a2832395b8be42ec988b0e
FINDINGS: [P3] [spec-accuracy] docs/spec/REPOSITORY_EXCHANGE_V1.md:447-450 (with :356-359) - step 6 "per receipt in topological order" leaves the tie-break unstated; the code's pass order (exchange.rs:1098-1127) makes siblings G→B(0x90)→A(0x10), so two sibling step-6 defects return B's code while another valid topological order returns A's - Closure evidence: state the tie-break or define the order; optionally pin it. | [P3] [spec-conformance] docs/spec/REPOSITORY_EXCHANGE_V1.md:243-245,277-279,386-395 - pre-existing, outside the delta: receipt/branch/leaf list ceilings return PACK_RESOURCE_LIMIT via decode_list (lib.rs:1093-1094; exchange.rs:682,731,767-771,276), whereas the spec requires EXCHANGE_RESOURCE_LIMIT for exchange-level bounds; step 2 omits these checks and there is no import-side test - Closure evidence: code mapping plus a 4,097-receipt import test, or an owner ruling plus a spec amendment naming the realized code. | [P4] [spec-precision] docs/spec/REPOSITORY_EXCHANGE_V1.md:386-390 - receipts decode all elements before the order check (exchange.rs:684-711); branches order-check each element before decoding it (exchange.rs:734-748); the text is silent on this - Closure evidence: state the interleave per list. | [P4] [record-completeness] bench/live/CORRUPT-SURFACE-DECISION.md:23-24,383-406 - the revision-3 P4 answer (header fixed at :16-22) is not recorded in the §5 answers - Closure evidence: add the one-line answer.
SUMMARY: All three revision-3 findings (P2 precedence, P3 step 3.3, P4 packet header) are verified CLOSED. The rewritten import-phase steps 1-6 match exchange.rs preflight check by check, and the new pairwise test and the resealed-pack pin both execute green on a fresh build; the spec checker exits 0 with PASS. Remaining items do not contradict any stated order: an unstated tie-break for the step-6 topological order, a pre-existing PACK_RESOURCE_LIMIT vs EXCHANGE_RESOURCE_LIMIT mismatch for exchange list ceilings (route to the contract owner), a list-interleave precision note, and a missing packet line for the P4 answer.
