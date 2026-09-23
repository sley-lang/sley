<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: vulcan; field: vulcan_surface_review_revision_6; dispatched: 2026-09-23T13:54:21Z; duration_s: 422; process_exit_code: 0 -->
# Vulcan Council review — complete_root_index_snapshot
Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

What I checked myself:
- **Scope check.**
  - `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, so the scope matches. The branch is `work/succ-context-impl`.
  - `git status --short` shows ten untracked verdict files from other packages and no tracked changes.
  - `df -i /tmp` shows 10% of inodes in use.
- **Git.**
  - `git diff --stat 2b0f1c9f..HEAD` touches 86 files.
  - I read the full diff for this package's paths: `scripts/check_complete_root_index_snapshot_profile.py`, `scripts/test_complete_root_index_snapshot_profile.py`, `crates/sley-repo/src/index_cache.rs` and `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`.
  - I also read the consumer's diff in `crates/sley-protocol/src/server.rs`. `crates/sley-protocol/src/lib.rs` has no diff.
  - `index_cache_path(` appears only at index_cache.rs:248, 306 and 378 in production code, each with `guard.repository_root()`. The other hits are tests.
- **Checkers and tests.** All ran in the foreground.
  - `python3 scripts/check_complete_root_index_snapshot_profile.py`: exit 0, `"result": "PASS"`, `"problems": []`, `"revision": 6`, `"status": "S20_300_FULL_IMPLEMENTED_REVIEW_PENDING"`, `"new_stable_error_codes": 3`.
  - `python3 -m unittest scripts/test_complete_root_index_snapshot_profile.py -v`: `Ran 21 tests … OK`. That is 4 CompletionBinding, 9 ProbeGate and 8 TokenAwareGate tests.
  - `cargo test --locked --offline -p sley-repo --lib index_cache`: `13 passed; 0 failed; 0 ignored; 412 filtered out`. This includes the new `a_symlinked_cache_directory_is_never_read_or_written_through` and the updated `a_non_canonical_repository_spelling_is_covered_by_its_guard`.
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: `5 passed; 0 failed; 123 filtered out`. This includes `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record` and `workspace_open_answers_from_the_single_checked_head_load`.
- **In-memory probes of `probe_gate_problems`.** I imported the checker and ran it on `tempfile` scratch trees holding copies of index_cache.rs, server.rs and lib.rs. Nothing was written to the tree. Twelve cases:
  - Baseline: `[]`.
  - The four bypasses from my revision-5 transcript are all refused now:
    - **A**: `['probe-caller:crates/sley-cli/src/x.rs']`
    - **B**: `['probe-caller:crates/sley-protocol/src/server.rs:aliased']`
    - **B2**: `[…server.rs:aliased]`
    - **C**: `['probe-caller:crates/sley-protocol/src/server.rs:references=2:in-consumer=1']`
  - Control: the name inside a `c"…"` string gives `[]`, which is correct.
  - Five new constructions (N1–N5) each return `[]`. They are listed in the findings.
- **rustc checks.** I compiled standalone files with rustc 1.93.0, edition 2024, in a temporary directory:
  - The N1 shape (raw C string) and the N2 shape (nested block comment with a quote) both compile, and both functions call the probe at run time.
  - A mirror of `covers` (`symlink_metadata` then `canonicalize`) printed `/alias covered=false`, `/alias/ covered=true` and `/alias/. covered=true`.
- **Machine summary.** In the `complete_root_index_snapshot` section:
  - `contract_revision` = 6, `status` = `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`, `implementation_complete` = false.
  - The `*_revision_5` fields hold the round-7 verdicts, with dated notes.
  - No `*_revision_6` fields are present.
  - My revision-5 transcript's sha256 `c0555ac6…ce716` matches `reviews/5/sha256` in `evidence/review/rounds/context-r7-2b0f1c9.json`.
- **Files read:**
  - `evidence/review/verdicts/complete_root_index_snapshot/vulcan_surface_review_revision_5-2b0f1c9.md:1-133`
  - `scripts/check_complete_root_index_snapshot_profile.py:85-234`
  - `scripts/test_complete_root_index_snapshot_profile.py:1-110`, plus the diff at 184-264
  - `crates/sley-repo/src/index_cache.rs:150-409`, plus the test diff at 762-827
  - `crates/sley-txn/src/maintenance.rs:23-37` and `195-217`
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:118-237`, plus the diff at 1-26 and 281-291
  - `crates/sley-protocol/src/server_tests.rs:7544-7604`
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:784-963`
  - `Cargo.toml:1-40` and `rust-toolchain.toml`

## Evidence checked

**Prior finding 1 — [P4] [gate-bypass] check_complete_root_index_snapshot_profile.py:96-97,141-155 (literal and comment bypasses A, B, B2, C): CLOSED.**
- `strip_rust` (checker:109-130) blanks line and block comments, plain, byte and raw strings, and char literals before any match. Newlines are kept, so `/**/as` reads as `as` (196-199).
- I re-ran all four demonstrated bypasses against HEAD, and each is refused (results above).
- New regression tests `TokenAwareGate`:
  - `test_a_url_string_does_not_hide_a_same_line_reference` covers A.
  - `test_a_block_comment_does_not_hide_an_alias` covers B and the B2 `pub(crate) use` alias.
  - `test_a_use_string_does_not_hide_a_second_reference` covers C.
- All pass. The literal and inclusion forms the blanker still misses are a new residual (finding 1 below). They are not the demonstrated bypasses.

**Prior finding 2 — [P4] [toctou-note] index_cache.rs:210-215,265-270,327-333 (cache path joined onto the caller's spelling): CLOSED.**
- All three entry points now derive the path from `guard.repository_root()` after `covers`: `complete_root_snapshot` at 248, `cached_complete_root_snapshot_id` at 306 and `verify_cached_snapshot` at 378.
- `guard.repository_root()` returns the canonical root (maintenance.rs:23-25).
- No production call joins `repository`.
- The spelling test now covers a relative spelling and asserts at 779 that the record is under `guard.repository_root()`.
- That assertion cannot tell old code from new without a race seam, because both spellings resolve to the same file. The code change is the evidence here, and I verified it at all three sites.

**Prior finding 3 — [P4] [doc-accuracy] spec 119-121,201-206,258-262 (three overclaims): CLOSED.**
- **(a)** Spec 126-130 now lists relative, parent-symlink and `..` spellings as covered, and a final-component symlink as refused. That matches `covers` for the bare spelling. A trailing-separator corner remains (new finding 4).
- **(b)** Spec 216-229 now names the `crates/<crate>/tests/` exemption, the consumer's own-name `use` import, and the wrapper rule. That matches `_exempt`/`probe_gate_problems` (152-220).
- **(c)** Spec 281-287 now attributes the exclusive-owner absence to the wrapper `materialized_head_snapshot`, notes that "`workspace.open` itself waits at the S20-390 head load", and puts only the unchanged discarded record through `workspace.open`. That is exactly server_tests.rs:7568-7571 (direct wrapper call, under 5 s) and 7573-7585 (`WorkspaceOpen`, 8 fields, bytes unchanged).

**New in revision 6, checked.**
- **Wrapper gate.** `materialized_head_snapshot` must appear exactly twice in server.rs outside `use` items: its definition, plus one call inside `workspace_open` (checker:210-216). Other files are refused unless exempt. The four wrapper tests pass.
- **Waiting in the probe consumer.** `BLOCKING` (103-106) now refuses a blocking shared acquire, an exclusive acquire, initialization and `.maintenance()` in the consumer body, with a test.
- **Directory components.** `cache_directories_are_real` (index_cache.rs:157-173) is applied before a read (323) and before and after `create_dir_all` on write-back (181-191). The new test shows that a symlinked `index/` or `index/v1/` gives probe absence and no write-back.
- **Retained head.** The server's retained-head change passes the same head to the wrapper (server.rs:2942-2947). The wrapper gate still counts one caller.

## Findings
[P4] [gate-bypass] scripts/check_complete_root_index_snapshot_profile.py:101,109-130,152-159,179 - The revision-6 blanker still mis-scans valid Rust. In each case below, a real probe reference returns `[]` on a scratch tree:
  - (N1) A raw C string `cr#"q"b"#` is not matched by `\bb?r…`. Its inner quote opens a false string that blanks the following `let g = sley_repo::cached_complete_root_snapshot_id;`.
  - (N2) A nested block comment `/* /* */ " */` ends at its first `*/`. The exposed `"` then opens a false string over the next reference. This contradicts the `strip_rust` docstring (126-127): "can only expose text, never hide it".
  - (N3) `#[path = "leak.inc"] mod leak;` compiles a non-`.rs` file that the `*.rs` glob never reads.
  - (N4) `#[path = "../tests/support/leak.rs"] mod leak;` in `src/lib.rs` compiles an exempt `crates/<crate>/tests/` file into the production library.
  - (N5) `SERVER_TEST_DECLARATION` is matched against raw lib.rs text. So `// #[cfg(test)] mod server_tests;` next to a real `mod server_tests;` keeps the wrapper exemption while the module compiles unconditionally.
  - N1 and N2 compile and run under rustc 1.93.0, edition 2024. Spec 216-229 says every Rust file is read with literals blanked and that any other reference fails the gate.
  - The impact is bounded: each construction is deliberate, and the probe discloses only a pointer.
  - Closure evidence needed: blank `c`/`cr` (raw) strings and track block-comment nesting; match the cfg declaration against `strip_rust(lib)`; refuse `#[path` and `include!` in `crates/` and `fuzz/`, or scan their targets; add negative tests N1–N5, or narrow the spec sentence to what the gate actually reads.
[P4] [toctou-note] crates/sley-repo/src/index_cache.rs:157-173,181-191,207-214,320-333 - The new `index/` and `index/v1/` check looks up the directories first, then uses them by name.
  - **Write path.** In `write_record`, a component swapped to a symlink after the second check (187) and before `create_new` (207-210) and `rename` (214) redirects the write-back. The sley process then creates a temp file and `<hex>.idx.scb1` in any directory it can write.
  - **Read path.** The read has the same window (323 → 333), bounded by `O_NOFOLLOW|O_NONBLOCK` and `accept_cached`.
  - Spec 133-137 ("a write-back refused") and evidence item 288-290 ("never read or written through") state the protection without that qualification.
  - This is a static trace, not executed, and it needs a writer inside the repository (the local trust boundary).
  - Closure evidence needed: do the work relative to a directory handle with no re-resolution by path (open `index/v1` with `O_DIRECTORY|O_NOFOLLOW` and create and rename relative to it through a safe wrapper), or qualify both spec statements as checked before use, with a concurrent in-repository writer named as the residual.
[P4] [audit-gap] crates/sley-repo/src/index_cache.rs:378-391 - `verify_cached_snapshot` now gets `None` from `read_record` for a symlinked or non-directory `index/` or `index/v1/`.
  - It then falls to `fs::symlink_metadata(&path)`, which follows the intermediate components.
  - So a symlinked `index/` with no record at its target (ENOENT), or `index` as a regular file (ENOTDIR), reports `Missing`. Spec 187-188 calls `Missing` benign.
  - Yet spec 133-137 calls such a component tampering, and the comment at 386-388 says tampering is "never a clean miss".
  - This is a static trace.
  - Closure evidence needed: return `Mismatch` when `!cache_directories_are_real(&path)`, and add both cases to `verify_distinguishes_missing_match_and_mismatch`.
[P4] [doc-accuracy] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:126-130 - "a symlink as the repository's own final component … [is] refused" holds only for the bare spelling.
  - `covers` is `require_real_directory` (lstat) followed by `canonicalize` (crates/sley-txn/src/maintenance.rs:35-37,195-207). lstat follows a final symlink written as `alias/` or `alias/.`.
  - A rustc 1.93 mirror of that logic printed `/alias covered=false`, `/alias/ covered=true` and `/alias/. covered=true`.
  - The test at index_cache.rs:780-782 checks only the bare spelling.
  - There is no safety impact now that the cache path comes from `guard.repository_root()`.
  - Closure evidence needed: qualify the sentence (bare final component), or make `covers` refuse these spellings, and test `alias/`.

## Assessment
Revision 6 closes all three of my revision-5 findings, and I checked each closure against code, tests and probe output:
- **Gate bypasses.** The literal and comment bypasses I demonstrated (A, B, B2, C) are refused by the blanker and each has a regression test.
- **Path spelling.** The cache path comes from the guard's canonical root at all three entry points.
- **Spec wording.** The coverage, gate and evidence sentences now match the code and the server test.

The package also tightened its surface beyond what I asked for:
- the consumer wrapper is pinned to exactly one caller inside `workspace_open`;
- the probe consumer is refused any blocking or exclusive acquisition;
- symlinked `index/` and `index/v1/` directories are refused on read and on write-back.

Results:
- The section checker passes with exit 0 at revision 6.
- Its 21 regression tests pass.
- sley-repo `index_cache` passes 13/13.
- sley-protocol `workspace_open` passes 5/5.
- The machine summary is honestly `REVIEW_PENDING`, with no revision-6 PASS fields yet.

Four P4 residuals remain:
- the regex blanker still misses C raw strings, nested comments and `#[path]` inclusion;
- the new directory-component check has a check-then-use window;
- the audit reports a tampered directory component as a benign miss;
- the final-component refusal depends on how the path is spelled.

None blocks acceptance at contract revision 6. I make no claim about GA or release readiness.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_4_P4_PRIOR_P3_P4_CLOSED
SECTION: complete_root_index_snapshot
FIELD: vulcan_surface_review_revision_6
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P4] [gate-bypass] scripts/check_complete_root_index_snapshot_profile.py:101,109-130,152-159,179 - the regex blanker still mis-scans valid Rust, so a real probe reference returns [] on scratch trees: (N1) raw C string cr#"q"b"# opens a false string over the reference, (N2) a nested block comment exposes a quote that hides the next reference (contradicting strip_rust's "never hide it" docstring), (N3) #[path] to a non-.rs file is never scanned, (N4) #[path] into an exempt crates/<crate>/tests/ file compiles into production, (N5) the cfg(test) server_tests declaration is matched on raw lib.rs text, so a comment satisfies it; N1/N2 compiled and ran under rustc 1.93, contrary to spec 216-229 - blank c/cr strings and nest comments, match the cfg on stripped text, refuse or scan #[path]/include!, add negative tests or narrow the spec; [P4] [toctou-note] crates/sley-repo/src/index_cache.rs:157-173,181-191,207-214,320-333 - the index/ and index/v1/ real-directory check is look-up-then-use by name, so a component swapped to a symlink after the second check and before create_new/rename redirects the write-back (and a read) outside the repository, while spec 133-137 and 288-290 state the protection without qualification (static trace, local trust boundary) - work relative to an O_DIRECTORY|O_NOFOLLOW handle, or qualify both statements with the concurrent-writer residual; [P4] [audit-gap] crates/sley-repo/src/index_cache.rs:378-391 - verify_cached_snapshot reports Missing (benign per spec 187-188) for a symlinked index/ with no record at its target or an index regular file, although spec 133-137 calls such a component tampering and the comment at 386-388 promises tampering is never a clean miss (static trace) - report Mismatch when !cache_directories_are_real and test both cases; [P4] [doc-accuracy] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:126-130 - "a symlink as the repository's own final component is refused" holds only for the bare spelling: covers (maintenance.rs:35-37,195-207) admits alias/ and alias/. (rustc mirror: covered=true), and index_cache.rs:780-782 tests only the bare spelling; no safety impact since the path derives from guard.repository_root() - qualify the sentence or refuse those spellings and test alias/
SUMMARY: S20-300 revision 6 closes all three of my revision-5 P4 findings, each checked in code, tests and scratch-tree probes: the demonstrated literal and comment bypasses (A, B, B2, C) are refused and tested, the cache path comes from the guard's canonical root at all three entry points, and the coverage, gate and evidence sentences match the code and the server test. The checker passes at revision 6 (exit 0), its 21 tests pass, sley-repo index_cache passes 13/13 and sley-protocol workspace_open passes 5/5. Four new P4 residuals remain: forms the regex blanker still misses (C raw strings, nested comments, #[path]/cfg text), a check-then-use window on the new directory-component check, an audit that reports a tampered directory as a benign miss, and a final-component symlink refusal that depends on the path's spelling. None blocks acceptance.
