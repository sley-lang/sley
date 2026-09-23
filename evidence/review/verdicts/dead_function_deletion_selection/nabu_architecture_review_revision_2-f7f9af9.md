<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f7f9af906dc0e585975cbc7addee75999889a3d2; role: nabu; field: nabu_architecture_review_revision_2; dispatched: 2026-09-23T04:14:58Z; duration_s: 830; process_exit_code: 0 -->
# Nabu Council review — dead_function_deletion_selection

Harness: claude-code
Reviewed checkpoint: f7f9af906dc0e585975cbc7addee75999889a3d2

What I ran and read myself:
- **Scope check.** `git rev-parse HEAD` returned `f7f9af906dc0e585975cbc7addee75999889a3d2`, so the scope matches. `git status --short` was empty at the start and at the end of the review.
- **Delta.** `git diff --stat 883361e3..HEAD` shows 8 files. I read the full diff for every non-transcript file:
  - `candidate_validation.rs`: doc comment only, +8/−6
  - `CANDIDATE_RESULT_V1.md`: +10/−3
  - `decision-dossier.json`: 3 lines
  - `REQ-11...md`: +36, Amendment 1
  - `rounds/req11-883361e.json`: new
  - `test-inventory.json`: 3 lines
  - the two archived 883361e transcripts
- **Whitespace.** `git diff --check 883361e3..HEAD` exited 0.
- **Round record binding.** `sha256sum` of the two 883361e transcripts gives `6d834fbd…4ba1` (Ariadne) and `f4fbda05…da39` (Nabu). Both match `evidence/review/rounds/req11-883361e.json`.
- **Required checkers:**
  - `python3 scripts/build_test_inventory.py --check`: exit 0, `"result": "PASS"`, `rust_unit_tests: 1965`.
  - `python3 scripts/check_decision_dossier.py`: exit 0, `"problems": []`, `"result": "PASS"`.
- **Every Python line of the Makefile `quick` target** (Makefile:4-134, 127 commands), run once at HEAD: 12 exited non-zero.
  - 8 persistent-fuzz slice checkers, each failing with `proof-record-predates-lane-change`: merge, semantic_delta, smp1, smp1_json_bridge, pack, adapter_responses, exchange, merge_judgment.
  - `check_reproducibility_and_independent_conformance.py`: `reproducibility-report:stale:69907ddf904a:2-surface-files-changed:crates/sley-policy/src/candidate_validation.rs,crates/sley-policy/src/native_test_plan.rs`.
  - `check_supply_chain_audit.py`: drift in `evidence/security/T54/secret-scan.json`.
  - `check_standards_sbom_and_provenance.py`: `closure:unverifiable`, `sbom:drift`, `provenance:drift`.
  - `build_candidate_content_report.py --check`: the untracked `evidence/runtime/s20-720-release-candidate/evidence.json` is missing.
- **Control at the parent main-line tip `acbc65f0`.** I made a `git clone --shared` in /tmp; nothing was written into the reviewed repository. I re-ran the 12 failing checkers there, plus the dossier and inventory checks.
  - All 8 fuzz checkers, the reproducibility check, the supply-chain audit, the dossier check and the inventory check exit 0 at `acbc65f0`.
  - `check_standards_sbom_and_provenance.py` fails there with the same three problems, so it is environmental and predates this change. The candidate-content failure is also environmental: the runtime file is untracked.
  - An earlier control I tried, on a synthetic single-commit history, was invalid because these checkers walk real ancestry. I discarded its results.
  - The /tmp clone could not be removed, because the sandbox blocks `rm` outside the worktree. It lies outside the reviewed tree.
- **Pattern history:**
  - `git log -- crates fuzz` and `git log 7995e057~1..acbc65f0`: source-changing fix commits (for example `7995e057`, which touches `fuzz/targets/vm_canonical_inputs.rs`) are followed by records-only mint commits (`6d752779`→`1a9f0aab`, `f3d4f450`→`6589c6ec`, `7527d82a`→`79fdcc63`, `69907ddf`→`8966da2e`).
  - `git show --stat 8966da2e`: that mint rebinds fuzz/T54/reproducibility/SBOM/provenance/dossier/GA records only and states `make quick exit 0`.
