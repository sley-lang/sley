<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: vulcan; field: vulcan_surface_review_revision_5; dispatched: 2026-09-23T12:12:54Z; duration_s: 307; process_exit_code: 0 -->
# Vulcan Council review — complete_root_index_snapshot
Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

What I checked myself:
- **Scope check.**
  - `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, so the scope matches. The branch is `work/succ-context-impl`.
  - `git status --short` shows nine untracked verdict files and no tracked changes.
  - I did not open the untracked Ariadne and Nabu revision-5 verdicts.
- **Git.**
  - `git diff --stat f0738119..HEAD` over the four scoped paths: 5 files, +249/−52.
  - I read the full diff of `crates/sley-repo`, `Cargo.lock`, the spec and `scripts/check_complete_root_index_snapshot_profile.py`.
  - `git log f0738119..HEAD -- scripts/test_complete_root_index_snapshot_profile.py` shows the file was added in `40a84a5c`.
  - `git diff --stat f0738119..HEAD -- '*Cargo.lock'` shows both `Cargo.lock` and `fuzz/Cargo.lock` gained one line each.
- **Checkers and tests.** All ran in the foreground.
  - `python3 scripts/check_complete_root_index_snapshot_profile.py`: exit 0, `"result": "PASS"`, `"problems": []`, `"revision": 5`, `"status": "S20_300_FULL_IMPLEMENTED_REVIEW_PENDING"`.
  - `python3 -m unittest scripts/test_complete_root_index_snapshot_profile.py -v`: exit 0, `Ran 13 tests ... OK` (4 CompletionBinding, 9 ProbeGate).
  - `cargo test --locked --offline -p sley-repo --lib index_cache`: `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 412 filtered out`. This covers all 11 `index_cache::tests`, including `a_symlink_to_a_valid_record_is_absence_for_the_probe`, `a_fifo_at_the_cache_path_answers_at_once` and `a_non_canonical_repository_spelling_is_covered_by_its_guard`, plus the exchange purge test.
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: `4 passed; 0 failed; 121 filtered out`. This includes `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`.
- **Negative probes of `probe_gate_problems`.** I imported the checker module and ran it on `tempfile` scratch trees holding copies of the definition and consumer files. Nothing was written to the tree.
  - Baseline: `[]`.
  - Control, a direct call in `crates/sley-cli/src/x.rs`: `['probe-caller:crates/sley-cli/src/x.rs']`.
  - Block-comment mention in another crate: refused. The gate fails closed here.
  - **Bypasses, each returning `[]`:**
    - **(A)** `let _u = "http://x"; let g = sley_repo::cached_complete_root_snapshot_id; g(a, b, c);` on one line in `crates/sley-cli/src/x.rs`.
    - **(B)** `use sley_repo::cached_complete_root_snapshot_id/**/as peek;` in `server.rs`, plus `fn leak() { let _ = peek; }`.
    - **(B2)** `pub(crate) use ...cached_complete_root_snapshot_id/**/as peek;` in `server.rs`, plus `crate::server::peek(a, b, c)` in `crates/sley-protocol/src/other.rs`.
    - **(C)** `fn leak() { drop(("use ", cached_complete_root_snapshot_id)); }` appended to `server.rs`.
    - **(D)** A decoy non-waiting acquisition followed by a blocking `acquire_shared_repository_maintenance(&self.repository)` inside `materialized_head_snapshot`. The runtime test at server_tests.rs:7567-7570 catches this, so it is not counted.
- **Machine summary.** I dumped `complete_root_index_snapshot`:
  - `contract_revision` = 5, `status` = `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`, `implementation_complete` = false.
  - The `*_revision_4` fields record the prior round.
  - No `*_revision_5` fields are present, and `status_note` says "The revision 5 reviews have not run".
  - Makefile lines 24 and 101 run the checker and its test file.
- **Files read:**
  - `evidence/review/verdicts/complete_root_index_snapshot/vulcan_surface_review_revision_4-f073811.md:1-101`
  - `crates/sley-repo/src/index_cache.rs:1-420`, plus the new tests at 637-724 (from the diff)
  - `crates/sley-txn/src/maintenance.rs:1-240`
  - `crates/sley-txn/src/repository.rs:2730-2760`, plus a grep of its shared acquisitions (829, 883, 920)
  - `crates/sley-protocol/src/server.rs:2880-2949` and 2453-2460, plus a grep of 399/506/545
  - `crates/sley-protocol/src/server_tests.rs:7536-7585`
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:105-264`
  - `scripts/check_complete_root_index_snapshot_profile.py:89-166` (from the diff) and `168-306`
  - `scripts/test_complete_root_index_snapshot_profile.py:1-188`
  - `scripts/check_sley2_trial_runner.py:296-301`
  - `Cargo.toml:3-33`, `Cargo.lock:166-169`, `fuzz/Cargo.lock:166-168` and `408-420`
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: a grep, with rows 686-692 read

