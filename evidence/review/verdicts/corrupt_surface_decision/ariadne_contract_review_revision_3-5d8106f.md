<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 5d8106f37511b6f0f21f62273a16352c7a0c4703; role: ariadne; field: ariadne_contract_review_revision_3; dispatched: 2026-09-23T04:31:01Z; duration_s: 318; process_exit_code: 0 -->
# Ariadne Council review — corrupt_surface_decision

Harness: claude-code
Reviewed checkpoint: 5d8106f37511b6f0f21f62273a16352c7a0c4703

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `5d8106f37511b6f0f21f62273a16352c7a0c4703`, which matches. The tree was not modified.
- **Delta.** `git diff --stat 01dd20cb..HEAD` and `git log --oneline 01dd20cb..HEAD` show three commits (ccb5b532, 42e32430, 5d8106f3) touching five files:
  - `bench/live/CORRUPT-SURFACE-DECISION.md`
  - `bench/live/SUCCESSION-COVERAGE.md`
  - `docs/spec/REPOSITORY_EXCHANGE_V1.md`
  - `evidence/review/rounds/corrupt-r2-01dd20c.json`
  - the revision-2 transcript
- **No code or test change.** `git diff --quiet 01dd20cb..HEAD -- crates bench/live/tests bench/fixtures` exited 0 and printed `NO_CODE_OR_TEST_DELTA`.
- **Spec checker.** `python3 scripts/check_repository_exchange_spec.py` exited 0 (printed `CHECKER_EXIT_0`) with `"problems": []`, `"result": "PASS"` and `"status": "S20_540_COMPLETE"`.
- **Rust suite.** `cargo test -p sley-repo --test s3_g2_corrupt -- --nocapture` ran twice and exited 0 with `test result: ok. 1 passed; 0 failed; 3 ignored`. The binary cargo ran (`/home/gfarch/Work/checkpoints/sley2-review-target/debug/deps/s3_g2_corrupt-9539b2911aaa89b3`, reported `Finished` in 0.03s) lists only 4 tests. It does not contain `s3_corrupt_exchange_resealed_embedded_pack`, which is present in the source at `crates/sley-repo/tests/s3_g2_corrupt.rs:562-563`. So the preset shared CARGO_TARGET_DIR held a stale binary built from other source.
  - A rebuild into a separate `/tmp` target directory was refused by the sandbox (approval required), and `stat` on the target directory was blocked.
  - I therefore did **not** re-execute the resealed pin at this scope. I rely on three things instead: the test source is unchanged since 01dd20cb, I executed it green at 01dd20cb in my revision-2 review (2 passed), and the committed `bench/live/succ-trials-20260923/corrupt/rust_s3_g2_corrupt.log:16-19` shows `s3_corrupt_exchange_resealed_embedded_pack ... ok` and `2 passed` at git_head 72036a4e.
  - This is an environment limitation, not a finding against the tree.

Files read, with line ranges:
- `evidence/review/verdicts/corrupt_surface_decision/ariadne_contract_review_revision_2-01dd20c.md:1-170` (whole file)
- `git diff 01dd20cb..HEAD` for all five files
- `docs/spec/REPOSITORY_EXCHANGE_V1.md:1-30`, `:96-235`, `:340-470`, `:555-595`
- `crates/sley-repo/src/exchange.rs:640-945`, `:1095-1460`, `:1885-1900`, `:3243-3250`, `:3289-3292`
- `crates/sley-repo/src/lib.rs:482-520`
- `crates/sley-repo/tests/s3_g2_corrupt.rs:545-653`
- `scripts/check_repository_exchange_spec.py:40-119`
- `bench/live/CORRUPT-SURFACE-DECISION.md:1-30`
- a grep of `scripts/` for conformance, `rejected.json` and `SHA256SUMS` consumers
- a glob of `conformance/repository-exchange/v1/`

## Evidence checked

1. **Prior P3 (packet wording).** `CORRUPT-SURFACE-DECISION.md` §2 now reads: "One of them, `nested-exchange`, replaces the embedded pack with a whole tag-540 exchange (exchange.rs:3243-3250,3291). It pins the tag-170 shape check (`EXCHANGE_PACK_INVALID`, exchange.rs:1381-1383) ahead of the digest tree", and "No mutation alters embedded-pack content under a valid tag-170 header".
   - I re-read `exchange.rs:3243-3250` (`build_exchange(exchange.pack_id, exchange.stored_bytes.clone(), …)`) and `:3289-3292` (`("nested-exchange", nested.stored_bytes)`). The citations resolve.
   - This is the closure wording I asked for. **CLOSED.**