- `git diff --stat 8966da2e..883361e3 -- crates fuzz Cargo.*`: the only source surfaces changed since the last mint are the two sley-policy files.
- **Cargo.** `cargo test -p sley-policy --lib function_deletion_tests`: exit 0, `test result: ok. 7 passed; 0 failed; 0 ignored; ... 81 filtered out`.
- **Stale-digest search.** `grep` of `ga-acceptance-report.json` and `machine-summary.json` for the old or new dossier digests and for 1958/1965 found no stale references. `check_decision_dossier.py`, which consumes the GA report, passes.
- **Latent-clause search.** Searched the repo, excluding verdicts, for `affected\.contains\(entity\)|latent clause|REQ-11`. The only hits are the code (`native_test_plan.rs:306`), the packet (lines 67 and 119) and the spec note (line 321). No finding-register entry exists for the latent clause.
- **Files read:**
  - prior verdict `nabu_architecture_review-883361e.md` (full)
  - `docs/spec/CANDIDATE_RESULT_V1.md` 1-14 and the phase-10 hunk (311-326)
  - `Makefile` 1-142
  - `REQ-11...md` Amendment 1 (lines 87-121, via the diff)
  - `candidate_validation.rs` 1736-1751 (via the diff)

## Evidence checked

**Prior P1 (test inventory / quick gate).**
- `test-inventory.json` now records sley-policy `tests: 93` and `rust_unit_tests: 1965`, with a new `inventory_digest`.
- The dossier entry is updated to 1965 and both dossier digests are recomputed.
- Both named checkers exit 0 at HEAD.
- The stated defect, inventory drift that turned the dossier check red, is fixed in the same commit, following the acbc65f0 practice.

**Remaining red quick lines, attributed.**
- The control proves that exactly these are red because of the source change: 8 fuzz proof records, the S20-730 reproducibility report and the T54 secret scan.
- These are precisely the records Amendment 1 lists as candidate-bound. The dossier/GA digests that follow them are currently consistent.
- SBOM/provenance and the candidate-content report fail identically at `acbc65f0`. They are environmental (untracked runtime evidence) and not attributable to this change.
- None of these records can be honestly regenerated without minting a candidate from a source tree that contains the change. That mint is what the records-only mint commits do.

**Restated make-quick criterion.** It is acceptable architecturally.
- These records are bound to a candidate identity: each fuzz proof names the source commit its lanes ran at, and the reproducibility report attests a specific minted artifact.
- Requiring them green at the fix commit itself would force either a circular self-reference or a mint of a non-landing commit.
- The repo has used the fix → records-only mint → `make quick` exit 0 at the landing tip sequence for at least five consecutive source-changing rounds. `8966da2e`'s message records the re-run fuzz proofs (not just re-hashed digests), two clean builds, and `make quick` exit 0.
- The authority boundary is intact. Review judges the source, and the mint judges the records. The candidate-bound checkers stay fail-closed: they are red at HEAD and would block a landing tip that lacked the mint.
- What I require before landing is stated as a landing condition in the Assessment. It is a P4 note, because the amendment already commits to it and it cannot be demonstrated at this SHA.

**Prior P3 (spec identity).** `CANDIDATE_RESULT_V1.md:321-326` now carries `(Amended 2026-09-23, REQ-11: …)`. The note says:
- the prior refusal (phase 11, `TEST_PLAN_SELECTION_INVALID`)
- that no frozen vector pinned it
- that the only movement is refuse-to-valid
- why `full_validation_profile_id` is unchanged

This matches the dated-correction convention at lines 5-8, and the note agrees with the code and with my prior analysis.

**Prior P4 #1 (claim precision).** Both corrections are accurate:
- The doc comment (`candidate_validation.rs:1739-1746`) now attributes the required-test refusal to the checker's required-test resolution at phase 11. It calls the kept-identity branch defensive (apply refuses a kind change; a reached one refuses at phase 11).
- The spec text says the same, naming `TargetKindMismatch` and `TEST_PLAN_SELECTION_INVALID`, and the packet supersedes "(WrongEntityKind)".

This is a comment-only Rust change, and the 7 tests still pass.

**Prior P4 #2 (dead clause / duplicated closure authority).**
- `native_test_plan.rs:306` is unchanged, which is what I asked for: not folded into this change.
- The deferral lives only as one packet sentence (REQ-11:119-121). The finding register has no entry or follow-up record. My closure evidence required "a separately recorded change", and that does not exist yet.
- It stays OPEN at P4 and does not block this change.

**Prior P4 #3 (traceability).**
- Amendment 1 states the deviation from `DEAD-TOMBSTONE-PROPOSAL.md`: caller-side projection instead of a checker-side tombstone index, with the S20-240 checker untouched.
- It explicitly does not claim the frozen DEAD benchmark positive is unblocked, and requires a succession-arm rerun first.

That satisfies the closure evidence, whose second half was a condition on future claims rather than on this change.

**Other checks.**
- The round record is correctly bound: paths, scope and sha256 match the archived transcripts.
- The historical transcripts are preserved verbatim. They match the hashes I computed; I did not edit or reissue them.
- No code behaviour changed in the delta.

