<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: ariadne; field: ariadne_contract_review_revision_6; dispatched: 2026-09-23T13:53:16Z; duration_s: 383; process_exit_code: 0 -->
# Ariadne Council review — complete_root_index_snapshot

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

This review was read-only. I did not edit, create or commit anything in the tree. Scratch fixtures lived in Python `tempfile` directories outside the worktree.

**Commands run**
- `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, which matches the scope. The only untracked files are other lanes' `-26d050e` verdict transcripts.
- `git log --oneline 2b0f1c9f..HEAD` listed 12 commits. The ones touching this package are `004cf5be` (code), `60be11c8` (contract revision 6), `1d8086fc` (revision-5 verdicts recorded) and `9e018922` (gate record §13).
- `git diff 2b0f1c9f..HEAD` restricted to this package's paths:
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` (+47/−19);
  - `crates/sley-repo/src/index_cache.rs` (+122);
  - `crates/sley-protocol/src/server.rs` and `server_tests.rs` (consumer hunks);
  - `scripts/check_complete_root_index_snapshot_profile.py` and `scripts/test_complete_root_index_snapshot_profile.py`;
  - `Makefile`;
  - the S20-300 rows of `docs/WORK_PACKAGES.md`, `docs/spec/ERROR_CODES_V1.md` and ADR-0029 (no S20-300 change in the last two).
- I compared the `complete_root_index_snapshot` machine-summary section at base and head in memory.

**Checkers and tests**
- `python3 scripts/check_complete_root_index_snapshot_profile.py`: exit 0, `"result": "PASS"`, `"revision": 6`, `"problems": []`, status `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`.
- `python3 -m unittest scripts/test_complete_root_index_snapshot_profile.py -v`: 21 tests, OK. That is 4 `CompletionBinding`, 9 `ProbeGate` and 8 `TokenAwareGate` tests.
- `cargo test --locked --offline -p sley-repo --lib index_cache`: 13 passed, 0 failed. This includes `a_symlinked_cache_directory_is_never_read_or_written_through` and `a_non_canonical_repository_spelling_is_covered_by_its_guard`.
- `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 5 passed, 0 failed. This includes `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record` and `workspace_open_answers_from_the_single_checked_head_load`.
- `python3 scripts/check_smp1_contract.py`: exit 0, PASS, revision 15. This is the consumer's pin.
- My own completion-gate probe, run in memory through the test module's `run_with_summary` against the real summary:
  - (a) COMPLETE flip with the real `*_revision_5` PASS fields present and no `*_revision_6`: exit 1, with `completion-unbound-review` for all three lanes.
  - (b) `*_revision_6` set to REVISE: exit 1, the same three problems.
  - (c) `*_revision_6` set to PASS: exit 0, PASS.
- My own gate-bypass probe, run on a `scratch_tree()` copy:
  - The control copy of the real tree gives `[]`.
  - (A) A plain probe call in `crates/sley-cli/src/leak.rs` is refused with `probe-caller:…`.
  - (B) The same call after `/* outer /* inner */ " */`, with a later `// "`, gives `[]`.
  - (C) A second `materialized_head_snapshot` caller in `server.rs`, hidden the same way, gives `[]`.
  - (D) `let r#use = cached_complete_root_snapshot_id;` in `server.rs` gives `[]`.
  - (E) The same binding named `peek` is refused with `references=2`.
- `rustc --edition 2024 --crate-type lib --emit=metadata` on a snippet containing the (B) and (D) constructs: exit 0 with no diagnostics. Both are valid Rust: block comments nest, and `r#use` is a legal raw identifier.