2. **Prior P4 (coverage label).** `SUCCESSION-COVERAGE.md:31` now opens "OBLIGATION ACCEPTED under owner ruling RULING_ACTOR: A …; agent-independent judge-side evidence, no agent-driven import claimed; agent-bound part = constant-restore smoke only", and labels the trial result separately as "Judge verdict (distinct from obligation acceptance): accepted".
   - Both ruling conditions are carried. The same file's prose entry was updated consistently (lines ~478-488).
   - **CLOSED.**

3. **Line-by-line check of the amended spec against `exchange.rs` `preflight` (`:1358-1430`).**

   | Spec (revision 9) | Code | Status |
   |---|---|---|
   | Step 1 (`:362-363`): bound bytes, envelope, payload, trailer | `decode_envelope` `:645-679`: size bound, magic, version, contract tag, trailer vs `RepositoryExchangeId::derive`; epoch check `:1360-1364`; `decode_payload` `:1365`; registry decode `:1366-1379` | Match |
   | Step 2 (`:364`): canonical order, counts, closed profiles | Inside `decode_payload` `:785-846`: version, pack-size bound, compression, signature profile, receipt order/duplicate `:700-711`, branch order `:733-743`, tree algorithm tag/count `:760-777`, leaf count `:820-822` | Match, but see F1(a) for `:813-816` |
   | "Steps 1 and 2 are one decode pass" (`:356-357`) | Payload decode and step-2 checks interleave inside `:1365`, then registry decode | Accurate |
   | Step 3.1 (`:366-367`): tag-170 check, `EXCHANGE_PACK_INVALID` | `:1381-1383`; `embedded_pack_header_is_tag_170` `:930-945` checks magic, version 1 and contract tag 170 | Match |
   | Step 3.2 (`:368-372`): complete S20-170 preflight, `PACK_*` preserved | `:1384` `preflight_conformance_pack` (`lib.rs:482-520`: envelope/trailer, payload, profile, registry, epochs, digest tree, roots, dependency and object closure, per-object preflight), errors propagated as `ExchangeError::Pack` | Match |
   | Step 3.2 example: `PACK_DIGEST_MISMATCH` under a valid exchange trailer | The pin asserts exactly that, target-free, at `s3_g2_corrupt.rs:600-601` | Match |
   | Step 3.3 (`:373-375`): tree keyed by the 3.2 `RepositoryPackId` | `:1386-1397`; `compute_leaves` `:864-905` uses `pack.pack_id` at `:880` | Match on keying; see F2 on "verify every declared leaf identity" |
   | Step 4 (`:376-377`): receipts, rules 1, 3, 5, 6 | Receipt decode and declared-id check `:1399-1408` (`EXCHANGE_RECEIPT_INVALID`); `verify_closure_rules` `:1130-1201` (rule 1 open/kind, receipt workspace, genesis count, cycle, rule 5 head `:1168-1172`, rule 3 root closure) | Match for receipts; see F1(b) for branch workspace |
   | Step 5 (`:378`): branches, rule 4 | `:1415-1419` `verify_branch_entry` `:1230-1276` | Match |
   | Rule 2 after step 5 (`:358-360`) | `verify_no_surplus` `:1420` | Match |
   | Step 6 (`:379-381`): receipts against pack objects | `topological_order` `:1421`, `verify_receipts_against_pack` `:1422` | Match |
   | Step 7 (`:382`): classify target | `import_repository_exchange` `:1891`, after `preflight` `:1890` | Match |

4. **Ruling on narrowing 1 (sub-steps 3.1-3.3 instead of a new top-level step): ACCEPTABLE.**
   - The checker hard-codes the marker `"re-run the complete step-7\n      classification"` (`check_repository_exchange_spec.py:92`), and row X-01 references step 8.1 (spec `:517`). A new top-level step would renumber both.
   - Sub-steps state the ordering my ruling asked for: tag-170 check, then S20-170 preflight, then tree.
   - The checker passes.

5. **Ruling on narrowing 2 (precedence stated as realized, not strict 1-7): ACCEPTABLE IN PRINCIPLE.**
   - My ruling's item 4 asked for strict 1-7 precedence, and that would itself have misstated the code: `verify_no_surplus` (`:1420`) runs after step-5 branch verification (`:1415-1419`).
   - Stating the realized groups is the correct response to `RULING_ORDER: CODE`.
   - However, the realized statement is incomplete for two reachable input classes (F1).