## Findings
[P4] [dead-code-duplicated-authority] crates/sley-policy/src/native_test_plan.rs:286,306,577-597 - Carried forward from the prior round, still OPEN. `affected.contains(entity)` compares TestCase ids against Function ids and never fires, and the local `affected_functions` duplicates the validator's closure derivation. The deferral is recorded only as a sentence in REQ-11:119-121; there is no finding-register entry or follow-up record. Output is unaffected - closure evidence needed: a finding-register entry or separate request for the follow-up, then a separate change that deletes the clause and its duplicate derivation (preferred), with a spec note that NATIVE_TEST_ADMISSION §2 item 2 is met through item 1.
[P4] [landing-condition] evidence/review/requests/REQ-11-dead-function-deletion-selection.md:89-98 - The restated criterion is accepted, but it cannot be shown at this SHA. At HEAD, 10 quick lines are red because of the change itself (8 fuzz proof records `proof-record-predates-lane-change`, the reproducibility report `stale:69907ddf904a:2-surface-files-changed`, and T54 drift); all 10 are green at acbc65f0 - closure evidence needed: the main line gains f7f9af90 only together with a records-only mint commit descended from it. That commit touches no crates/, fuzz/, scripts/, docs/spec/ or Cargo files; re-runs every persistent-fuzz proof in the clean mint worktree; and ships a transcript of full `make quick` (including `cargo test --workspace --locked`) exiting 0 at the landing tip. The main line must never rest at f7f9af90 without that mint.

## Assessment
The delta addresses the prior round precisely and changes no behaviour.

Prior finding status:
- Prior P1 [gate-evidence-binding] test-inventory drift: CLOSED. The inventory is regenerated, the dossier is rebound, and `build_test_inventory.py --check` and `check_decision_dossier.py` both exit 0. The make-quick part is carried as the landing condition above.
- Prior P3 [spec-identity] undated phase-10 amendment: CLOSED. There is a dated REQ-11 note at `CANDIDATE_RESULT_V1.md:321-326`, with the refuse-to-valid and profile-id rationale.
- Prior P4 [claim-precision] policy/WrongEntityKind wording: CLOSED. The doc comment, spec and packet are corrected and accurate.
- Prior P4 [dead-code-duplicated-authority] native_test_plan.rs:306 clause: OPEN. It is correctly kept out of this change, but only a packet sentence records it. It is carried at P4 and does not block.
- Prior P4 [traceability] proposal deviation and DEAD rerun: CLOSED. The deviation is stated, and the DEAD positive is explicitly not claimed.

The make-quick restatement is sound. My control run proves that the remaining red lines are exactly the candidate-bound records the amendment names. Those records can only be regenerated by a mint of source containing the change. The fix → records-only mint pattern is established in this repo, and it keeps the candidate-bound checkers fail-closed.

The change may land on the main line, subject only to the landing condition: land it together with a records-only mint, and show `make quick` exit 0 at the landing tip.

Nothing here addresses GA or release readiness. Those are out of scope.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_2_P4
SECTION: dead_function_deletion_selection
FIELD: nabu_architecture_review_revision_2
SCOPE_SHA: f7f9af906dc0e585975cbc7addee75999889a3d2
FINDINGS: [P4] [dead-code-duplicated-authority] crates/sley-policy/src/native_test_plan.rs:286,306,577-597 - carried forward and still OPEN: affected.contains(entity) never fires, and affected_functions duplicates the validator closure; the deferral lives only in REQ-11:119-121 with no finding-register or follow-up record - record the follow-up, then remove the clause and its duplicate derivation in a separate change. | [P4] [landing-condition] evidence/review/requests/REQ-11-dead-function-deletion-selection.md:89-98 - restated criterion accepted; 10 quick lines (8 fuzz proofs, S20-730 reproducibility, T54) are red because of the change at HEAD and green at acbc65f0 - land only with a descendant records-only mint (no crates/fuzz/scripts/docs/spec/Cargo changes, fuzz proofs re-run) and a full make quick exit-0 transcript at the landing tip.
SUMMARY: The delta closes prior P1, P3 and two of the three P4s, verified by checker exits (build_test_inventory --check 0, check_decision_dossier 0), the dated spec note, corrected wording, a stated design deviation and 7/7 passing regressions. A control run at acbc65f0 shows that the only remaining red quick lines caused by the change are exactly the candidate-bound records the amendment names, so the fix-then-records-only-mint landing pattern is architecturally acceptable. The native-plan dead clause remains an open P4, recorded only in the packet. PASS is conditioned on landing together with the records-only mint and make quick exit 0 at the landing tip.