## Evidence checked

**Revision-4 P2, completion bound to the current revision: CLOSED.**
- At `COMPLETE_STATUS`, check_complete_root_index_snapshot_profile.py:280-284 now also requires `section[f"{key}_revision_{SPEC_REVISION}"]` to start with `PASS`, with `SPEC_REVISION = 5` at line 24. This is the same rule as the sister checker (check_sley2_trial_runner.py:296-301).
- test_complete_root_index_snapshot_profile.py:72-99 proves three cases:
  - a COMPLETE flip backed only by the revision-3 base PASS values is refused with `completion-unbound-review:<lane>` ×3;
  - a flip backed by `_revision_4` fields is refused;
  - a flip with bound `_revision_5` PASS values is admitted.
- These tests serve the summary from memory, and I ran them (OK).

**Revision-4 P3, probe-caller gate: CLOSED.** I checked each item of the closure evidence I asked for:
- The gate matches the bare identifier (`\bcached_complete_root_snapshot_id\b`, line 131).
- It exempts the definition file only by full path (line 137) and only `crates/<crate>/tests/` (line 139).
- It scans `fuzz/` too (line 132).
- It refuses an alias (line 148).
- It requires exactly one reference outside `use` items, located inside `materialized_head_snapshot` (lines 150-155).
- It pins `acquire_shared_repository_maintenance_nonblocking(` and forbids `initialize_repository_maintenance` and `self.maintenance()` in that body (lines 156-161).
- Negative tests cover the three bypasses I demonstrated at revision 4 (129-145), plus fuzz, a second consumer reference, a consumer-side alias and a waiting consumer (152-184).
- A runtime pin also exists: server_tests.rs:7567-7570 takes an exclusive guard and requires `materialized_head_snapshot` to return `None` within 5 s. Separate `flock` open file descriptions conflict within one process, so a blocking shared acquisition there would hang the test. That covers bypass (D) above.

The remaining text-level bypasses are narrower and need deliberately odd code. They are recorded as a new P4 below.

**Revision-4 P3, the lstat→open window: CLOSED.**
- `read_record` (index_cache.rs:283-300) no longer has a `symlink_metadata` pre-check. It opens once with `custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)` under `#[cfg(unix)]`, requires `is_file` on the open handle (292), and keeps the `take(MAX_SNAPSHOT_RECORD_BYTES + 1)` cap (296).
- On Linux, `O_NOFOLLOW` fails a final-component symlink with `ELOOP`. `O_RDONLY|O_NONBLOCK` on a FIFO returns immediately and then fails the handle `is_file` check. A socket fails with `ENXIO`. Every such case is `None`.
- The comment at 276-282 now describes this accurately.
- Tests at 640-685 show a symlink to a byte-identical valid record and a FIFO with no writer. Both answer `None` from the probe and `Err` from `complete_root_snapshot`, and both ran to completion.
- The query path still fails closed on a non-file (224-231). `verify_cached_snapshot` reports a present non-file as `Mismatch` (340-346).
- libc is used only for two constants, so the workspace-wide `unsafe_code = "forbid"` (Cargo.toml:33) is unaffected. `libc =0.2.189` is pinned in `crates/sley-repo/Cargo.toml:11` and was already resolved at 0.2.189 in both lockfiles, which each gained only the `sley-repo → libc` edge.