6. **Ruling on narrowing 3 (frozen `rejected.json` mutation deferred): ACCEPTABLE AS A RECORDED DEFERRAL.**
   - `conformance/repository-exchange/v1/` holds `SHA256SUMS`, `accepted.json` and `rejected.json`. The independent-conformance, GA, release-provenance and decision-dossier builders reference conformance paths (`build_independent_conformance_report.py:74,124`; `build_ga_acceptance_report.py`, `build_release_provenance.py` and `build_decision_dossier.py` each hit the conformance/`SHA256SUMS` grep). So re-freezing does move bound digests and warrants its own review.
   - The interim pin discriminates the order. `s3_g2_corrupt.rs:600-601` asserts `PACK_DIGEST_MISMATCH` from target-free preflight for a resealed object-byte flip. A tree-first order would instead return `EXCHANGE_DIGEST_TREE_MISMATCH`, because the section-1 leaf hashes the flipped pack bytes (`compute_leaves` `:880`) while the declared leaves are unchanged.
   - The deferral is recorded as pending in spec `:462-468`, in packet §5 and in the SUCCESSION-COVERAGE CORRUPT row and prose.
   - Caveat: I could not re-execute this pin at this scope (see above).

7. **Other records.**
   - The status paragraph (spec `:10-16`) accurately states revision 9 as a text-only amendment. No `crates/` change exists in the delta.
   - The round index `corrupt-r2-01dd20c.json` records the revision-2 verdict string, path and scope exactly as the transcript states.

## Findings

[P2] [spec-accuracy] docs/spec/REPOSITORY_EXCHANGE_V1.md:354-360 - The new normative precedence paragraph promises that import "returns the exact code of the first failing check" under the stated groups (steps 1-2 as one decode pass, then step 3, then 4-6, with only closure rule 2 moved after step 5). Two realized early exits contradict it. (a) `decode_payload` returns `EXCHANGE_ANCESTRY_OPEN` for an empty receipt set inside the step-1/2 decode pass (crates/sley-repo/src/exchange.rs:813-816), before step 3, while the spec assigns zero genesis only to closure rule 1 in step 4 (spec:158-165, :376-377). Failure scenario: a resealed exchange with zero receipts, one branch (leaf_count 3) and a nested exchange as `object_pack`; code returns `EXCHANGE_ANCESTRY_OPEN`, the spec's precedence yields `EXCHANGE_PACK_INVALID` at step 3.1. (b) Closure rule 6 for origin and ref records is proved in step 5 (`verify_branch_entry` exchange.rs:1250-1252, iterated per branch at :1415-1419), not in step 4 as spec:376-377 says; only rule 2 is named as moved. Failure scenario: branch 0 has an origin/ref binding failure and branch 1 has a foreign WorkspaceId; code returns `EXCHANGE_BRANCH_INVALID`, the spec's precedence yields `EXCHANGE_WORKSPACE_MISMATCH`. The digest-tree record's own profile and count failures (exchange.rs:760-777, :820-822) also return `EXCHANGE_DIGEST_TREE_MISMATCH` inside step 2, before step 3's "then the digest tree"; the text should name that too. - Closure evidence: amend the precedence paragraph (or steps 2/4/5) to state that the decode pass rejects an empty receipt set as `EXCHANGE_ANCESTRY_OPEN`, that tree-record profile/count failures are step-2 `EXCHANGE_DIGEST_TREE_MISMATCH`, and that rule 6 for origin/ref records is proved per branch in step 5; re-run the spec checker.

[P3] [spec-accuracy] docs/spec/REPOSITORY_EXCHANGE_V1.md:373-375,435-438 - Step 3.3 says "verify every declared leaf identity", and the amendment note says revision 9 "moves" revision 8's "every declared identity" into step 3.3. The code at 3.3 (exchange.rs:1386-1397, `compute_leaves` :864-905) only recomputes the leaf list from the declared identifiers: section 1 from the 3.2 pack id, section 3 name keys recomputed (an ungrammatical branch name returns `EXCHANGE_BRANCH_INVALID` here, :858-861, :893, ahead of steps 4-5), and sections 2 and 4 taken as declared. The declared receipt identities are verified against bytes in step 4 (:1399-1406, `EXCHANGE_RECEIPT_INVALID`), and the head's membership and receipt id in step 4 rule 5 (:1168-1172, `EXCHANGE_HEAD_INVALID`), after rule 1 (:1137-1166). Failure scenario: a head naming a transaction absent from the set plus two genesis receipts, with a consistent tree; code returns `EXCHANGE_ANCESTRY_OPEN`, while the 3.3 reading "verify every declared leaf identity" yields `EXCHANGE_HEAD_INVALID` at 3.3. - Closure evidence: reword 3.3 to "recompute every leaf from the declared identifiers (section 3 name keys recomputed, an ungrammatical name being `EXCHANGE_BRANCH_INVALID` here) and verify the leaf list and root"; state that declared receipt and head identities are verified in step 4; correct the note's "moves them into step 3.3" sentence.

[P4] [record-accuracy] bench/live/CORRUPT-SURFACE-DECISION.md:16,18 - The header says of the revision-2 review, "It also closed its own P3 (section 2 [P1]) and P4", and says "the spec was amended" as though the ruling amended it. The review raised those findings and did not close them. This revision's edits address them, and closure is decided by this review (verified CLOSED above). - Closure evidence: reword to "this revision addresses the review's P3 and P4 (closure verified in the revision-3 review)" and "the ruling required the spec to be amended; revision 9 does so".

