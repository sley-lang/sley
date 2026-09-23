<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: nabu; field: nabu_architecture_review_revision_6; dispatched: 2026-09-23T13:54:20Z; duration_s: 730; process_exit_code: 0 -->
# Nabu Council review — complete_root_index_snapshot
Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

I checked everything below myself. I did not rely on the author's records or notes.

**Git commands**
- `git rev-parse HEAD` → `26d050e629b669ef4acbd54826ef4008c05a061d`, so the scope matches.
- `git log --oneline 2b0f1c9f..HEAD` → 12 commits. The S20-300 code is in 004cf5be and the contract, checker and summary are in 60be11c8.
- `git diff --stat 2b0f1c9f..HEAD` → 86 files, +4996/−537.
- Full diffs over this package's paths:
  - the spec (+66)
  - `crates/sley-repo/src/index_cache.rs` (+122)
  - the checker (+99) and its unit tests (+78)
  - the consumer in `crates/sley-protocol/src/server.rs` (+64)
  - the WORK_PACKAGES S20-300 row
- `git diff --name-status 26d050e6~1..26d050e6` → only `evidence/security/T54/secret-scan.json`.
- I compared the machine-summary section `complete_root_index_snapshot` field by field against `git show 2b0f1c9f:…`. `contract_revision` went 5→6. The three `_revision_5` verdict fields and notes were added; mine reads `PASS_0_P0_0_P1_0_P2_0_P3_4_P4_PRIOR_P3_P4_CLOSED`, which matches my transcript. The `status_note` was rewritten. There are no `_revision_6` fields.

**Checkers and tests**
- `python3 scripts/check_complete_root_index_snapshot_profile.py` → exit 0, `"result": "PASS"`, `"revision": 6`, `"status": "S20_300_FULL_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
- `python3 -m unittest scripts/test_complete_root_index_snapshot_profile.py -v` → `Ran 21 tests … OK`: 4 CompletionBinding, 9 ProbeGate and 8 TokenAwareGate cases.
- `cargo test -p sley-repo --lib index_cache` → `13 passed; 0 failed`. This includes `a_symlinked_cache_directory_is_never_read_or_written_through`, `a_non_canonical_repository_spelling_is_covered_by_its_guard` and `exchange::tests::an_incomplete_clone_resumes_without_adopting_the_index_cache_it_carried`.
- `cargo test -p sley-protocol --lib workspace_open` → `5 passed`. This includes `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record` and `workspace_open_answers_from_the_single_checked_head_load`.
- `cargo test -p sley-protocol --lib materialized` → `1 passed`.
- `python3 scripts/check_supply_chain_audit.py` → exit 0: `"result": "DEFERRED"` (release SBOM deferred), `"t52_local_lock_inventory": "PASS"`, `"t54_high_confidence_scan": "PASS"`.

**Scratch probes of `probe_gate_problems`**
These ran in memory against tempdir trees; nothing was written to the worktree.
- Refused as they should be:
  - my revision-5 case A, a second `self.materialized_head_snapshot` caller → `probe-wrapper-caller:…:references=3`;
  - case B, a blocking shared acquire → `probe-consumer:initializes-or-waits`;
  - case B2, a non-blocking exclusive acquire → refused;
  - case C, a `"http://x"` literal before a call → `probe-caller:…`;
  - case D, `server_tests.rs` naming the wrapper without `#[cfg(test)]` → refused.
- Admitted (`[]`), which they should not be:
  - N1: a nested block comment exposing `//`;
  - N2: a nested block comment exposing `"`;
  - N3: `let r#use = cached_complete_root_snapshot_id;` in server.rs;
  - N4: a wrapper call behind a nested comment;
  - B3: `Self::maintenance(self)` in the wrapper.
- `rustc --edition 2024` on a tempdir file with N1–N3 → exit 0. Running it printed `live calls: 6`, so all three are real calls.

**Files read**
- My prior transcript, in full.
- Gate record 784-964.
- Spec 112-291.
- index_cache.rs 100-399 and 728-827.
- Checker 24-95 and 130-339.
- Checker test file 1-110 plus its diff.
- server.rs 1150-1190, 2375-2400, 2466-2500 and 2913-2971.
- server_tests.rs 7540-7639.
- sley-txn maintenance.rs 1-70 and 100-210, and lib.rs 35-38.
- exchange.rs 1680-1702.
- ADR-0029 1-60.
- SMP1.md 752-771.
- SLEY2_TRIAL_RUNNER_V1.md 18-25 and 326-334.
- sley-protocol lib.rs 14-20.
- WORK_PACKAGES.md:35.

Not reviewed: the Ariadne and Vulcan revision-5 transcripts, and the other packages' round-7 deltas beyond the S20-300 consumer path.