**Revision-4 P4, cost wording: CLOSED.**
- index_cache.rs:243-250 now states "at most one cache file is read (bounded by `MAX_SNAPSHOT_RECORD_BYTES`) and bounded-decoded ... the decode still verifies the record's digest and edge inversion". This matches spec 183-185.
- The module doc (4-10) names the probe.

**Guard coverage (new in revision 5).**
- All three cache entry points now use `guard.covers(repository)` (210, 265, 327).
- `covers` (maintenance.rs:35-37) calls `canonical_real_directory`. That refuses a final-component symlink or a non-directory, then canonicalizes and compares with the guard's canonical root.
- A different repository is still refused (`guard_for_another_repository_is_refused`).
- A parent-symlink spelling and a `..` spelling are admitted, and a final-component symlink is refused (index_cache.rs:694-723).
- The production consumer keeps the caller's spelling (server.rs:399, 506, 545 store `repository.into()`). Before this change, that raw-equality check turned field 9 into a silent absence for any non-canonical path, so the change is a correctness repair. Its check-then-use residual is recorded as a new P4 below.

**Probe semantics unchanged.**
- `cached_complete_root_snapshot_id` (260-274) is still only `read_record` → `accept_cached(...).ok()`, with no `fresh_snapshot`, write or remove.
- The server test at 7572-7584 shows that a record with one flipped byte gives an eight-field `workspace.open` and that the file bytes stay unchanged.
- `workspace_open_v3_answers_the_version_2_open_summary` passes. That is consistent with the spec 191-194 wording that names the consumer for version 2 and version 3.

## Findings
[P4] [gate-bypass] scripts/check_complete_root_index_snapshot_profile.py:96-97,141-155 - The gate strips comments with `//[^\n]*` and `use` items with `\buse\s[^;]*;` on raw text, and it detects aliases with `PROBE\s+as\b`. None of these steps understands string literals or block comments. As a result, a same-line `"http://x"` string hides a function-pointer reference in any crate (A). An alias written `cached_complete_root_snapshot_id/**/as peek` escapes the alias check, including as a `pub(crate) use` re-export from server.rs that other files call as `crate::server::peek` (B, B2). A string such as `"use "` followed by the identifier with no `;` in between hides a second reference in the consumer (C). All four return `[]` on scratch trees, yet spec lines 201-206 say any other reference "(a call, an import, an alias, a function pointer) ... fails the gate". The impact is bounded because the identity is only a pointer and each construction is deliberate. - Closure evidence needed: tokenize before matching (drop string/char literals and block comments, then match identifier tokens and `as` token-wise), and add negative tests for A, B/B2 and C.
[P4] [toctou-note] crates/sley-repo/src/index_cache.rs:210-215,265-270,327-333 - `covers` canonicalizes `repository` once, but the cache path is then joined onto the caller's non-canonical spelling (`index_cache_path(repository, ...)`), and the consumer passes that spelling (server.rs:506/545). Before revision 5, the path used was byte-equal to the guard's canonical root. Now, retargeting a parent symlink in the caller's spelling between the check and the open or write-back aims the read, or the `create_dir_all` and rename in `complete_root_snapshot`, at a directory the guard does not cover. This is inside the local-filesystem trust boundary and is not executed here. - Closure evidence needed: after `covers` succeeds, derive the cache path from `guard.repository_root()`, and extend `a_non_canonical_repository_spelling_is_covered_by_its_guard` to assert the path is rooted there.
[P4] [doc-accuracy] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:119-121,201-206,258-262 - Three statements overclaim:
  - (a) "a relative or otherwise non-canonical spelling of the same repository is covered". A final-component symlink spelling is refused by `covers`, and index_cache.rs:720-722 asserts that.
  - (b) "Any other reference to the identifier ... in `crates/` ... fails the gate". The checker exempts `crates/<crate>/tests/` without limit (checker line 139; `test_crate_integration_tests_are_exempt`).
  - (c) "through `workspace.open`, absence while an exclusive maintenance owner holds the boundary". The test calls `materialized_head_snapshot` directly (server_tests.rs:7567-7570). Through `workspace.open`, the S20-390 head load blocks under an exclusive owner (repository.rs:883), as the spec itself says at 196-198.
  - Closure evidence needed: reword each of the three statements to match the code and the tests.

