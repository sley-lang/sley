<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f073811914297505803f5b731cd4691e1623de84; role: ariadne; field: ariadne_contract_review_revision_4; dispatched: 2026-09-23T08:55:06Z; duration_s: 368; process_exit_code: 0 -->
# Ariadne Council review — complete_root_index_snapshot

Harness: claude-code
Reviewed checkpoint: f073811914297505803f5b731cd4691e1623de84

I verified the following myself:
- **Scope.** `git rev-parse HEAD` returned `f073811914297505803f5b731cd4691e1623de84`, which matches the scope SHA. The worktree is clean.
- **Git commands run:**
  - `git diff --stat ab42a3a9..HEAD -- docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md crates/sley-repo scripts/`
  - the full `git diff ab42a3a9..HEAD` of the spec, `crates/sley-repo`, the section checker, and the other six changed scripts
  - `git diff ab42a3a9..HEAD -- crates/sley-protocol/src/server.rs crates/sley-protocol/src/server_tests.rs` (the probe's consumer)
  - `git log --oneline ab42a3a9..HEAD -- <scoped files>`: 2 commits, c2e37384 and a8b4cddb
  - `git log -- docs/adr/ADR-0029-…`
- **Section checker.** `python3 scripts/check_complete_root_index_snapshot_profile.py` exited 0 with `"result": "PASS"`, `"revision": 4`, `"status": "S20_300_FULL_IMPLEMENTED_REVIEW_PENDING"` and `"problems": []`.
- **Unit tests.** `cargo test --locked --offline -p sley-repo --lib index_cache` exited 0 with `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 412 filtered out`. The 9 tests include `probe_reports_only_a_materialized_snapshot_and_never_builds`, `guard_for_another_repository_is_refused`, and `exchange::tests::an_incomplete_clone_resumes_without_adopting_the_index_cache_it_carried`.
- **In-memory checker probes** (no files written):
  - (a) I patched `Path.rglob` and `Path.read_text` to inject `crates/sley-cli/src/fake_probe.rs`, which calls the probe, plus `crates/sley-protocol/src/tests/allowed.rs`. Result: exit 1, `FAIL ['probe-caller:crates/sley-cli/src/fake_probe.rs']`. The checker skips the file under a `tests/` directory, as designed.
  - (b) I patched the summary to `S20_300_FULL_COMPLETE` with `implementation_complete: true` and no `*_revision_4` fields. Result: exit 0, `PASS []`.
- **Files read:**
  - `crates/sley-repo/src/index_cache.rs:1-653` and `crates/sley-repo/src/lib.rs:19-30`
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:1-254`
  - `scripts/check_complete_root_index_snapshot_profile.py:1-233`
  - `crates/sley-protocol/src/server.rs:2435-2460` and `2890-2931`
  - `crates/sley-txn/src/maintenance.rs:1-222`
  - `crates/sley-txn/src/repository.rs:878-885`
  - `crates/sley-query/src/snapshot.rs:457-478` and `612-754`
  - `crates/sley-protocol/src/lib.rs:1068-1097`
  - `docs/adr/ADR-0029-…:25-51`
  - `docs/spec/SMP1.md:690-712`
  - `docs/spec/ERROR_CODES_V1.md:264-273`
  - `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md:404-427`
  - `docs/WORK_PACKAGES.md:35` (S20-300 row)
  - `machine-summary.json` section `complete_root_index_snapshot`
- **Not run:** the `sley-protocol` server tests. My statements about the consumer (`server.rs`) come from reading the code and the test diff, not from running them.

## Evidence checked

**1. The probe can't build, write, or delete.**
- `cached_complete_root_snapshot_id` (`index_cache.rs:256-270`) does three things: it checks the guard, computes the path, and runs `read_record(&path).and_then(|r| accept_cached(revision, &r).ok()).map(snapshot_id)`.
- It never reaches `fresh_snapshot`, `write_record`, `create_dir_all`, `fs::write`, `rename`, or `remove`.
- `read_record` (`:277-291`) only calls `symlink_metadata`, `File::open` (read-only), `metadata`, and a bounded `take(MAX+1).read_to_end`.
- The test at `:569-604` covers:
  - cold absence, with `!path.exists()`;
  - identity equal to `built.snapshot_id()` after the query path has materialized the record;
  - a hit that still succeeds after `objects/` is removed;
  - a record with one flipped trailer byte, which yields `None` and leaves the file byte-identical (`"no write-back"`).

  Those are the four revision-4 evidence items in spec §8 (`:233-235`).

**2. The four acceptance rules are the same as on the query hit path.** Both `complete_root_snapshot` (`:214-216`) and the probe (`:267-269`) call the same `read_record` and `accept_cached`, so the probe cannot accept anything the query path would reject.
- Rule 1 (arm 2 and context): `decode_complete_root_snapshot` rejects a rootless context (`snapshot.rs:461`) and checks epoch, schema hash, and root context (`:646-651`), arm (`:652-659`), and root presence (`:660`).
- Rule 2 (trailer digest): `:743`.
- Rule 3 (inventory matches bindings): `accept_cached` (`index_cache.rs:113-124`) checks length and order.
- Rule 4 (edge endpoints and inversion): endpoints at `snapshot.rs:687-691` and `:704`/`:728`; exact inversion at `:746`.
- Size bound: `MAX_SNAPSHOT_RECORD_BYTES` (`:620`).

**3. No stale or discarded identity can leak.**
- The returned ID is the trailer, and it is taken only after the digest check (`snapshot.rs:466-470`, `:743`).
- A record for another root or epoch fails rule 1.
- Every discarded record becomes `None` through `.ok()`.
- `server.rs:2906-2912` pairs field 9 with fields 1-8 of the same `head` value. If the head moves between the head load and the probe, the result is still a self-consistent response for one accepted revision; it can't mix two revisions.

**4. Locks.**
- The probe takes a `&RepositoryMaintenanceGuard` and does no locking of its own.
- Its only caller (`server.rs:2927`) uses `acquire_shared_repository_maintenance_nonblocking`. That call:
  - requires an existing `locks/` directory and `maintenance.lock` file (`maintenance.rs:153-161`), so it creates nothing and doesn't initialize the boundary;
  - uses `try_lock_shared`, which returns `WouldBlock` when the lock is contended (`:180-186`);
  - has every failure turned into absence by `.ok()?`.
- A guard for a different repository gets `INDEX_SNAPSHOT_IO` (`index_cache.rs:261-265`); the test at `:561-565` checks this.

**5. Caller gate.**
- A repo-wide grep finds exactly one production call, at `server.rs:2928`.
- The checker allowlists only `crates/sley-protocol/src/server.rs` (`checker:180-187`).
- Probe (a) confirmed that an unlisted caller fails the check.

**6. Records.**
- The machine summary has `contract_revision: 4` and `cache_consumers: READ_ONLY_DERIVED_QUERY_SURFACES_AND_WORKSPACE_OPEN_IDENTITY_PROBE`.
- Its status is `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`; the status note says the revision-4 reviews have not run.
- ADR-0029:39-41 was amended for revision 4.
- SMP1 appendix A (`:697-712`) matches §5 of this profile.

## Findings

[P3] [contract-consistency] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:36-40 - The preamble still says a cache hit is accepted "only for read-only derived query surfaces". That contradicts revision 4's own status paragraph (lines 9-12), §5 (lines 151-152 and 164-183), and ADR-0029:39-41, which all allow the non-query `workspace.open` identity probe to read a cache hit. The same outdated "hits serve read-only derived query surfaces only" wording is also in docs/spec/ERROR_CODES_V1.md:272-273, docs/WORK_PACKAGES.md:35 (the row that announces revision 4), and the module doc at crates/sley-repo/src/index_cache.rs:4-8 - closure evidence: amend those sentences to name the identity probe as the only non-query hit reader (or limit "only" to reading snapshot contents), then re-run the section checker.

[P3] [gate] scripts/check_complete_root_index_snapshot_profile.py:207-210 - The completion gate checks only the base `ariadne_contract_review`/`nabu_architecture_review`/`vulcan_surface_review` fields, which still hold the revision-3 PASS values (the machine-summary status note says so). In-memory probe (b) set the status to `S20_300_FULL_COMPLETE` with no `*_revision_4` field, and the checker returned exit 0 `PASS` with no problems, so the package could return to complete without any revision-4 review - closure evidence: require `<lane>_revision_<SPEC_REVISION>` PASS fields at completion (as scripts/check_sley2_trial_runner.py:298-301 now does), and show that the unbound flip FAILs.

[P4] [gate-coverage] scripts/check_complete_root_index_snapshot_profile.py:180-187 - The probe-caller gate scans only `crates/**/*.rs`, and only for a literal `cached_complete_root_snapshot_id(` call. Callers in `fuzz/targets/*.rs` (which depend on sley-repo, fuzz/Cargo.toml:102), aliased imports, and function-pointer uses would get past it, so spec lines 182-183 ("A new probe caller fails the stage checker") promise more than the gate checks. No such caller exists today - closure evidence: scan fuzz/ too and match the identifier in imports as well as calls, or narrow the spec sentence.

[P4] [contract-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:177 - The spec allows field 9 "under a version 2 selection" (and SMP1.md:697 says "version 2 selection only"). crates/sley-protocol/src/server.rs:2907 calls the probe for `protocol_version >= PROTOCOL_VERSION_V2`, so a version 3 (native draft) selection also discloses field 9 - closure evidence: either say "version 2 or later selection" in this profile, SMP1 appendix A, and the v3 draft, or gate on `== PROTOCOL_VERSION_V2`; add a v3 `workspace.open` test either way.

[P4] [robustness] crates/sley-repo/src/index_cache.rs:272-290 - `read_record` checks `symlink_metadata` and then calls `File::open`, which follows symlinks and sets neither O_NOFOLLOW nor O_NONBLOCK. A writer with index/v1 access who swaps in a symlink to a FIFO between those two calls makes the open block, so the probe's "adds no wait" claim (server.rs:2917) is not unconditional. The comment's "cannot redirect into a planted symlink" also overstates: the post-open fstat accepts a symlink to a regular file. This is pre-existing since revision 3, the probe inherits it, and it falls within the §5 same-filesystem-authority residual. I derived it from POSIX open semantics and did not execute it - closure evidence: open with `custom_flags(O_NOFOLLOW | O_NONBLOCK)` and keep the fstat check, or reword the comment and limit the no-wait claim to taking the guard.

## Assessment

The substance of revision 4 conforms. `cached_complete_root_snapshot_id` is exactly the cheap half of `complete_root_snapshot`: it goes through the same `read_record` and `accept_cached` path, so it accepts under the same four rules as the query hit path. It cannot build, write back, create directories, or delete. Every missing, unreadable, non-file, or discarded record becomes absence, and the only error is a guard for another repository. The one sanctioned consumer takes the maintenance lock without waiting, never initializes the boundary, and treats every failure as absence. The disclosed identity is the digest-verified trailer of a record accepted for the same head that supplies fields 1-8. All four §8 revision-4 evidence items are covered by `probe_reports_only_a_materialized_snapshot_and_never_builds`, which passes. The checker passes and demonstrably rejects an unlisted caller.

There is no dedicated server-level test for a contended or absent maintenance boundary. I verified that behavior by reading `maintenance.rs:148-193` and `server.rs:2923-2931`.

The two P3 findings are about records and gates, not probe behavior: the profile's own preamble contradicts §5, and the completion gate isn't tied to revision-4 reviews. They should be closed before the package returns to complete. Neither affects correctness or safety.

This verdict accepts S20-300 revision 4 for the Ariadne contract lane only. It claims nothing about GA, release readiness, or the Nabu and Vulcan lanes.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_3_P4
SECTION: complete_root_index_snapshot
FIELD: ariadne_contract_review_revision_4
SCOPE_SHA: f073811914297505803f5b731cd4691e1623de84
FINDINGS: [P3] [contract-consistency] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:36-40 - preamble still says a cache hit is accepted "only for read-only derived query surfaces", contradicting revision 4's status paragraph (9-12), §5 (151-152, 164-183) and ADR-0029:39-41; the same outdated wording is in ERROR_CODES_V1.md:272-273, WORK_PACKAGES.md:35 and index_cache.rs:4-8 - closure: amend them to name the identity probe as the only non-query hit reader and re-run the checker | [P3] [gate] scripts/check_complete_root_index_snapshot_profile.py:207-210 - completion gate checks only the base lane fields (revision-3 PASS); an in-memory flip to S20_300_FULL_COMPLETE with no *_revision_4 fields returns exit 0 PASS - closure: require <lane>_revision_<SPEC_REVISION> PASS fields at completion and show the unbound flip FAILs | [P4] [gate-coverage] scripts/check_complete_root_index_snapshot_profile.py:180-187 - probe-caller gate scans only crates/ for a literal call; fuzz/ (depends on sley-repo) and aliased or function-pointer uses would get past it, so spec 182-183 promises more than the gate checks - closure: scan fuzz/ and match the identifier, or narrow the sentence | [P4] [contract-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:177 - spec allows field 9 "under a version 2 selection" but server.rs:2907 calls the probe for protocol_version >= V2, so v3 also discloses it - closure: say "version 2 or later" in this profile, SMP1 and the v3 draft, or gate on == V2, plus a v3 test | [P4] [robustness] crates/sley-repo/src/index_cache.rs:272-290 - read_record opens without O_NOFOLLOW/O_NONBLOCK after a symlink_metadata check, so a racing FIFO swap can block the probe and the comment's symlink claim overstates (pre-existing, inherited, derived-not-executed) - closure: open with custom_flags(O_NOFOLLOW|O_NONBLOCK) and keep the fstat check, or reword the comment and limit the no-wait claim to the guard
SUMMARY: At f0738119 the revision-4 identity probe does what the contract says: it accepts under the same four rules as the query hit path, never builds, writes back, deletes, initializes the lock boundary, or waits for the lock, and turns every discarded or missing record into absence; `server.rs` is its only caller and the checker enforces that. The section checker PASSes (exit 0), and `cargo test -p sley-repo --lib index_cache` passes 9/9, including the new probe test that covers all four §8 revision-4 evidence items. Two P3 findings remain: the profile's preamble (and three companion documents) still limit cache hits to query surfaces, and the completion gate is not tied to revision-4 lane verdicts. Three P4 notes cover the gate's scan coverage, the v2-versus-v2+ selection wording, and a pre-existing symlink/FIFO open race in read_record.
