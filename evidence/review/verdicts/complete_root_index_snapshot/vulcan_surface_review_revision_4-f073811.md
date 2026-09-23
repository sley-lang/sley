<!-- engine: claude-code; observed model: claude-opus-5-5; scope: f073811914297505803f5b731cd4691e1623de84; role: vulcan; field: vulcan_surface_review_revision_4; dispatched: 2026-09-23T09:23:09Z; duration_s: 390; process_exit_code: 0 -->
# Vulcan Council review — complete_root_index_snapshot

Harness: claude-code
Reviewed checkpoint: f073811914297505803f5b731cd4691e1623de84

What I checked myself:
- **Scope check:** `git rev-parse HEAD` returned `f073811914297505803f5b731cd4691e1623de84`, so the scope matches. `git status --short` shows four untracked verdict files and no tracked changes. I did not open the untracked Ariadne or Nabu revision-4 verdicts. A repo-wide grep for `cached_complete_root_snapshot_id` printed one-line fragments of them, and I relied on none of it.
- **Git diffs:**
  - `git diff --stat ab42a3a9..HEAD -- docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md crates/sley-repo scripts/`: 10 files, +161/−26.
  - Full diff of `crates/sley-repo`, the spec, and `scripts/check_complete_root_index_snapshot_profile.py`.
  - Full diff of the other seven changed scripts (`check_cli_contract.py`, `check_session_handle_profile.py`, `check_sley2_trial_runner.py`, `check_smp1_contract.py`, `check_smp1_json_bridge_contract.py`, `generate_smp1_json_bridge_table.py`, `test_smp1_contract.py`).
- **Checkers and tests:**
  - `python3 scripts/check_complete_root_index_snapshot_profile.py`: exit 0, `"result": "PASS"`, `"problems": []`, `"revision": 4`, `"status": "S20_300_FULL_IMPLEMENTED_REVIEW_PENDING"`.
  - `cargo test --locked --offline -p sley-repo --lib index_cache`: exit 0, `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 412 filtered out`. This includes `probe_reports_only_a_materialized_snapshot_and_never_builds` and `guard_for_another_repository_is_refused`.
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: exit 0, `2 passed; 0 failed; 121 filtered out`. The two tests are `workspace_open_v2_discloses_only_the_materialized_head_snapshot` and `workspace_open_under_version_1_never_carries_field_9`.
- **In-memory negative probes of the section checker:** I patched the module's `read` for the summary, and `pathlib.Path.rglob`/`read_text` for fake sources. Nothing was written to the tree.
  - Baseline: PASS.
  - Status flipped to `S20_300_FULL_COMPLETE` with `implementation_complete: true` and no revision-4 review fields: **exit 0, PASS**.
  - Control, base `vulcan_surface_review` set to FAIL: exit 1, `completion-without-review:vulcan_surface_review`.
  - Aliased import `use sley_repo::cached_complete_root_snapshot_id as peek; peek(a, b, c)` in a new `crates/sley-cli/src/zz_probe_alias.rs`: **exit 0, PASS**.
  - Control, direct call in a new file: exit 1, `probe-caller:crates/sley-cli/src/zz_probe_direct.rs`.
  - Direct call in `crates/sley-protocol/src/index_cache.rs`: **PASS**.
  - Direct call in `crates/sley-protocol/src/tests/helper.rs`: **PASS**.
