<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: nabu; field: nabu_architecture_review_revision_5; dispatched: 2026-09-23T12:06:02Z; duration_s: 412; process_exit_code: 0 -->
# Nabu Council review — complete_root_index_snapshot
Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

I verified everything below myself. I did not rely on the author's records or notes.

**Git commands**
- `git rev-parse HEAD` → `2b0f1c9f4940c020565137891a49a3769cd02061`, so the scope matches.
- `git log --oneline f0738119..HEAD`: 13 commits. The S20-300 revision 5 repair is 40a84a5c.
- `git diff --stat` and the full diff `f0738119..HEAD` over the spec, `crates/sley-repo`, `Cargo.lock` and the section checker: 5 files, +249/−52.
- Also diffed over the same range: `server.rs`, `server_tests.rs` and `fuzz/Cargo.lock`.
- `git diff --name-status 746d5b30..HEAD`: 2 files added and 16 modified after the last T54 regeneration.

**Checkers and tests**
- `python3 scripts/check_complete_root_index_snapshot_profile.py` → exit 0, `"result": "PASS"`, `"revision": 5`, `"status": "S20_300_FULL_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
- `python3 -m unittest scripts/test_complete_root_index_snapshot_profile.py -v` → `Ran 13 tests ... OK`. That is 4 CompletionBinding and 9 ProbeGate cases.
- `cargo test -p sley-repo --lib index_cache` → exit 0, `test result: ok. 12 passed; 0 failed`. This includes:
  - `a_symlink_to_a_valid_record_is_absence_for_the_probe`
  - `a_fifo_at_the_cache_path_answers_at_once`
  - `a_non_canonical_repository_spelling_is_covered_by_its_guard`
  - `guard_for_another_repository_is_refused`
  - `probe_reports_only_a_materialized_snapshot_and_never_builds`
- `cargo test -p sley-protocol --lib workspace_open` → exit 0, `4 passed`. This includes:
  - `workspace_open_v3_answers_the_version_2_open_summary`
  - `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`
- `python3 scripts/check_supply_chain_audit.py` → exit 1: `{"reason": "generated audit evidence drifted: {\"drift\": [\"evidence/security/T54/secret-scan.json\"] ...}", "result": "FAIL"}`.
  - An in-memory field diff of T54 against the generator shows `candidate_files_scanned` 2404 → 2406, and `candidate_bytes_scanned` and `candidate_file_manifest_sha256` also changed.
  - The untracked counters differ too, but they are masked in the comparison.
  - I ran the remaining supply-chain checks in memory, skipping only the drift step: inventory, secret scan, summary and reconciliation all PASS. The T52 inventory has 178 relationships, including `sley-repo → libc@0.2.189`.
  - The `Cargo.lock` sha256 `107f4df8…` equals `s20_710_pre_release_audit.cargo_lock_sha256`.
- In-memory scratch-tree probes of `probe_gate_problems` wrote only to tempdirs. All three returned `[]`:
  - (A) an extra `self.materialized_head_snapshot(h)` caller in server.rs;
  - (B) a blocking `acquire_shared_repository_maintenance(` added beside the non-waiting call;
  - (C) a probe call after a `"http://x"` literal on the same line in another crate.

**Files read**
- crates/sley-repo/src/index_cache.rs:1-480 and 560-748
- crates/sley-txn/src/maintenance.rs:1-210
- crates/sley-protocol/src/server.rs:2885-2944
- The server_tests.rs diff; new tests at 7468-7585
- docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md: the full diff and 100-281
- scripts/check_complete_root_index_snapshot_profile.py: 89-165 (via the diff) and 168-307
- scripts/test_complete_root_index_snapshot_profile.py:1-189
- crates/sley-repo/src/exchange.rs:1674-1705
- crates/sley-repo/src/lib.rs:9-30
- docs/adr/ADR-0029-complete-root-index-snapshot-boundary.md:28-49
- docs/spec/ERROR_CODES_V1.md:268-276
- docs/audits/S20_710_PRE_RELEASE_AUDIT.md:140-170
- scripts/generate_supply_chain_evidence.py:538-637
- scripts/check_supply_chain_audit.py:60-79, 262-286
- bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:603-782
- The machine-summary `complete_root_index_snapshot` section, all keys
- My lane's prior transcript, `evidence/review/verdicts/complete_root_index_snapshot/nabu_architecture_review_revision_4-f073811.md`

I did not read the untracked Ariadne revision-5 transcript. I did not review the §12.6 scratch-cleanup code beyond its effect on T54.

## Evidence checked

**Status of my revision-4 findings**

- **[P3] identity discipline — CLOSED.**
  - All three cache entry points now delegate to the canonical authority `RepositoryMaintenanceGuard::covers`: `complete_root_snapshot` index_cache.rs:210, the probe :265 and `verify_cached_snapshot` :327.
  - `covers` (maintenance.rs:35-36) is `canonical_real_directory(root) == self.repository_root`. That is the same canonicalization `acquire_repository_maintenance` applies (:153), so no second identity rule was introduced.
  - Test :691-723 proves parent-symlink and `..` spellings reach the probe identity, `Match` and a query-path build. It also proves a final-component symlink stays refused.
  - The check-then-use shape (`covers(caller path)`, then I/O through the caller's spelling) is the established S20-390 idiom (sley-txn repository.rs:4067).
  - Spec :117-121 and :188 now state the rule.
- **[P3] evidence binding — CLOSED.**
  - At COMPLETE the checker now requires `<lane>_revision_{SPEC_REVISION}` to start with `PASS`, or it reports `completion-unbound-review` (checker :280-284).
  - Negative tests refuse a flip on the historical base PASS and a flip on revision-4 fields; a bound current PASS is admitted.
  - The machine summary has no `_revision_5` fields yet, so COMPLETE is correctly unreachable.
- **[P4] checker precision — CLOSED as requested.** The new residual is recorded as finding 1 below.
  - The exemption is the full relative path (:137), not the file name.
  - Only `crates/<crate>/tests/` is exempt (:139).
  - The match is the bare-identifier regex `\b…\b`, covering calls, imports, aliases and function pointers.
  - The consumer is pinned to exactly one reference inside `materialized_head_snapshot`, and aliases are refused.
  - `fuzz/` is scanned.
  - There are eight negative cases.
- **[P4] test coverage — CLOSED.**
  - server_tests.rs:7543-7585 shows that while an exclusive guard is held, `materialized_head_snapshot` → `None` in under 5 s.
  - It also shows that a corrupted record gives an 8-field `workspace.open` body with the file bytes unchanged.

**The rest of the revision-5 delta**

- **`read_record` (index_cache.rs:283-300).**
  - It opens once with `O_NOFOLLOW | O_NONBLOCK`, then requires `is_file` on the handle, then reads under the `MAX_SNAPSHOT_RECORD_BYTES + 1` cap.
  - This removes the rev-4 lstat→open window.
  - The query path's fail-closed behaviour for a non-file is unchanged: `read_record` → `None` → fresh build → the `symlink_metadata` refusal at :224-231. The symlink and FIFO tests assert this.
- **The probe still cannot build, write, delete or lock.**
  - It is `covers`, then `read_record`, then `accept_cached(...).ok()` (:265-273).
  - It shares `accept_cached` with the hit path, so there is still one acceptance authority.
- **Dependency direction.**
  - The dependency direction is unchanged: protocol → repo (the cache owner) → query/txn.
  - The new `libc =0.2.189` edge is exact-pinned, like `blake3 =1.8.2`. It was already in the registry set via `cpufeatures`.
  - It is mirrored in `Cargo.lock`, `fuzz/Cargo.lock`, T52 (178 relationships) and S20-710 audit :152-158.
- **The consumer (server.rs:2925-2933).**
  - It takes `acquire_shared_repository_maintenance_nonblocking`, never initializes, and flattens any error to absence.
  - The rev-5 docstring and spec :196-198 correctly say the head load keeps the S20-390 blocking acquisition.
  - The version rule `>= PROTOCOL_VERSION_V2` matches spec :191-194, and there is a v3 test.
- **Unchanged pins.** ADR-0029:39-41 still names "SMP1 revision 13" in a sentence dated to profile revision 4. That is accurate as history, so I am not raising it.

## Findings
[P4] [checker-precision] scripts/check_complete_root_index_snapshot_profile.py:96,141,148-161; crates/sley-protocol/src/server.rs:2925; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:201-206 - The probe gate pins the identifier to one call inside `materialized_head_snapshot`, but it does not gate who calls that helper, and revision 5 widened the helper to `pub(crate)`. A second disclosure path, such as `revision.read` calling `self.materialized_head_snapshot`, passes (scratch case A → `[]`). The "never waits" check only looks for `self.maintenance()` or `initialize_repository_maintenance`, so a blocking `acquire_shared_repository_maintenance(` added beside the non-waiting call passes (case B → `[]`). The `//` stripping also runs inside string literals, so a call after `"http://…"` on the same line is hidden (case C → `[]`). That contradicts the spec's "any other reference … fails the gate". This is drift-guard precision, not a safety defect: today the tree has exactly one caller - closure evidence: require exactly one non-test `materialized_head_snapshot` reference, inside `fn workspace_open`; refuse any blocking acquisition in the consumer body; strip comments in a string-aware way; negative tests for all three cases
[P4] [spec-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:119-121,258-262; crates/sley-txn/src/maintenance.rs:35-36,195-207; crates/sley-repo/src/index_cache.rs:720-722; crates/sley-protocol/src/server_tests.rs:7563-7570 - Two overstatements. "A relative or otherwise non-canonical spelling of the same repository is covered" is too broad: `covers` refuses a final-component symlink spelling, and the revision-5 test asserts that refusal. The relative spelling works through `canonicalize` but is untested. The required-evidence line promises "through `workspace.open`, absence while an exclusive maintenance owner holds the boundary", but the test calls the `materialized_head_snapshot` helper directly. Through `workspace.open` the head load would block under S20-390, as spec :196-198 itself states - closure evidence: wording that excludes a final-component symlink and places the exclusive-owner evidence at the consumer helper, or a relative-path test plus a matching rewording
[P4] [fail-closed-structure] crates/sley-repo/src/index_cache.rs:157-186,283-291; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:122-125; crates/sley-repo/src/exchange.rs:1680-1689 - `O_NOFOLLOW` protects only the final path component. A symlinked `index/` or `index/v1/` is still followed on read, and on write-back through `create_dir_all`/rename, which can place a derived record outside the repository. Import refuses a symlinked `index`, so the two paths treat the same tamper differently. The flags are also `#[cfg(unix)]`-only while the text states them unconditionally. This predates revision 5 and stays inside the section 5 same-authority residual (rules 1-4 still bind any accepted bytes), so there is no leak - closure evidence: either scope the section 5 sentence to the final component and Unix, naming intermediate directories under the residual; or require real directories for `index` and `index/v1` (a component lstat or an `O_DIRECTORY|O_NOFOLLOW` walk), with a symlinked-`index` test for both the probe and the write-back
[P4] [evidence-binding/out-of-section] evidence/security/T54/secret-scan.json; bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:720 - At the scope SHA, `check_supply_chain_audit.py` exits 1 because T54 has drifted. T54 was last regenerated at 746d5b30. d307a22f (scratch.py, test_scratch_cleanup.py and 14 modified bench files) and 2b0f1c9f (the record itself) came after it, so `candidate_files_scanned` is 2404 recorded vs 2406 derived. Gate row 12.4 records PASS without naming the commit it ran at. The libc mirror itself is correct (lock digest and 178 relationships verified), and this has no bearing on S20-300 acceptance - closure evidence: regenerate T54 as the final commit, or bind the §12.4 row to 746d5b30, then show `check_supply_chain_audit.py` exit 0 at the new head

## Assessment
Revision 5 closes all four of my lane's revision-4 findings, and I verified each against code, tests and checker output.

- **Guard identity:** every cache entry point now goes through the canonical `covers` authority, and non-canonical spellings are tested.
- **Evidence binding:** the completion gate is bound by field name to `<lane>_revision_5` PASS, with refusal tests.
- **Probe gate:** it now matches the bare identifier by full path, including `fuzz/`, and is pinned to one call in the consumer helper.
- **Consumer obligations:** the no-wait and discarded-record obligations are exercised by tests.
- **Safe open:** the probe opens once with `O_NOFOLLOW|O_NONBLOCK`, has no check-then-open window, and still shares the single `accept_cached` acceptance authority.
- **Ownership and dependencies:** ownership and dependency direction are unchanged, and the new libc edge is pinned and mirrored.

The four new notes are all P4:

- one level of gate indirection remains;
- two spec sentences overstate;
- intermediate cache-directory symlinks are still followed (pre-existing, within the stated residual);
- the out-of-section T54 record is stale at this head.

None of them breaks correctness, fail-closed behaviour or identity disclosure. This lane accepts S20-300 revision 5.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_4_P4_PRIOR_P3_P4_CLOSED
SECTION: complete_root_index_snapshot
FIELD: nabu_architecture_review_revision_5
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS:
[P4] [checker-precision] scripts/check_complete_root_index_snapshot_profile.py:96,141,148-161; crates/sley-protocol/src/server.rs:2925; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:201-206 - callers of the now `pub(crate)` consumer helper are not gated, a blocking acquire beside the non-waiting one passes, and `//` inside a string literal hides a call (scratch cases A/B/C all `[]`) - closure evidence: exactly one non-test `materialized_head_snapshot` reference inside `fn workspace_open`, refuse any blocking acquisition in the body, string-aware comment stripping, negative tests
[P4] [spec-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:119-121,258-262; crates/sley-txn/src/maintenance.rs:35-36,195-207; crates/sley-repo/src/index_cache.rs:720-722; crates/sley-protocol/src/server_tests.rs:7563-7570 - "otherwise non-canonical spelling is covered" is too broad (a final-component symlink is refused) and the relative spelling is untested; the exclusive-owner evidence is claimed "through workspace.open" but is tested on the helper - closure evidence: corrected wording, or a relative-path test plus rewording
[P4] [fail-closed-structure] crates/sley-repo/src/index_cache.rs:157-186,283-291; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:122-125; crates/sley-repo/src/exchange.rs:1680-1689 - O_NOFOLLOW covers only the final component, so symlinked `index`/`index/v1` are followed on read and write-back, unlike the import purge; the flags are Unix-only while the text is unconditional (pre-existing, within the residual) - closure evidence: scope the sentence, or require real directories for both components with a test
[P4] [evidence-binding/out-of-section] evidence/security/T54/secret-scan.json; bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:720 - check_supply_chain_audit.py exits 1 at 2b0f1c9f because T54 was minted at 746d5b30, before d307a22f and 2b0f1c9f (2404 vs 2406 files); the §12.4 PASS row names no commit - closure evidence: regenerate T54 last, or bind the row to 746d5b30, and show exit 0 at the new head
SUMMARY: S20-300 revision 5 closes all four of this lane's revision-4 findings, verified against code, tests and checker output: canonical `covers` at every cache entry point, completion bound to `<lane>_revision_5` PASS, a full-path bare-identifier probe gate, and consumer contention and discard tests. The section checker exits 0 with PASS at revision 5; checker unit tests pass 13/13, `index_cache` 12/12 and `workspace_open` 4/4. The one-open `O_NOFOLLOW|O_NONBLOCK` read keeps one acceptance authority and the same dependency direction, and the libc edge is pinned and mirrored. Four P4 notes remain (gate indirection, two spec overstatements, intermediate-directory symlinks, and an out-of-section stale T54), none of which blocks acceptance.
