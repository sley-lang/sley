<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f073811914297505803f5b731cd4691e1623de84; role: nabu; field: nabu_architecture_review_revision_4; dispatched: 2026-09-23T09:08:06Z; duration_s: 372; process_exit_code: 0 -->
# Nabu Council review — complete_root_index_snapshot
Harness: claude-code
Reviewed checkpoint: f073811914297505803f5b731cd4691e1623de84

I verified the following myself; I did not rely on the author's records or notes:
- `git rev-parse HEAD` → `f073811914297505803f5b731cd4691e1623de84` (scope matches); `git log --oneline ab42a3a9..HEAD` (15 commits, probe landed in dda51a16/a8b4cddb).
- `git diff --stat` and full `git diff ab42a3a9..HEAD -- docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md crates/sley-repo scripts/` (10 files, +161/−26); `git diff --stat ab42a3a9..HEAD -- crates/sley-txn crates/sley-protocol crates/sley-cli` (only server.rs and server_tests.rs changed; the non-blocking shared acquire already existed).
- `python3 scripts/check_complete_root_index_snapshot_profile.py` → exit 0, `"result": "PASS"`, `"revision": 4`, `"status": "S20_300_FULL_IMPLEMENTED_REVIEW_PENDING"`, `"problems": []`.
- `cargo test -p sley-repo --lib index_cache` → exit 0, `test result: ok. 9 passed; 0 failed` (includes `probe_reports_only_a_materialized_snapshot_and_never_builds` and `guard_for_another_repository_is_refused`). Artifacts were fresh in the preset CARGO_TARGET_DIR.
- `cargo test -p sley-protocol --lib workspace_open` → exit 0, `2 passed` (`workspace_open_v2_discloses_only_the_materialized_head_snapshot`, `workspace_open_under_version_1_never_carries_field_9`).
- `python3 scripts/check_smp1_contract.py` → exit 0, `"result": "PASS"`, `"revision": 13`; `python3 -m unittest scripts/test_smp1_contract.py` → `Ran 14 tests ... OK`.
- Files read:
  - crates/sley-repo/src/index_cache.rs:1-654 (whole file)
  - docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:1-254 (whole file)
  - scripts/check_complete_root_index_snapshot_profile.py:1-233 (whole file)
  - crates/sley-protocol/src/server.rs:495-555, 2430-2469, 2675-2714, 2860-2970, 3290-3316
  - crates/sley-protocol/src/server_tests.rs:605-654, 7380-7461
  - crates/sley-txn/src/maintenance.rs:1-260
  - crates/sley-txn/src/repository.rs:881-906, 4063-4073
  - crates/sley-query/src/snapshot.rs:440-484, 612-721
  - crates/sley-repo/src/root_query.rs:85-134
  - crates/sley-cli/src/lib.rs:340-379, 1115-1149
  - docs/spec/SMP1.md:690-719
  - docs/adr/ADR-0029-complete-root-index-snapshot-boundary.md:28-47
  - machineresearch/sley-2.0/machine-summary.json:781-838
  - prior verdict lines of evidence/review/verdicts/complete_root_index_snapshot/nabu-final-review-d384f0f.md
- I did not read the untracked Ariadne revision-4 transcript.

## Evidence checked

- **The probe cannot build, write or delete.** `cached_complete_root_snapshot_id` (index_cache.rs:256-270) makes three calls: `index_cache_path`, then `read_record` (read-only `File::open` with a `take(MAX+1)` bound, :277-291), then `accept_cached(...).ok()`.
  - It never calls `fresh_snapshot`, `write_record`, `create_dir_all`, `rename` or `remove`.
  - The unit test covers cold absence with no file created, a hit after objects are removed, and a digest-corrupt record giving `None` with the bytes left unchanged (:568-604).
- **The probe cannot wait on or create locks.**
  - The probe borrows the caller's guard and takes no lock itself.
  - The one consumer, server.rs:2927, uses `acquire_shared_repository_maintenance_nonblocking`. That maps to `try_lock_shared` (maintenance.rs:142-146, 180-186) and opens the existing `locks/maintenance.lock` without `create` (:153-161). `initialize_repository_maintenance` is not called on this path.
  - The blocking shared lock inside `head()` (repository.rs:881-885) is existing S20-390 behaviour, and server.rs:2920-2922 says so accurately.