## Evidence checked

**Status of my revision-5 findings**

- **[P4] checker-precision (callers of the `pub(crate)` wrapper not gated, a blocking acquire beside the non-waiting one, `//` inside a string literal) — CLOSED.**
  - The wrapper is now gated. `probe_gate_problems` requires exactly two wrapper references in server.rs, the definition plus one inside `fn workspace_open(` (checker :210-216).
  - Any other file naming it is refused. The only exemptions are crate integration tests and `server_tests.rs`, and the latter only while lib.rs declares it `#[cfg(test)]` (:152-159, lib.rs:17-18).
  - `BLOCKING` (:103-105) refuses blocking shared acquisition, both exclusive forms, `initialize_repository_maintenance` and `.maintenance()`. The underlying names are confirmed in maintenance.rs:107-146.
  - `strip_rust` blanks string, raw-string and char literals.
  - My exact cases A, B and C are refused now, and the unit suite has a negative test for each. The new residuals are recorded as finding 2.
- **[P4] spec-precision (coverage overstatement; exclusive-owner evidence placed at the wrong layer) — CLOSED.**
  - Spec :126-130 now says what `covers` does: relative, parent-symlink and `..` spellings are covered, and a symlink as the final component or another root is refused. This matches maintenance.rs:35-36 and 195-207.
  - The relative spelling is tested (index_cache.rs:765-778).
  - Spec :284-287 now puts exclusive-owner absence at the wrapper `materialized_head_snapshot` and says `workspace.open` waits at the head load. The discarded-record case is through `workspace.open`. This matches server_tests.rs:7564-7585.
- **[P4] fail-closed-structure (symlinked `index`/`index/v1` followed; Unix-only flags stated unconditionally) — CLOSED.**
  - `cache_directories_are_real` (index_cache.rs:160-173) lstat's both components.
  - It gates `read_record` (:323-325) and runs before and after `create_dir_all` in `write_record` (:181-191).
  - The test at :785-827 covers a symlinked `index/` on both the probe and the write-back, and a symlinked `index/v1/` on the write-back.
  - Spec :137-140 scopes `O_NOFOLLOW`/`O_NONBLOCK` to Unix.
  - This meets the "require real directories … with a test" option I named. Residual imprecisions are recorded as findings 3–5.
- **[P4] evidence-binding/out-of-section (T54 drift at the scope) — CLOSED.**
  - 26d050e6 is the final commit and touches only T54.
  - `check_supply_chain_audit.py` exits 0 at HEAD with `t54_high_confidence_scan: PASS`.
  - Gate §13.4 now names the commit it ran at (`4e9cdeda`). It records "PASS" where the top-level result is `DEFERRED`; the T52/T54 sub-results are PASS. This is a nit, not a finding.

**The rest of the revision-6 delta**

- **Identity discipline.** The cache path is derived from `guard.repository_root()` after `covers` at all three entry points (:248, :306, :378). The caller's spelling is never used for I/O after the check. This replaces the S20-390 check-then-use idiom with a stronger one.
- **Ownership and dependency direction are unchanged.**
  - `sley-repo` still owns the only cache.
  - The probe is still `covers`, then `read_record`, then `accept_cached(...).ok()`. There is no build, write, delete or lock, and one acceptance authority remains.
  - No `Cargo.toml` or `Cargo.lock` changes.
- **Consumer (server.rs:2963-2971).**
  - It is unchanged apart from pins.
  - The new retained-head plumbing (:1181-1205, :2936-2945) feeds `workspace_open` the head its session check loaded. The probe still runs only at `>= PROTOCOL_VERSION_V2`.
- **Completion stays unreachable.** The status is REVIEW_PENDING, `SPEC_REVISION = 6` and there are no `_revision_6` fields.