**Files read**
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`: lines 1-60 and 110-309.
- `crates/sley-repo/src/index_cache.rs`: lines 150-405 and 735-784, plus the diff of lines 785-828.
- `crates/sley-protocol/src/server.rs`: lines 2910-2979, plus the diff hunks at 1176-1295 and 2405-2420.
- `crates/sley-protocol/src/server_tests.rs`: lines 7528-7585 and the new tests at 7588-7686.
- `crates/sley-protocol/src/lib.rs`: lines 12-18.
- `crates/sley-txn/src/maintenance.rs`: lines 20-38, 153-166 and 195-215.
- `scripts/check_complete_root_index_snapshot_profile.py`: lines 92-221.
- `scripts/test_complete_root_index_snapshot_profile.py`: lines 1-110, plus the diff for 187-264.
- `docs/spec/SMP1.md`: line 3 and lines 735-769.
- `Makefile`: lines 24, 44, 101 and 174.
- Gate record `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: lines 895-964 (§13.3-13.5).
- My lane's revision-5 transcript `evidence/review/verdicts/complete_root_index_snapshot/ariadne_contract_review_revision_5-2b0f1c9.md`: all 115 lines.

## Evidence checked

**Round-7 (revision-5) findings of this lane**

Finding 1 — [P4] evidence-precision, spec 258-262 ("through `workspace.open`, absence while an exclusive maintenance owner holds the boundary"): CLOSED.
- Spec 284-287 now reads "absence from the consumer wrapper `materialized_head_snapshot` (the probe's non-waiting guard acquisition; `workspace.open` itself waits at the S20-390 head load) while an exclusive maintenance owner holds the boundary, and an unchanged discarded record through `workspace.open`."
- The test matches this split exactly:
  - `server_tests.rs:7568-7571` calls the wrapper directly while an exclusive guard is held and gets `None` in under 5 s.
  - `server_tests.rs:7573-7585` goes through `server.call(Method::WorkspaceOpen…)`, gets an 8-field body, and leaves the cache bytes unchanged.
- The text agrees with `SMP1.md:763-766` ("an exclusive owner makes the opener wait; only the probe adds no wait").

Finding 2 — [P4] gate-precision, spec 200-206 (unstated exemptions for integration tests and the `use` import): CLOSED.
- Spec 216-229 now states both exemptions: crate integration tests (`crates/<crate>/tests/`, which the gate exempts), and "That file may also import the probe by its own name (a `use` item, never an alias)".
- The checker matches:
  - line 191 exempts the probe only in integration tests;
  - lines 196-200 check the alias and strip `use` items before counting;
  - line 204 requires exactly one reference, inside `materialized_head_snapshot`.
- The new wrapper rule also matches the spec. There must be exactly one caller, inside `workspace_open` (checker 210-216). The `server_tests.rs` exemption holds only while `lib.rs` declares `#[cfg(test)] mod server_tests;` (checker 152-159; `lib.rs:17-18` satisfies this). `test_the_server_test_module_may_name_the_wrapper_only_while_cfg_test` pins that condition.
- One residual: the blanking the gate sentence relies on can be bypassed. That is new finding 1 below, not this finding.

Finding 3 — [P4] contract-precision, spec 118-121 (every "non-canonical spelling" covered): CLOSED.
- Spec 126-130 now enumerates the covered spellings: relative, through a symlinked parent, or containing `..`. It says a symlinked final component and another root are refused.
- This matches `covers` (`maintenance.rs:35-37`), which is `require_real_directory` (an lstat symlink refusal, 200-208) followed by `canonicalize` equality.
- The tests cover each case:
  - parent-symlink and `..` spellings: `index_cache.rs:749-764`;
  - a relative spelling: 768-778;
  - a refused final-component symlink: 780-782;
  - another root refused: `guard_for_another_repository_is_refused`, line 597.

**Revision-6 changes checked**

Cache path derived from the guard's canonical root:
- All three entry points now use `index_cache_path(guard.repository_root(), …)` after `covers` (`index_cache.rs:248`, `306`, `378`).
- `repository_root()` returns the root canonicalized at acquisition (`maintenance.rs:21-25`, `153`).
- The code conforms to spec 131-133. The test evidence for this change does not discriminate it; see new finding 2.

