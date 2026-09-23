<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: ariadne; field: ariadne_contract_review_revision_5; dispatched: 2026-09-23T11:55:14Z; duration_s: 317; process_exit_code: 0 -->
# Ariadne Council review — complete_root_index_snapshot

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

This review was read-only. I did not edit, create or commit anything.

**Commands run**
- `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, which matches the scope, and the tree was clean.
- `git log --oneline f0738119..HEAD` listed 15 commits, including `40a84a5c` (S20-300 revision 5), `46118639` (fuzz libc lock), `7fcb313b` (S20-710 mirror) and `2b0f1c9f` (gate record §12).
- `git diff --stat` and `git diff f0738119..HEAD` over the spec, `crates/sley-repo`, `Cargo.lock` and the section checker: 5 files changed, +249/−52. I also diffed the new `scripts/test_complete_root_index_snapshot_profile.py` (+188).

**Checkers and tests**
- `python3 scripts/check_complete_root_index_snapshot_profile.py`: exit 0, `"result": "PASS"`, `"revision": 5`, `"problems": []`, status `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`.
- `python3 -m unittest scripts/test_complete_root_index_snapshot_profile.py -v`: 13 tests, OK.
- `cargo test -p sley-repo --lib index_cache`: 12 passed, 0 failed. This includes `a_symlink_to_a_valid_record_is_absence_for_the_probe`, `a_fifo_at_the_cache_path_answers_at_once`, `a_non_canonical_repository_spelling_is_covered_by_its_guard` and `symlink_at_the_cache_path_fails_closed`.
- `cargo test -p sley-protocol --lib workspace_open`: 4 passed, 0 failed. These are the v1, v2 and v3 `workspace.open` tests plus `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`.
- My own in-memory completion-gate probe, run against the checker module with the summary substituted in memory:
  - (a) Status flipped to `S20_300_FULL_COMPLETE` with no `*_revision_5` fields: exit 1, with `completion-unbound-review` reported for all three lanes.
  - (b) `*_revision_5` set to REVISE: exit 1, same three problems.
  - (d) `*_revision_5` set to PASS: exit 0, no problems.
- `python3 scripts/check_smp1_contract.py`: exit 0, revision 14.

**Files read**
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`: lines 1-19 and 42-48 (through the diff) and 108-281.
- `crates/sley-repo/src/index_cache.rs`: lines 1-420 and 640-725.
- `scripts/check_complete_root_index_snapshot_profile.py`: lines 24-89, 92-165 and 168-307.
- `scripts/test_complete_root_index_snapshot_profile.py`: all 188 lines.
- `crates/sley-protocol/src/server.rs`: line 43 and lines 2880-2935.
- `crates/sley-protocol/src/server_tests.rs`: lines 7393-7585.
- `crates/sley-txn/src/maintenance.rs`: lines 35-37, 142-218.
- `docs/spec/SMP1.md`: lines 57-82, 682 and 716-760.
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: lines 1-14 and the appendix headers.
- `docs/adr/ADR-0029-complete-root-index-snapshot-boundary.md`: lines 25-60.
- `docs/spec/ERROR_CODES_V1.md`: lines 273-274.
- `docs/WORK_PACKAGES.md`: line 35.
- The `complete_root_index_snapshot` section of `machineresearch/sley-2.0/machine-summary.json`.
- Gate record `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: rows 686-692.
- My lane's revision-4 transcript `evidence/review/verdicts/complete_root_index_snapshot/ariadne_contract_review_revision_4-f073811.md`: lines 83-109.
- The `libc` entries in `Cargo.lock` and `fuzz/Cargo.lock` (lines 166-169), and `Makefile:101`.

## Evidence checked

I checked each of my five revision-4 findings against code, tests and checker output, not against the gate record.

1. **[P3] Preamble "only for read-only derived query surfaces" — CLOSED.**
   - Spec lines 42-48 now name "the one non-query hit reader, the section 5 identity probe (which reads a record's identity, never its contents as evidence)". Section 5 lines 165-166 and section 9 lines 274-276 agree.
   - `ERROR_CODES_V1.md:273-274`, `WORK_PACKAGES.md:35` and the module doc at `index_cache.rs:4-10` all name the probe.
   - The checker exits 0.

2. **[P3] Completion gate accepted the revision-3 base PASS — CLOSED.**
   - Checker lines 280-284 require `<lane>_revision_{SPEC_REVISION}` to start with PASS, where `SPEC_REVISION = 5` (line 24).
   - My probes (a) and (b) fail with `completion-unbound-review:<lane>` for all three lanes; probe (d) passes.
   - The four `CompletionBinding` regression tests pass, and `Makefile:101` runs them.

3. **[P4] The probe gate scanned only `crates/` and only literal calls — CLOSED in substance.**
   - `probe_gate_problems` (checker lines 119-165) scans `crates/` and `fuzz/` (line 133) for the identifier itself (line 132), rejects aliases (line 148), and requires exactly one reference outside `use` items. That reference must be inside `materialized_head_snapshot`, which must use the non-blocking acquisition (lines 150-161).
   - The eight negative `ProbeGate` tests pass, covering an aliased import, a same-named file in another crate, a `src/tests/` path, a fuzz function pointer, a second reference in the consumer, an aliased import in the consumer, and a waiting or initializing consumer.
   - The spec's new text states the gate more strictly than the checker enforces it. That residual is new finding 2.

4. **[P4] Field 9 under version 3 — CLOSED.**
   - The rule is now stated for version 2 and version 3 selections in:
     - spec lines 190-194;
     - `SMP1.md:682` (row 201) and `SMP1.md:722-725`;
     - the `NATIVE_TEST_ADMISSION_V1.md` revision 6 status (lines 3-10);
     - the `server.rs` docstring (lines 2890-2903).
   - The `>= PROTOCOL_VERSION_V2` gate (`server.rs:2909`) now matches that text.
   - `workspace_open_v3_answers_the_version_2_open_summary` (`server_tests.rs:7468`) passes. It returns eight fields when the cache is cold. When warm it returns field 9 equal to the `run_root_query` snapshot identity, and it refuses a non-empty body.

5. **[P4] `read_record` checked the file type and then opened it without O_NOFOLLOW/O_NONBLOCK — CLOSED.**
   - `index_cache.rs:283-300` opens the file once with `custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)` on Unix. It then requires `is_file()` on the open handle and caps the read at `MAX_SNAPSHOT_RECORD_BYTES + 1`.
   - Both new tests pass. A symlink to a byte-identical valid record gives `None`, and `complete_root_snapshot` fails closed. A FIFO with no writer returns promptly on both paths.

**Revision-5 changes checked**
- **Guard checks:** all three now use `guard.covers(repository)` (`index_cache.rs:210`, `265`, `327`). `covers` compares against the canonicalized real directory (`maintenance.rs:35-37`, `195-207`). The probe's only `Err` is the coverage refusal (lines 265-269), as spec lines 188-189 state.
- **Consumer:** `server.rs:2925-2933` takes `acquire_shared_repository_maintenance_nonblocking` (`maintenance.rs:142-146`, `exclusive=false`, `wait=false`), never initializes the boundary, and flattens any error to `None`. This matches spec lines 195-198 and `SMP1.md:740-745`.
- **The new libc dependency:**
  - The direct pin is `libc = "=0.2.189"` (`crates/sley-repo/Cargo.toml`), and the dependency edge was added to `Cargo.lock`.
  - libc 0.2.189 was already the single locked version in both `Cargo.lock` and `fuzz/Cargo.lock`, with the same checksum.
  - The S20-710 audit records the new edge (178 relationships).
- **Machine summary:** `contract_revision: 5`, status `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`, `implementation_complete: false`. The revision-4 verdicts are kept as `_revision_4` fields, and the historical base, final and initial fields are unchanged.

## Findings

[P4] [evidence-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:258-262 - The revision-5 evidence bullet requires "through `workspace.open`, absence while an exclusive maintenance owner holds the boundary". That cannot happen: the same profile (lines 196-198) and SMP1.md appendix A ("an exclusive owner makes the opener wait; only the probe adds no wait") say the opener's accepted-head load blocks while an exclusive owner holds the boundary. The test that serves as this evidence calls the consumer function `materialized_head_snapshot` directly (crates/sley-protocol/src/server_tests.rs:7567-7569), not `workspace.open`. Only the discarded-record half of the bullet goes through `workspace.open`. - closure evidence needed: reword the bullet so the exclusive-owner case names the probe's consumer (`materialized_head_snapshot`, i.e. the non-waiting guard acquisition), or add an interleaving test that really goes through `workspace.open`.

[P4] [gate-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:200-206 - The spec says the stage checker "admits exactly one reference" outside index_cache.rs and that "any other reference to the identifier (a call, an import, …) in `crates/` or `fuzz/` fails the gate". The checker admits more than that. It skips every file under `crates/<crate>/tests/` (scripts/check_complete_root_index_snapshot_profile.py:139-140), and `test_crate_integration_tests_are_exempt` (scripts/test_complete_root_index_snapshot_profile.py:147) pins that exemption. It also admits the consumer's non-aliased `use` import (checker lines 150-151; crates/sley-protocol/src/server.rs:43). The checker docstring (lines 124-127) and gate record row 688 state both exemptions; the governing spec does not. This is the same kind of spec-promises-more-than-the-gate gap as my revision-4 P4, now narrowed to these two exemptions. - closure evidence needed: state both exemptions in section 5, or remove them from the gate, then re-run the checker and its tests.

[P4] [contract-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:118-121 - The spec says that under `RepositoryMaintenanceGuard::covers` "a relative or otherwise non-canonical spelling of the same repository is covered". `covers` actually refuses a spelling whose last path component is a symlink to the repository: `canonical_real_directory` calls `require_real_directory`, which rejects a symlink (crates/sley-txn/src/maintenance.rs:35-37, 195-207). The revision-5 test pins that refusal (crates/sley-repo/src/index_cache.rs:720-722), and gate record row 690 records it. The code fails closed, so this is a wording gap, not a safety defect. - closure evidence needed: narrow the wording to spellings through symlinked parent directories or dot components, excluding a symlinked final component, or defer to `covers` without the broader wording.

## Assessment

Revision 5 closes all five of my revision-4 findings (2 P3, 3 P4). I verified each against code, tests and checker output, not the gate record.

- **Code:** the code matches the contract. The cache open is a single `O_NOFOLLOW | O_NONBLOCK` open checked on the handle. All three guard checks use canonical coverage. The probe's only error is a guard that does not cover the repository. The consumer takes the guard without waiting, never initializes the boundary, and treats every failure as absence.
- **Checker:** completion is now bound by field name to `<lane>_revision_5` PASS values. My own in-memory probes confirm that an unbound or REVISE flip fails and a bound PASS is admitted.
- **Tests:** the checker exits 0 on revision 5; 13 checker regressions, 12 `index_cache` tests and 4 `workspace_open` tests all pass.

The three new findings are P4 wording and precision notes. In each case the behavior is correct or fails closed, but the spec says more than the tests or the gate deliver.

Observations I am not counting as findings:
- `materialized_head_snapshot` is `pub(crate)` and the gate works at the identifier level. A second caller of that function inside sley-protocol would therefore not trip the gate. Today its only callers are `workspace_open` (`server.rs:2910`) and tests.
- ADR-0029:39-41 still reads "SMP1 revision 13 … field 9 only" as a dated revision-4 addendum.
- `O_NOFOLLOW` protects only the final path component. Intermediate `index/v1` symlinks stay within the §5 same-filesystem-authority residual.

This verdict accepts S20-300 revision 5 for my lane only. It makes no claim about package completion, freeze, the other lanes, release or GA.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_3_P4_PRIOR_P3_P4_CLOSED
SECTION: complete_root_index_snapshot
FIELD: ariadne_contract_review_revision_5
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P4] [evidence-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:258-262 - the revision-5 evidence bullet requires "through workspace.open, absence while an exclusive maintenance owner holds the boundary", but the same profile (196-198) and SMP1 appendix A say the opener's head load blocks under an exclusive owner; the test calls materialized_head_snapshot directly (server_tests.rs:7567-7569) - closure: reword the bullet to name the probe consumer (non-waiting guard acquisition) or add an interleaving workspace.open test | [P4] [gate-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:200-206 - spec says the checker admits exactly one reference and any other (including an import) fails, but the checker exempts crates/<crate>/tests/ (check_complete_root_index_snapshot_profile.py:139-140, pinned by test_crate_integration_tests_are_exempt at test_complete_root_index_snapshot_profile.py:147) and admits the consumer's non-aliased use import (checker 150-151; server.rs:43) - closure: state both exemptions in section 5 or remove them, re-run checker and tests | [P4] [contract-precision] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:118-121 - "a relative or otherwise non-canonical spelling of the same repository is covered" is broader than RepositoryMaintenanceGuard::covers, which refuses a symlinked final component (maintenance.rs:35-37, 195-207; pinned at index_cache.rs:720-722) - closure: narrow the wording to parent-symlink and dot-component spellings, or defer to covers without the broader wording
SUMMARY: S20-300 revision 5 is accepted by the Ariadne contract lane. All five revision-4 findings (2 P3, 3 P4) are verified closed by code, tests and checker output: preamble and record wording, revision-bound completion, identifier-level probe gate over crates/ and fuzz/, the version 3 consumer scope, and the O_NOFOLLOW|O_NONBLOCK open. The section checker exits 0 (PASS, revision 5); 13 checker regressions, 12 index_cache tests and 4 workspace_open tests pass. Three new P4 wording notes remain, each where the spec states more than the tests or gate deliver, with the code correct or failing closed.