- **Machine summary:** dumped `machine-summary.json` → `complete_root_index_snapshot`. Status is `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING` and `contract_revision` is 4. The base `ariadne_contract_review`, `nabu_architecture_review` and `vulcan_surface_review` all read `PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4`, the revision-3 history. No key names revision 4 except `contract_revision`, and `status_note` says "the revision 4 Ariadne, Nabu, and Vulcan reviews have not run".
- **Files read:**
  - `crates/sley-repo/src/index_cache.rs:1-420`
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:110-254`
  - `scripts/check_complete_root_index_snapshot_profile.py:1-233`
  - `crates/sley-protocol/src/server.rs:1175-1224`, `2425-2484`, `2681-2849`, `2860-2970`
  - `crates/sley-protocol/src/server_tests.rs:7380-7461`
  - `crates/sley-txn/src/maintenance.rs:1-260`
  - `crates/sley-txn/src/repository.rs:878-897`
  - `crates/sley-query/src/snapshot.rs:457-471`, `575-774`
  - `crates/sley-repo/src/lib.rs` (grep: `pub use index_cache::*;` at line 24)
  - `docs/spec/SMP1.md:305-329`, `690-719`
  - `docs/adr/ADR-0029-complete-root-index-snapshot-boundary.md:33-45`
  - `scripts/test_current_contract_review.py:1-90`
  - `scripts/check_smp1_contract.py:170-209`
  - `scripts/check_sley2_trial_runner.py:300-301`

## Evidence checked

**The probe cannot build, write or delete.** `cached_complete_root_snapshot_id` (index_cache.rs:256-270) does four things: the guard check, `index_cache_path`, `read_record` (a read-only `File::open` plus `take(MAX+1)`, 277-291), and `accept_cached(...).ok()`. It never reaches `fresh_snapshot`, `write_record`, `create_dir_all` or `remove*`. The test at index_cache.rs:569-604 proves:
- a cold call leaves no file behind;
- the identity equals the one the query path built;
- a hit still works after `objects/` is removed;
- a record with one flipped byte comes back as `None` and the file is not rewritten.

**The four acceptance rules match the query hit path exactly.** Both paths call the same `read_record` → `accept_cached` pair (214-216 and 267-269).
- Rule 1: `decode_complete_root_snapshot` pins arm 2 and requires `Some(root)` (snapshot.rs:461-464). The epoch, field-schema hash, claimed root and completeness are checked at 646-662.
- Rule 2: the trailer is compared with `IndexSnapshotId::derive(preimage)` (743). The identity returned comes from that verified trailer (466-469).
- Rule 3: the inventory must line up with `entity_bindings` by identity and count (index_cache.rs:113-124).
- Rule 4: every endpoint must be in the inventory (688-689, 704, 728), and the reverse groups must equal `invert_edges(direct)` (746).

**No stale or discarded identity reaches field 9.**
- The cache file is keyed by `revision.state_root().root`. A record for another root, epoch or arm fails rule 1, and a changed inventory fails rule 3. Any discard becomes `None`.
- `workspace_open` probes the same `head` it encodes in fields 1-8 (server.rs:2906-2912), so the disclosed identity always belongs to the root in the same response, even if the head moves afterwards.
- If a forged record is accepted (the documented residual), the identity is only a pointer. `query.root` re-binds it against the snapshot it actually serves (server.rs:2687-2700). `capsule` builds fresh and refuses a mismatched preimage with `QUERY_SNAPSHOT_MISMATCH` (2761-2771), so exported evidence can never rest on it.

**Lock behaviour.**
- The probe acquires no lock; it takes its guard as a parameter. Its only error is a guard naming another repository, which the tests cover (index_cache.rs:561-565).
- The sanctioned caller uses `acquire_shared_repository_maintenance_nonblocking` (server.rs:2927 → maintenance.rs:142-146, 180-186). That path opens the existing lock file without `create`, and every failure becomes absence through `.ok()?`. It never calls `initialize_repository_maintenance`, unlike `self.maintenance()` (server.rs:2443-2447).
- Loading the head beforehand already takes a blocking shared lock (repository.rs:883), as the comment at server.rs:2920-2922 says.

**Documents and budget.**
- ADR-0029:39-41 was updated for the second reader.
- The spec's required-evidence list at lines 233-235 matches the new test.
- A v2 `workspace.open` is charged one dispatch unit plus response bytes (server.rs:1191-1210), the same model as a `query.root` hit, so the probe adds no new amplification class.

**Rule parity in the sister checker.** In this same diff range, `check_sley2_trial_runner.py:300-301` gained `completion-unbound-review`, which binds `<lane>_revision_<N>`. The S20-300 checker got no equivalent.

## Findings
[P2] [evidence-integrity] scripts/check_complete_root_index_snapshot_profile.py:207-210 - The completion gate only requires the base `ariadne_contract_review`/`nabu_architecture_review`/`vulcan_surface_review` fields to start with PASS. Those fields hold the revision-3 PASS values, which `status_note` itself calls history. As a result, flipping the section to `S20_300_FULL_COMPLETE` with `implementation_complete: true` and no revision-4 review passes the checker (in-memory probe: exit 0, `"result": "PASS"`). The gate enforces nothing for the current revision, while the sister checker changed in this same range (`check_sley2_trial_runner.py:300-301`) does bind `<lane>_revision_<N>`. - Closure evidence needed: at `COMPLETE_STATUS`, bind each lane to `<lane>_revision_{SPEC_REVISION}` (or to a `current_delta_review` whose `contract_revision == SPEC_REVISION`, all PASS), and add a negative test showing that a COMPLETE flip backed only by stale base PASS fields is refused.
[P3] [gate-bypass] scripts/check_complete_root_index_snapshot_profile.py:178-187 - The probe-caller allowlist matches the literal text `cached_complete_root_snapshot_id(` file by file. It misses an aliased import (`... as peek; peek(...)`, demonstrated: PASS). It exempts every file named `index_cache.rs` under `crates/` (demonstrated with `crates/sley-protocol/src/index_cache.rs`: PASS) and every path with a `tests` component (demonstrated with `crates/sley-protocol/src/tests/helper.rs`: PASS). It allowlists all of `server.rs`, so the non-blocking, no-initialize guard at the sanctioned call site (server.rs:2927), which the spec relies on at lines 177-180, is pinned by neither the checker nor a test: server_tests.rs:7393-7461 has no case with the boundary held exclusively. - Closure evidence needed: match the identifier itself, not `name(`; exempt only `crates/sley-repo/src/index_cache.rs` by full path and only real test locations; pin `acquire_shared_repository_maintenance_nonblocking` at the call site, or add a test calling `materialized_head_snapshot` while an exclusive guard is held that shows a prompt `None`; add negative checker tests for the three bypasses above.
[P3] [toctou-availability] crates/sley-repo/src/index_cache.rs:272-291 - `read_record` checks the path with `symlink_metadata` (278) and then calls `File::open` (282). That open follows symlinks and opens FIFOs without `O_NONBLOCK`. A local writer can swap a FIFO, or a symlink to one, into `index/v1/<root>.idx.scb1` inside that window. The probe then blocks in open(2) indefinitely while holding the shared maintenance guard (server.rs:2927), which hangs v2 `workspace.open` and starves any exclusive owner (GC or import). The comment at 272-276 is also inaccurate: a swap *can* redirect through a planted symlink, because the check after opening (283) only rejects targets that are not regular files. This is a static trace, not executed. The same window already existed on the query path (214) since revision 3; revision 4 extends it to the opener. - Closure evidence needed: open with `OpenOptionsExt::custom_flags(O_NOFOLLOW | O_NONBLOCK)`, check `is_file` on the open handle before reading, correct the comment, and add a unit test with a FIFO/symlink at the cache path showing a prompt absence from both the probe and `complete_root_snapshot`.
[P4] [doc-accuracy] crates/sley-repo/src/index_cache.rs:245-246 - The claim "the caller's cost class stays metadata-only" overstates. Each probe does the same full decode as a query hit: it reads up to `MAX_SNAPSHOT_RECORD_BYTES` = 67,108,864 bytes (snapshot.rs:29), hashes the preimage with BLAKE3 (743), and inverts up to 400,000 edges (746). That runs on every v2 `workspace.open`. The module doc at index_cache.rs:4-8 also still says the cache serves only the derived query surfaces. The spec states the bound accurately (COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:169-170). - Closure evidence needed: reword both comments to the spec's bound (at most one cache file, bounded decode, no object reads).

## Assessment
The probe does what revision 4 says:
- It is exactly the cheap half of `complete_root_snapshot`. It never builds, writes back or deletes, and it acquires no lock itself.
- Its acceptance rules are identical to the query hit path because it calls the same two functions.
- Any discard, absence or non-file becomes `None`.
- The identity it discloses always belongs to the root reported in the same response. It only ever acts as a pointer: `query.root` re-binds it, and `capsule` refuses it unless a fresh build agrees.

The section checker passes, and both bounded test runs pass (sley-repo 9/9, sley-protocol 2/2).

Acceptance is blocked by the section's own gate. At revision 4 the completion check can be satisfied by revision-3 verdicts, and I demonstrated that in memory. The sister trial-runner checker was fixed for exactly this in the same range, so the omission here is an evidence-integrity gap, not a style point.

The probe-caller allowlist can be bypassed by an aliased import and by two broad exemptions. The pre-existing lstat→open window in `read_record` is now reachable from the session opener. Both are minor under the spec's local-filesystem trust boundary but should be closed.

Outside this lane and not counted as a finding: SMP1.md:703-704 says field 9 is the identity `capsule` binds. Under the documented forged-record residual, `capsule` binds the fresh identity instead. The S20-300 text at 180-182 already states this correctly, and the protocol lane may want to align the SMP1 wording.

I make no claim about GA or release readiness.

VERDICT: REVISE_0_P0_0_P1_1_P2_2_P3_1_P4
SECTION: complete_root_index_snapshot
FIELD: vulcan_surface_review_revision_4
SCOPE_SHA: f073811914297505803f5b731cd4691e1623de84
FINDINGS: [P2] [evidence-integrity] scripts/check_complete_root_index_snapshot_profile.py:207-210 - completion gate binds only the base lane fields, which hold revision-3 PASS history, so a COMPLETE flip with no revision-4 review passes (in-memory probe exit 0 PASS), unlike check_sley2_trial_runner.py:300-301 in the same range - bind <lane>_revision_{SPEC_REVISION} (or current_delta_review at SPEC_REVISION, all PASS) at COMPLETE_STATUS plus a negative test refusing a stale-base flip; [P3] [gate-bypass] scripts/check_complete_root_index_snapshot_profile.py:178-187 - probe-caller allowlist is a literal `name(` text match: an aliased import, any file named index_cache.rs, and any path with a `tests` component all pass (demonstrated), and the non-blocking no-initialize guard at server.rs:2927 is pinned by neither checker nor test - match the identifier, exempt only the defining file by full path and real test locations, pin or test the non-blocking guard, add negative checker tests; [P3] [toctou-availability] crates/sley-repo/src/index_cache.rs:272-291 - the lstat→File::open window follows swapped-in symlinks and blocks on a swapped-in FIFO while the shared guard is held (static trace), which hangs v2 workspace.open and starves exclusive owners, and the comment at 272-276 overclaims - open with O_NOFOLLOW|O_NONBLOCK, check is_file on the handle, fix the comment, add a FIFO/symlink test; [P4] [doc-accuracy] crates/sley-repo/src/index_cache.rs:245-246 - "cost class stays metadata-only" overstates a full decode of up to 64 MiB/400,000 edges per v2 workspace.open, and the module doc at 4-8 omits the probe - reword both to the spec's bound at lines 169-170
SUMMARY: The revision-4 identity probe is sound: it cannot build, write, delete or acquire locks, it uses the same four acceptance rules as the query hit path through shared code, and it discloses only an identity belonging to the disclosed head, with capsules still building fresh. The section checker passes, and the sley-repo index_cache (9/9) and sley-protocol workspace_open (2/2) tests pass. Acceptance is blocked because the checker's completion gate is satisfied by revision-3 PASS history, demonstrated in memory. Minor fixes remain for the bypassable probe-caller allowlist, the read_record FIFO/symlink window now reachable from the opener, and an overstated cost comment.