- **The four acceptance rules are the same as on the query hit path, by construction.** Both paths run `read_record` then `accept_cached` (index_cache.rs:214-216 vs :267-269).
  - `decode_complete_root_snapshot` → `inspect_candidate_for_arm` enforces:
    - rule 1: context, epoch, field-schema hash and arm 2 (snapshot.rs:641-662);
    - rule 4: edge endpoints in the inventory, ordered, plus the reverse-group checks (:678-721);
    - the size bound (:620-621);
    - rule 2: the digest.
  - `accept_cached` enforces rule 3, inventory aligned to `entity_bindings` (index_cache.rs:113-124).
- **No stale or discarded identity can leak.**
  - The identity is taken from the trailer of the exact bytes that were accepted (snapshot.rs:465-470), so nothing can change between acceptance and disclosure.
  - Files are keyed by `StateRoot`, and rules 1 and 3 bind to that root's context and bindings, so another head's record can never be accepted.
  - Any discard maps to `None`.
  - A forged digest-valid record is the rev-3 residual (spec :144-155). The disclosed id is then the same id `query.root` would bind; capsules build fresh and would refuse it.
  - Field 9 is paired with the same `head` used for fields 1-8 (server.rs:2906-2912).
- **Ownership and dependency direction.**
  - The direction is protocol → repo (cache owner) → query/txn.
  - The probe reuses private helpers, so there is no second acceptance authority.
  - ADR-0029:39-41 and SMP1:697-712 match spec §5 (:164-183); `machine-summary.cache_consumers` was updated.
- **The checker** now pins the spec and cache markers, `SPEC_REVISION=4`, and a probe-caller allowlist of `{server.rs}`. Today server.rs has one import (:43) and one call (:2928).

## Findings
[P3] [identity-discipline] crates/sley-repo/src/index_cache.rs:256-265; crates/sley-txn/src/maintenance.rs:153,188-189,195-197; crates/sley-protocol/src/server.rs:2927-2930; crates/sley-cli/src/lib.rs:357,1136-1140; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:174 - The new probe copies the raw `guard.repository_root() != repository` check. The guard holds the canonicalized root, and the coverage authority `RepositoryMaintenanceGuard::covers` (used at sley-txn repository.rs:4067 and sley-repo gc.rs:499,555,723,1260) is bypassed. The server passes its uncanonicalized `repository`, taken verbatim from `sley serve --repository`. So with a relative or symlinked repository path the probe returns INDEX_SNAPSHOT_IO for its own repository, and server.rs:2929 turns that into a silent, permanent absence of field 9. The rev-4 sentence "the only error is a guard naming another repository" is therefore inaccurate. The failure direction is safe (no leak). The same raw check on the rev-3 query path (:208, :318) makes query.root fail for that spelling too - closure evidence: delegate all three cache entry points to `guard.covers(repository)` (or canonicalize once at Server construction), plus a test in which a relative or symlinked repository path still yields the probe identity and a query.root hit
[P3] [evidence-binding/checker] scripts/check_complete_root_index_snapshot_profile.py:24,207-210; machineresearch/sley-2.0/machine-summary.json:797-799,825 - SPEC_REVISION moved to 4, but the COMPLETE_STATUS gate still accepts the unsuffixed lane fields. Those fields still hold the revision-3 PASS values, and the section's own status note says "the revision 4 Ariadne, Nabu, and Vulcan reviews have not run". Flipping `status` back to S20_300_FULL_COMPLETE would pass this checker with zero revision-4 reviews. The S20-620 checker added exactly this binding in the same range (scripts/check_sley2_trial_runner.py:295-301, `completion-unbound-review`) - closure evidence: require `<lane>_revision_{SPEC_REVISION}` to start with PASS at COMPLETE_STATUS, plus a negative case showing that a COMPLETE status carrying only unsuffixed PASS fields is refused
[P4] [checker-precision] scripts/check_complete_root_index_snapshot_profile.py:180-187; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:182-183 - The probe allowlist is per file, so any number of extra call sites inside the ~3.7k-line server.rs pass (for example a disclosure added to revision.read). `path.name in ("index_cache.rs",)` exempts every same-named file in any crate. The regex needs a literal `cached_complete_root_snapshot_id(`, so aliased imports and function-pointer uses (`.map(cached_complete_root_snapshot_id)`) are missed. "A new probe caller fails the stage checker" overstates the gate - closure evidence: exempt only the relative path crates/sley-repo/src/index_cache.rs, pin exactly one call inside `fn materialized_head_snapshot`, and match the bare identifier outside `use` lists, with negative tests
[P4] [test-coverage] crates/sley-protocol/src/server.rs:2923-2931; crates/sley-protocol/src/server_tests.rs:615-640,7393-7461; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:178-180 - Rev 4 writes the consumer's obligations into the S20-300 text: take the guard without waiting, never initialize, treat any failure as absence. These are shown only by code reading. The server tests cover cold, warm and version-1 cases, but not a contended boundary (an exclusive holder) or a discarded record reached through workspace.open - closure evidence: an in-crate test of `materialized_head_snapshot` that returns None without blocking while an exclusive maintenance guard is held, and a server test in which a corrupted cache file gives an eight-field body and leaves the file's bytes unchanged