## Findings
[P3] [identity-discipline] docs/adr/ADR-0029-complete-root-index-snapshot-boundary.md:3-5,48-51; docs/WORK_PACKAGES.md:35; scripts/check_complete_root_index_snapshot_profile.py:46,60-69 - ADR-0029's Status line still says the full contract "is a draft at revision 1 with Council review pending". The S20-300 work-package row was edited in 60be11c8 to name contract revision 6 and "revision 5 reviews PASS x3", yet the same row still says "revision 5 review pending" and names "the SMP1 revision 13 `workspace.open`" as the consumer. That contradicts spec :3 and :206 (revision 6, SMP1 revision 15) and summary `contract_revision: 6`. ADR decision 6 says the checker "binds the contract, ADR, work-package row", but the ADR and work-package markers ignore revision, so the stale identity passes. This round rated the same defect class P3 for ADR-0034 and ADR-0035 - closure evidence: ADR-0029 Status names revision 6 with dated history; the row says revision 6 review pending and SMP1 revision 15; the checker anchors on the ADR's current-revision line and the row's revision and pin, with revert tests
[P4] [checker-precision] scripts/check_complete_root_index_snapshot_profile.py:102-105,109-127 - `strip_rust` ends a nested block comment at its first `*/` and claims this "can only expose text, never hide it" (:111 says "fail-closed"). In fact the exposed text can open a `//` comment or a string literal that blanks real code. Scratch cases N1 (`/* /* */ // */ sley_repo::cached_complete_root_snapshot_id(a, b, c);` in another crate), N2 (the same with an exposed `"`) and N4 (a wrapper call in server.rs behind the same comment) all return `[]`; rustc compiled and ran them as live calls. `USE_ITEM` also swallows `let r#use = cached_complete_root_snapshot_id;` (N3 → `[]`), and `BLOCKING` misses `Self::maintenance(self)` (B3 → `[]`), which contradicts spec :222-223. These are crafted inputs, not drift, and the tree has exactly one caller - closure evidence: depth-counting block-comment stripping (or narrow the docstring), a `use` match that excludes `r#use`, a path/UFCS-aware `maintenance` pattern, and negative tests for N1-N4 and B3
[P4] [fail-closed-structure] crates/sley-repo/src/index_cache.rs:160-173,379-391; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:134-137,185-188 - The audit API `verify_cached_snapshot` was not aligned with the new component rule. With a symlinked or non-directory `index/`, `read_record` returns `None`, and the fallback `fs::symlink_metadata(&path)` then follows the `index` link (or fails with ENOTDIR). So a link with nothing behind it, or a file at `index`, is reported as `Missing`, which spec :187 calls benign, although section 5 now treats it as tampering and every write-back is silently refused. A link with a record behind it is reported as `Mismatch`. The same tamper gets two audit answers - closure evidence: `Mismatch` whenever an existing cache directory component is not a real directory (or a spec sentence stating the audit's behaviour), with a test
[P4] [evidence-binding] crates/sley-repo/src/index_cache.rs:248,306,378,765-779,807,817-826; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:288-290; bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:920 - No test can catch a revert of the canonical-root derivation. Every covered spelling resolves to the same file, so going back to `index_cache_path(repository, …)` would leave the relative-spelling assertions green: they are a probe read plus `is_file` on the canonical path, and the file was written earlier through `through_link`. The record's "asserted in the spelling test" and the evidence line's "read and written under the guard's canonical root" both overclaim, since the relative spelling is only read. The `index/v1/` case probes after the outside record was deleted (:807), so its "never read through" half passes trivially - closure evidence: a mechanical anchor for `index_cache_path(guard.repository_root()` at all three sites (or record and evidence wording that rests on code inspection), and a valid record planted behind the `index/v1` link before the probe assertion
[P4] [spec-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:134-140; crates/sley-repo/src/index_cache.rs:181-191,316-333; crates/sley-repo/src/exchange.rs:1680-1690 - Three imprecisions in section 5. (1) "on every platform … a symlink … at the cache path is never followed": off Unix there is no `O_NOFOLLOW`, so the open follows a symlink and the handle reports the target as a regular file; the symlink tests are `cfg(unix)`. (2) The component rule is an lstat followed by a path-based open, create or rename, so a concurrent same-authority swap between them is not excluded, and section 5 does not name that window under its residual. (3) "the import purge refuses the same tamper" holds for `index/` only; a symlinked `index/v1/` is removed by `remove_dir_all`, not refused. Nothing leaks, because rules 1-4 still bind accepted bytes - closure evidence: the symlink clause scoped to Unix, the component check-then-use window named under the section 5 residual, and the purge parenthetical limited to `index/`
[P4] [out-of-section/spec-precision] crates/sley-protocol/src/server.rs:1181-1186,2375-2392,2469-2472,2932-2933; docs/spec/SMP1.md:763-765; docs/spec/SLEY2_TRIAL_RUNNER_V1.md:24-25,330-332 - Three texts say that an absent maintenance boundary "fails the method" before the probe: the round's new `workspace_open` docstring, S20-620 revision 7 section 9, and SMP1 appendix A. But `workspace.open`'s session check loads the head through `head_binding_mixed` → `head_mixed`, which calls `self.maintenance()`, and that runs `initialize_repository_maintenance` before the blocking shared acquisition. An absent boundary is therefore re-created and the probe then runs. I established this by reading the code and did not execute it. The S20-300 text (:210-213) is accurate - closure evidence: correct the three sentences (the head load initializes the boundary), or stop the head load from initializing; add a test that removes `locks/` before `workspace.open`

## Assessment
Revision 6 closes all four of this lane's revision-5 findings, and I checked each one against code, tests and checker output:
- the wrapper has one gated caller, and the gate is literal-aware and refuses blocking acquisition;
- the coverage and consumer-evidence sentences now match `covers` and the tests;
- both cache directory components must be real directories on read and write-back, with a test;
- T54 is regenerated last, and the supply-chain check exits 0 at HEAD.

The new canonical-root path derivation tightens identity discipline. Ownership, dependency direction and the single `accept_cached` acceptance authority are unchanged.

The six new findings are one P3 and five P4s:
- **P3:** stale revision identity in the ADR-0029 Status line and the WORK_PACKAGES row, which the checker's revision-agnostic markers let through.
- **P4:** the gate's comment stripping can be defeated by crafted nested comments, `r#use` and the UFCS form.
- **P4:** the audit reports a tampered directory component as a benign miss.
- **P4:** no test can catch a revert of the canonical-root derivation.
- **P4:** three section 5 sentences overstate (the non-Unix symlink clause, the unnamed component race, the purge parenthetical).
- **P4 (out of section):** an absent-boundary claim in SMP1, S20-620 and the server docstring that the code contradicts.

None of them affects correctness, fail-closed behaviour of accepted bytes, or what identity is disclosed. This lane accepts S20-300 at contract revision 6.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_5_P4_PRIOR_P3_P4_CLOSED
SECTION: complete_root_index_snapshot
FIELD: nabu_architecture_review_revision_6
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS:
[P3] [identity-discipline] docs/adr/ADR-0029-complete-root-index-snapshot-boundary.md:3-5,48-51; docs/WORK_PACKAGES.md:35; scripts/check_complete_root_index_snapshot_profile.py:46,60-69 - ADR-0029 Status still says "draft at revision 1"; the S20-300 row names revision 6 but still says "revision 5 review pending" and the "SMP1 revision 13" consumer; the checker's ADR and work-package markers ignore revision - closure evidence: the ADR and the row name revision 6 and SMP1 15, with checker anchors and revert tests
[P4] [checker-precision] scripts/check_complete_root_index_snapshot_profile.py:102-105,109-127 - nested block comments (N1, N2, N4), `r#use` (N3) and `Self::maintenance(self)` (B3) all pass the gate as `[]`, and rustc confirms the hidden calls are live; the "can only expose text, never hide it" claim is false - closure evidence: depth-counting stripping or a narrower claim, `r#use`-safe `use` matching, a UFCS-aware maintenance pattern, negative tests
[P4] [fail-closed-structure] crates/sley-repo/src/index_cache.rs:160-173,379-391; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:134-137,185-188 - `verify_cached_snapshot` reports a symlinked or non-directory `index/` as benign `Missing` when nothing is behind it and as `Mismatch` otherwise - closure evidence: `Mismatch` for any non-real existing component (or stated audit behaviour), with a test
[P4] [evidence-binding] crates/sley-repo/src/index_cache.rs:248,306,378,765-779,807,817-826; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:288-290; bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:920 - no test can catch a revert of the canonical-root derivation; "read and written" overclaims for the relative spelling, which is only read; the `index/v1` read case passes trivially - closure evidence: a checker anchor or reworded record and evidence, and a record planted behind the `index/v1` link
[P4] [spec-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:134-140; crates/sley-repo/src/index_cache.rs:181-191,316-333; crates/sley-repo/src/exchange.rs:1680-1690 - the "every platform … never followed" clause is false off Unix; the lstat-then-open component window is not named under the residual; the purge refuses only `index/`, not `index/v1/` - closure evidence: scoped wording and the residual named
[P4] [out-of-section/spec-precision] crates/sley-protocol/src/server.rs:1181-1186,2375-2392,2469-2472,2932-2933; docs/spec/SMP1.md:763-765; docs/spec/SLEY2_TRIAL_RUNNER_V1.md:24-25,330-332 - the claim that "an absent boundary fails the method" is contradicted by the session check's head load, which initializes the boundary through `self.maintenance()` (found by reading the code, not executed) - closure evidence: corrected sentences or a non-initializing head load, and a test that removes `locks/`
SUMMARY: S20-300 revision 6 closes all four of this lane's revision-5 P4 findings, each checked against code, tests and checker output: a gated single wrapper caller with a literal-aware gate, exact coverage and evidence wording, real-directory cache components on read and write-back, and T54 regenerated last with the supply-chain check exiting 0. The section checker exits 0 with PASS at revision 6, the unit suite passes 21/21, the `index_cache` tests 13/13 and the `workspace_open` tests 5/5; the cache path now comes from the guard's canonical root, and ownership, dependency direction and the single acceptance authority are unchanged. One P3 (stale revision identity in ADR-0029 and the work-package row) and five P4 notes remain, none of which breaks correctness, fail-closed acceptance or identity disclosure.