Real cache directory components:
- `cache_directories_are_real` (`index_cache.rs:160-173`) lstat-checks both `index/` and `index/v1/`; an absent component passes.
- It is checked on read (`323-325`, giving absence) and on write-back before and after `create_dir_all` (`181-191`). A refused write-back is fail-open: `let _ = write_record(…)` at 268 still returns the fresh build.
- `a_symlinked_cache_directory_is_never_read_or_written_through` discriminates the change. Under base code, the non-final `index` symlink would be followed on both read and write, and the test's None and "not written outside" assertions would fail. This matches spec 133-137 and evidence 289-290.

Unix scoping of the open flags:
- The flags are applied under `#[cfg(unix)]` (`index_cache.rs:328-332`), and the `is_file()` check on the handle is unconditional (334). This matches spec 137-140.

Consumer pin:
- The profile names SMP1 revision 15 (206).
- SMP1 revision 15 names S20-300 §5 revision 6 (`SMP1.md:751`), and `check_smp1_contract.py` passes at revision 15.
- The server docstrings name SMP1 15 and S20-300 6 (`server.rs:2916`, `2955`).
- Answering `workspace.open` from the head its session check retained (`server.rs:1183-1189`, `2942-2945`) does not change the probe's obligations. Its only test here, `workspace_open_answers_from_the_single_checked_head_load`, passes.

Completion binding:
- `SPEC_REVISION = 6` binds `<lane>_revision_6`. My probes (a) to (c) confirm this, including that the real revision-5 PASS values no longer admit COMPLETE.

Machine summary:
- `contract_revision` is 6, and the status stays `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`.
- The three revision-5 verdicts are recorded as `_revision_5` fields with notes. Mine is `PASS_0_P0_0_P1_0_P2_0_P3_3_P4_PRIOR_P3_P4_CLOSED`, which matches my transcript.
- The 43 other keys (base, initial, final and revision-4 fields) are byte-unchanged.

`make quick`:
- The S20-300 checker and its regression suite stay in `quick` (`Makefile:24`, `101`).

## Findings

[P4] [gate-precision] scripts/check_complete_root_index_snapshot_profile.py:102,109-130,200 (spec COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:216-229) - The spec says the gate reads every Rust file "with comments and string and char literals blanked", and that any other reference to either identifier fails. The docstring (126-127) and the comment at line 111 say a nested block comment ending at its first `*/` "can only expose text, never hide it". That is false: the exposed remainder can open a string match that swallows real code. My probe (B), `/* outer /* inner */ " */` followed by a real `cached_complete_root_snapshot_id(…)` call and a later `// "`, gives `[]`. The same construct hides a second `materialized_head_snapshot` caller in `server.rs` (probe C, `[]`). Separately, `USE_ITEM` (`\buse\s[^;]*;`) also matches the raw identifier `r#use`, so `let r#use = cached_complete_root_snapshot_id;` in the consumer is stripped before counting (probe D, `[]`; the control with `peek` is refused). rustc 2024 accepts both constructs with exit 0. These are deliberate-obfuscation bypasses of a governance lint; the shipped tree is clean. - closure evidence needed: blank block comments with a nesting-depth scan (or one left-to-right Rust tokenizer), and exclude a `use` preceded by `r#` from `USE_ITEM`. Add negative tests for both constructs, or narrow the spec sentence and the docstring to what the regex actually guarantees.