## Assessment
Revision 5 answers every revision-4 finding from my lane, and I checked each closure against code, tests and checker output:
- **P2:** completion is now bound by field name to `<lane>_revision_5` PASS, with negative tests.
- **P3, probe-caller gate:** the gate matches the identifier and exempts only the defining file by full path and crate integration tests. It pins the non-waiting acquisition at the call site, and a runtime test with an exclusive owner pins it again.
- **P3, lstat→open window:** there is a single `O_NOFOLLOW|O_NONBLOCK` open with an `is_file` check on the handle, and symlink and FIFO tests that ran promptly.
- **P4, cost wording:** the documentation now states the actual cost.

The new `covers` relaxation is a real repair for non-canonical server paths, and it still refuses other repositories and final-component symlinks. The probe still cannot build, write or delete.

The section checker passes (exit 0), its 13 regression tests pass, sley-repo `index_cache` passes 12/12 and sley-protocol `workspace_open` passes 4/4.

Three residuals remain, all P4:
- the text gate can still be fooled by deliberately constructed literals and comments;
- a small check-then-use divergence in path spelling that `covers` introduced;
- three spec sentences that say slightly more than the code does.

None blocks acceptance.

Outside this scope and not counted: the gate record's row 688 says "comments and `use` items stripped" and "Eight negative tests". In fact only `//` comments are stripped, and the ProbeGate class has seven negative and two positive cases.

I make no claim about GA or release readiness.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_3_P4_PRIOR_P2_P3_P4_CLOSED
SECTION: complete_root_index_snapshot
FIELD: vulcan_surface_review_revision_5
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P4] [gate-bypass] scripts/check_complete_root_index_snapshot_profile.py:96-97,141-155 - the regex comment/`use` stripping and `\s+as` alias check ignore string literals and block comments, so a same-line `"http://x"` hides a reference in any crate, `/**/as peek` escapes the alias check (including a pub(crate) re-export from server.rs), and a `"use "` string hides a second consumer reference (all demonstrated `[]` on scratch trees), contrary to spec 201-206 - tokenize (drop literals/block comments, match identifier and `as` tokens) and add negative tests for each; [P4] [toctou-note] crates/sley-repo/src/index_cache.rs:210-215,265-270,327-333 - `covers` canonicalizes once but the cache path is joined onto the caller's non-canonical spelling (kept by server.rs:506/545), so a parent symlink retargeted between check and use aims the read or write-back outside the guarded repository (static trace, local trust boundary) - derive the path from guard.repository_root() after covers and assert it in the spelling test; [P4] [doc-accuracy] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:119-121,201-206,258-262 - "otherwise non-canonical spelling is covered" (a final-component symlink is refused), "any other reference in crates/ fails the gate" (crate tests/ are exempt), and "through workspace.open, absence while an exclusive owner holds the boundary" (the test calls materialized_head_snapshot directly; workspace.open blocks at the S20-390 head load) each overclaim - reword the three statements
SUMMARY: S20-300 revision 5 closes all four revision-4 Vulcan findings, each verified in code and tests: the completion gate binds to `<lane>_revision_5`, the probe gate matches the identifier and pins the non-waiting consumer, cache files open once with O_NOFOLLOW|O_NONBLOCK and an is_file check on the handle, and the cost wording is accurate. The section checker passes (exit 0), its 13 regression tests pass, sley-repo index_cache passes 12/12 and sley-protocol workspace_open passes 4/4. Three P4 residuals remain: deliberate literal and comment tricks still get past the text gate, the path spelling can diverge between the `covers` check and its use, and three spec sentences overclaim. None blocks acceptance.