## Assessment

- **Prior findings.** Both of my revision-2 findings are closed:
  - P3: the packet now names `nested-exchange` as pinning the tag-170 shape check ahead of the tree.
  - P4: the coverage row separates obligation acceptance under RULING_ACTOR A from the judge verdict, and carries both ruling conditions.
- **Central amendment.** The central ruling-order amendment is correct: step 3 now states the realized order tag-170 check → S20-170 preflight (`PACK_*` preserved) → digest tree keyed by the 3.2 `RepositoryPackId`, exactly as `exchange.rs:1381-1397` runs. The spec checker passes.
- **Narrowings.** All three narrowings are acceptable as reasoned: sub-steps preserve checker and X-row numbering, a strict 1-7 order would have misstated the code, and the frozen-vector deferral is justified by digest bindings, recorded as pending, and covered in the interim by a discriminating Rust pin.
- **Why REVISE.** The review criterion is that the spec states the code order accurately. The newly introduced precedence paragraph is normative and makes a "first failing check" promise. Two reachable inputs contradict it (F1). The step-3.3 identity wording also overstates what 3.3 checks (F2). These are text-only fixes; no code change is indicated, consistent with RULING_ORDER: CODE.
- **Limitation.** The shared review target served a stale `s3_g2_corrupt` binary, and a fresh build was not permitted in this sandbox. The resealed pin's execution at this scope rests on unchanged source plus prior executed and logged evidence.
- No GA or release claim is made or implied.

VERDICT: REVISE_0_P0_0_P1_1_P2_1_P3_1_P4_PRIOR_P3_P4_CLOSED
SECTION: corrupt_surface_decision
FIELD: ariadne_contract_review_revision_3
SCOPE_SHA: 5d8106f37511b6f0f21f62273a16352c7a0c4703
FINDINGS: [P2] [spec-accuracy] docs/spec/REPOSITORY_EXCHANGE_V1.md:354-360 - The new precedence paragraph promises "the exact code of the first failing check", but (a) the decode pass returns EXCHANGE_ANCESTRY_OPEN for an empty receipt set before step 3 (exchange.rs:813-816; spec assigns it to rule 1, step 4, :158-165, :376-377), so zero receipts plus a nested-exchange object_pack gives ANCESTRY_OPEN in code and PACK_INVALID by the spec; and (b) rule 6 for origin/ref records is proved per branch in step 5 (exchange.rs:1250-1252, :1415-1419), so an earlier BRANCH_INVALID branch plus a later foreign-workspace branch gives BRANCH_INVALID in code and WORKSPACE_MISMATCH by the spec; step-2 tree-record failures (exchange.rs:760-777, :820-822) also go unnamed - Closure evidence: amend the paragraph/steps to state these realized exits and re-run the spec checker. | [P3] [spec-accuracy] docs/spec/REPOSITORY_EXCHANGE_V1.md:373-375,435-438 - Step 3.3 "verify every declared leaf identity" and the note's "moves them into step 3.3" overstate the code: 3.3 recomputes leaves from declared identifiers (an ungrammatical branch name is BRANCH_INVALID here, exchange.rs:858-861, :893), while receipt and head declared identities are verified in step 4 (exchange.rs:1399-1406 RECEIPT_INVALID; :1168-1172 HEAD_INVALID after rule 1) - Closure evidence: reword 3.3 and the note accordingly. | [P4] [record-accuracy] bench/live/CORRUPT-SURFACE-DECISION.md:16,18 - The header says the revision-2 review "closed its own P3 and P4" and that "the spec was amended" by the ruling; the review raised the findings and required the amendment - Closure evidence: reword to say this revision addresses them and revision 9 performs the required amendment.
SUMMARY: My revision-2 P3 (nested-exchange wording) and P4 (coverage-row label) are verified CLOSED, and revision 9's step 3.1-3.3 text matches exchange.rs:1381-1397 (tag-170 check, then S20-170 preflight with PACK codes preserved, then the digest tree keyed by the verified pack id); the spec checker exits 0 with PASS. All three stated narrowings are acceptable as reasoned: sub-steps preserve checker and X-row numbering, a strict 1-7 order would misstate the code, and the frozen-vector deferral is digest-justified and pinned by s3_g2_corrupt.rs (I could not re-execute that pin here because the shared target served a stale binary). REVISE because the new normative precedence paragraph is contradicted by two reachable realized exits (empty-receipt ANCESTRY_OPEN in the decode pass; branch-record workspace checks in step 5), and step 3.3 overstates the declared-identity checks; all fixes are text-only.