[P4] [evidence-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:288-289 (crates/sley-repo/src/index_cache.rs:765-779; gate record row 920) - The revision-6 evidence bullet claims "a relative spelling covered with the cache read and written under the guard's canonical root", and gate row 920 says the canonical-root derivation is "asserted in the spelling test". The test uses the relative spelling only for a probe read (775-778). The only write in that test comes through the parent-symlink spelling (754). The `is_file()` assertion at 779 checks a path that every spelling resolves to physically, so the test also passes against the base code that joined the caller's spelling. This is derived, not executed: I did not run the mutated code. The derivation itself is correct by inspection (248, 306, 378). - closure evidence needed: reword the bullet and row 920 to say the relative spelling is exercised through the probe only and that the canonical-root derivation is shown by construction. Alternatively, add a relative-spelling write, plus a test that separates the two paths, for example a test-only seam that retargets a parent symlink between `covers` and the path join.

## Assessment

Revision 6 closes all three of my revision-5 P4 findings. I verified each against the spec text, code, tests and checker output, not against the gate record's claims.
- **Code:** conforms to the revision-6 contract. All three entry points derive the cache path from the guard's canonical root. Both cache directory components must be real directories on read and on write-back, and the write-back stays fail-open. The Unix-only flags are scoped as the spec says. The consumer takes the probe's guard without waiting, and its single-caller wrapper is gated.
- **Checker and gates:** the completion gate binds to `<lane>_revision_6`, and my probes show that the real revision-5 PASS values no longer admit COMPLETE. The checker exits 0 at revision 6.
- **Tests:** 21 checker regressions, 13 `index_cache` tests and 5 `workspace_open` tests pass.

The two new findings are both P4 precision notes:
- The new literal- and comment-aware blanking still has two deliberately obfuscated bypasses.
- One revision-6 evidence sentence says more than its test shows.

In neither case does the shipped tree or behavior violate the contract.

Observations I am not counting as findings:
- `verify_cached_snapshot` reports `Missing` for a non-directory `index` or an `index` symlink with nothing behind it. Revision 6 now calls that state tamper in the code comments, but no spec sentence is contradicted and there is no regression from base.
- The definition file is skipped for the wrapper scan as well. That reference is impossible to compile, because sley-repo cannot depend on sley-protocol.
- The directory checks are check-then-use. That race stays inside the §5 same-filesystem-authority residual.
- The gate is lexical, so `#[path]`, `include!` and macro-assembled identifiers remain out of its reach.

This verdict accepts S20-300 contract revision 6 for the Ariadne lane only. It makes no claim about package completion, freeze, the other lanes, release or GA.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_2_P4_PRIOR_P3_P4_CLOSED
SECTION: complete_root_index_snapshot
FIELD: ariadne_contract_review_revision_6
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P4] [gate-precision] scripts/check_complete_root_index_snapshot_profile.py:102,109-130,200 (spec 216-229) - strip_rust ends a nested block comment at its first */ and the exposed remainder can open a string match that swallows real code (probe B: `/* outer /* inner */ " */` + real probe call + `// "` gives []; probe C hides a second materialized_head_snapshot caller the same way), and USE_ITEM also strips a `let r#use = cached_complete_root_snapshot_id;` binding (probe D gives []); both constructs compile under rustc 2024; the docstring claim "can only expose text, never hide it" is false - closure: a depth-counting comment scan or tokenizer, a USE_ITEM that excludes r#use, negative tests, or narrowed spec/docstring wording | [P4] [evidence-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:288-289 (index_cache.rs:765-779; gate record row 920) - the relative spelling is exercised only by a probe read, never a write, and the is_file assertion at 779 holds for the base caller-spelling join as well, so it does not discriminate the canonical-root derivation (derived, not executed) - closure: reword the bullet and row 920 to "probe only, derivation by construction" (248/306/378), or add a relative-spelling write plus a discriminating seam test
SUMMARY: S20-300 contract revision 6 is accepted by the Ariadne contract lane. All three revision-5 P4 findings are verified closed: the evidence bullet names the wrapper and the workspace.open split as the test does, the gate sentence states both exemptions, and the coverage wording matches RepositoryMaintenanceGuard::covers. The checker exits 0 at revision 6 with completion bound to <lane>_revision_6; 21 checker regressions, 13 index_cache tests and 5 workspace_open tests pass. Two new P4 notes remain: an obfuscation bypass of the literal- and comment-aware gate (nested block comment with a quote, r#use), and a revision-6 evidence sentence that its test does not discriminate.