## Assessment
The probe itself does what the revision-4 contract says:
- It shares the query hit path's `read_record` and `accept_cached`, so the four rules are the same by construction.
- It performs no build, write, delete or lock operation.
- It derives the identity from the exact accepted bytes, so a discarded record or another head's record cannot be disclosed.
- Its one consumer takes the guard without blocking and never initializes the boundary.
- Ownership stays with sley-repo; the dependency direction is protocol → repo → query/txn; ADR, SMP1 and spec agree.

The section checker passes and the probe tests pass. Two report-grade issues remain:
- **Identity discipline:** the probe copies a raw-path guard check instead of using the canonical `covers` authority, and the consumer silently drops the resulting error. A non-canonical repository path therefore disables field 9 permanently, and the spec states the error surface too narrowly.
- **Evidence binding:** the checker's completion gate is not bound to revision 4, so the stale revision-3 PASS fields would satisfy it.

Neither issue is a safety or correctness break; both fail in the safe direction. They do need closing before this lane accepts revision 4.

VERDICT: REVISE_0_P0_0_P1_0_P2_2_P3_2_P4
SECTION: complete_root_index_snapshot
FIELD: nabu_architecture_review_revision_4
SCOPE_SHA: f073811914297505803f5b731cd4691e1623de84
FINDINGS:
[P3] [identity-discipline] crates/sley-repo/src/index_cache.rs:256-265; crates/sley-txn/src/maintenance.rs:153,188-189,195-197; crates/sley-protocol/src/server.rs:2927-2930; crates/sley-cli/src/lib.rs:357,1136-1140; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:174 - probe copies raw `guard.repository_root() != repository` equality against a canonicalized guard root instead of `RepositoryMaintenanceGuard::covers`; a relative or symlinked `--repository` makes the probe error on its own repository, which server.rs:2929 silently turns into permanent absence of field 9, and "the only error is a guard naming another repository" is inaccurate - closure evidence: use `guard.covers(repository)` (or canonicalize at Server construction) for all three cache entry points, plus a relative/symlinked-path probe and query.root test
[P3] [evidence-binding/checker] scripts/check_complete_root_index_snapshot_profile.py:24,207-210; machineresearch/sley-2.0/machine-summary.json:797-799,825 - SPEC_REVISION=4 but the COMPLETE gate still accepts the unsuffixed revision-3 PASS fields, so a status flip passes with no revision-4 review (compare check_sley2_trial_runner.py:295-301) - closure evidence: require `<lane>_revision_{SPEC_REVISION}` PASS at COMPLETE, plus a negative case
[P4] [checker-precision] scripts/check_complete_root_index_snapshot_profile.py:180-187; docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:182-183 - per-file allowlist admits any number of server.rs call sites, the name-only exemption skips every index_cache.rs, and the `(`-anchored regex misses aliases and function references - closure evidence: relative-path exemption, exactly one call pinned in `materialized_head_snapshot`, bare-identifier match, negative tests
[P4] [test-coverage] crates/sley-protocol/src/server.rs:2923-2931; crates/sley-protocol/src/server_tests.rs:615-640,7393-7461 - the no-wait and failure-as-absence consumer obligations stated in the rev-4 spec are shown only by code; no test covers a contended boundary or a discarded record through workspace.open - closure evidence: an exclusive-guard-held `materialized_head_snapshot` → None test and a corrupted-cache workspace.open test with the bytes unchanged
SUMMARY: The rev-4 identity probe is sound: it shares the query hit path's four acceptance rules exactly, never builds, writes, deletes or locks, and cannot disclose a discarded or foreign-root identity; the section checker passes (exit 0) and the probe tests pass (9/9 index_cache, 2/2 workspace_open). Two P3s block lane acceptance. The probe copies a raw-path guard check instead of the canonical `covers` authority, so a non-canonical repository path silently disables field 9 and the spec's error claim is inaccurate. The checker's completion gate also accepts the stale revision-3 PASS fields for revision 4. Two P4 notes cover checker allowlist precision and missing contended/discarded consumer tests.
