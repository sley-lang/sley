# Vulcan re-review — s20_700_remaining_surface_audit / vulcan_review (QA + security lane)

Role: Vulcan (independent QA, security, tests, hardening). Same-lane full
re-review of the 2026-09-04 Council FAIL (`FAIL_1_P0_2_P1_6_P2_3_P3`),
re-deriving closure at HEAD instead of accepting the harness "final" fields.

## 1. Scope verification

- `git rev-parse HEAD` = `43f2f5ba738b587a9a0bb365a55f3096527e3e3d`, branch `main`.
  Matches SCOPE_SHA. `git status --short` was empty before and after every
  command below (read-only mandate held; the only write is this file).
- Date of review: 2026-09-14/15 (UTC), host primary-host.

## 2. Inputs read in full

Prior round and harness finals:
- `machineresearch/sley-2.0/reviews/s20-700fuzz-vulcan-surface-review-2026-09-04.log`
  (the transcript behind the FAIL string; 55 lines, read whole).
- `machineresearch/sley-2.0/reviews/verdicts.json` (700fuzz rows: ariadne
  FAIL 1/4/6/2, nabu FAIL 2/4/5/4, vulcan FAIL 1/2/6/3).
- `evidence/review/verdicts/s20_700_remaining_surface_audit/{ariadne,nabu,vulcan}-final-review-d384f0f.md`.
- `evidence/review/finding-register.json` obligations 249-256 (row 256 =
  this field, state `PENDING`, disposition `FAIL_1_P0_2_P1_6_P2_3_P3`).

Subject:
- `docs/audits/S20_700_REMAINING_SURFACE_BLOCKERS.md` (105 lines).
- `docs/audits/S20_700_VM_INPUT_PERSISTENT_SLICE.md` (170 lines); the other
  17 `S20_700_*_SLICE.md` files were grepped for crash dispositions
  (pack, semantic-checkers read at the cited ranges).
- `fuzz/Cargo.toml` (21 `[[bin]]` entries), `fuzz/targets/vm_canonical_inputs.rs`
  (1834 lines, whole), `fuzz/regressions/*.json` (4 files, whole), all 21
  `fuzz/targets/*.rs` surveyed for their production entry calls.
- `scripts/run_vm_persistent_fuzz.py` (833 lines, whole),
  `scripts/check_vm_persistent_fuzz_slice.py` (283 lines, whole),
  `scripts/check_s20_700_frontier.py` (206 lines, whole); the other 19
  `check_*_persistent_fuzz_slice.py` / `check_merge_judgment_fuzz_slice.py`
  and 20 runners surveyed by grep for proof-record validation, run floors,
  `Done N runs` parsing, and crash retest.
- `Makefile` lines 80-100 and 225-345 (`*-persistent-fuzz-smoke`,
  `persistent-fuzz-all`, the `quick` frontier line).
- `machineresearch/sley-2.0/machine-summary.json`: every `s20_700_*` block
  and `s20_700_remaining_surface_audit` (dumped and read whole).
- `docs/adr/ADR-0039-vm-extended-opcode-boundary.md` (head),
  `docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md` (status block, section 5, 6).
- Master goal `Sley2.0mastergoal.md` section 18.5 (lines 2456-2470).
- Runtime evidence (gitignored, present in the worktree):
  `evidence/runtime/s20-700-*/evidence.json` for vm, merge, merge-judgment,
  semantic-delta, complete-root-snapshot, semantic-checkers, pack; the
  four on-disk crash artifacts (bytes read with Python).

## 3. Tool results (commands run and exact results)

All checkers ran with `SLEY2_MASTER_GOAL=/home/dev/machineresearch/Sley2.0mastergoal.md`.

1. `python3 scripts/check_s20_700_frontier.py` -> exit 0,
   `{"contract":"s20-700-persistent-fuzz-frontier-v1","full_s20_700_complete":false,"remaining_required_surfaces":[],"result":"PASS"}`.
2. All 19 `scripts/check_*_persistent_fuzz_slice.py` plus
   `check_merge_judgment_fuzz_slice.py` and `check_schema_fuzz_slice.py`
   -> every one exit 0, `"result": "PASS"` (adapter_responses,
   candidate_result, complete_root, complete_root_snapshot, context_capsule,
   exchange, merge, mutation_candidate, pack, query, root_query, scb1,
   schema, semantic_checkers, semantic_delta, smp1_json_bridge, smp1,
   transaction_receipt, vm, merge_judgment, schema_fuzz).
3. `make vm-persistent-fuzz-smoke` -> `VM_EXIT=0`. Runner evidence:
   `result PASS`, `source_commit 43f2f5ba…`, `executed_runs 1576`,
   `runs_floor 1576`, `corpus_count 789`, `coverage_ok true`,
   `owner_lib_sancov 125` (libsley_vm), `new_crash_artifacts []`,
   `retested_prior_crashes [{crash-b55c33e9…, returncode 0, still_crashes false}]`,
   `fuzz_toolchain_layout "arch"`, `toolchain_overridden false`,
   `cc /usr/lib/llvm18/bin/clang-18` (`clang version 18.1.8`),
   rust `nightly-2026-02-27`, `unexpected_warnings []`, `problems []`.
4. `make merge-persistent-fuzz-smoke` -> `MERGE_EXIT=0`. Both runners PASS
   at `43f2f5ba…` on the same canonical arch layout:
   `merge_conflict_decoder` executed 638 / floor 638, `owner_lib_sancov 149`;
   `merge_judgment` executed 512 / floor 512, `owner_lib_sancov 149`;
   no crash artifacts, no unexpected warnings.
   (Wall time for both smokes: 29 s; builds were cached under the
   gitignored per-slice target dirs.)
5. Host toolchain: `rustup toolchain list` contains
   `nightly-2026-02-27-x86_64-unknown-linux-gnu`; `/usr/lib/llvm18/bin/clang-18 --version`
   = `clang version 18.1.8`; the pinned libFuzzer archive exists at
   `/usr/lib/llvm18/lib/clang/18/lib/linux/libclang_rt.fuzzer-x86_64.a`;
   no `RUSTFLAGS`/`SLEY_FUZZ_*` in the environment. `cargo fuzz` is not
   installed and is not needed (the runners drive `cargo +nightly rustc`
   with `-Cpasses=sancov-module` and link the archive directly).
6. Proof-currency re-derivation (Python over machine-summary): for each
   slice block, `git merge-base --is-ancestor <source_commit> HEAD` and
   `git diff --name-only <source_commit> HEAD -- <target(s)> <runner> fuzz/Cargo.toml fuzz/Cargo.lock crates/<owner crates>`.
   Result: every proof commit is an ancestor of HEAD. `changed_since = 0`
   only for the VM slice (proof 209c661). Every other slice's runner
   changed after its proof (commit 5277791, the pin-layout repair). Two
   slices also had their *target* change after their durable proof:
   `fuzz/targets/semantic_delta_decoder.rs` (proof a33e1e8; changed in
   fc98a89) and `fuzz/targets/complete_root_snapshot_decoder.rs` (proof
   d3dee56; changed in 209c661). `ssmc_graph_cfg_checker.rs` also differs
   from its proof commit df295d8 (that proof was recorded with the target
   dirty, per its own `worktree_dirty_files`).
7. Runtime-vs-durable comparison: runtime `evidence.json` source commits are
   merge 43f2f5b (mine), merge-judgment 43f2f5b (mine), semantic-delta
   209c661, complete-root-snapshot 209c661, vm 43f2f5b (mine),
   semantic-checkers 5277791, pack 5277791; the tracked durable records
   for merge / semantic-delta / complete-root-snapshot still say
   a33e1e8 / a33e1e8 / d3dee56 (machine-summary.json:2336, :2280, :2443).
8. Crash artifacts on disk (4):
   `s20-700-pack-import-libfuzzer/artifacts/crash-35737a63…` (1422 B, `02 53 4c 45 59 53 43 42 31 …` = selector 2 + canonical pack),
   `…/crash-4b1158f7…` (1425 B, `02 03 00 07 53 4c 45 …` = selector 2 + control class 3 / index 0 / bit 7 + canonical),
   `s20-700-semantic-checkers-libfuzzer/graph-cfg-artifacts/crash-47e105f9…` (16 B, `02 ef 01 10 ff ef 00 …`),
   `s20-700-vm-input-libfuzzer/artifacts/crash-b55c33e9…` (71 B, `00 00 02 00 01 01 03 ff …`, the original of `S20_700_VM_001`).
   `fuzz/regressions/` holds exactly four records: HARNESS_001 (type_checker,
   `c2`), SMP1_001, VM_001 (`ffff02`), VM_002 (`20456b`).

## 4. Independently re-derived claims

- Section 18.5's eleven required surfaces each have a landed libFuzzer
  binary that calls the production entry it names (static survey of every
  target plus the sancov owner-rlib counts in the proofs): SCB1 decoder ->
  `scb1_decoder` (`decode_standalone`, `decode_payload*`); schema decoder ->
  `schema_bootstrap_decoder` (`import_bootstrap_preimage`); SSMC graph
  validator and CFG checker -> `ssmc_graph_cfg_checker` (`validate_function_graph`,
  `validate`); type checker -> `type_checker` (`check_type`, `check_consistency`);
  query requests -> `restricted_query_request` (`execute_restricted_query`) and
  `root_query_engine` (`execute_root_query`); mutation candidates ->
  `mutation_candidate` (`import_candidate`, `build_candidate`, codec pair);
  pack importer -> `repository_pack_importer` (`import_conformance_pack`);
  merge engine -> `merge_conflict_decoder` (`decode_merge_conflict`) and
  `merge_judgment` (`judge_merge`); VM canonical inputs -> `vm_canonical_inputs`
  (`execute_function`, `lower_function`, `judge_function_operations`,
  `validated_execution_input_hashes`); adapter responses -> `adapter_responses`
  (`import_for`, `validated_hash`, `check_constant`). `remaining_required_surfaces: []`
  is therefore true, not a summary string.
- Every `*-persistent-fuzz-smoke` Make target is checker-then-runner
  (Makefile:234-310); no checker substitutes for the runner, and no checker
  reads a PASS string as proof of a smoke. Every runner computes
  `runs_floor = max(--runs, corpus_file_count + 256)`, parses libFuzzer's
  `Done N runs` line into `executed_runs` (`-1` when absent), and gates PASS
  on `executed_runs >= runs_floor`, inline-counter coverage with monotonic
  `ft`, no new crash artifacts, no still-crashing retested priors, and no
  unexpected WARNING lines (run_vm_persistent_fuzz.py:129-130, 241-315; the
  other 19 runners carry the same markers). `--runs 0` no longer yields a
  PASS with zero executions.
- The VM seed corpus is 789 unique seeds by construction (256 + 256 + 9x6x2
  + 9x9x2 + 7 = 789), matching `corpus_count 789` in my run and the docs.
- The extended-family lane draws each fixture's request from
  `fixture.fixture.expected_input_types` under `generous_limits()`
  (vm_canonical_inputs.rs:355-363) and requires the first execution to be
  `ExecutionTermination::Success` with a result-canonical payload
  (:385-401); the outer canonical lane requires the same under limits
  profile 0 (:132-159). Because my 1576-run smoke replays all 162 family
  seeds (9 outer fixtures x 9 families x 2) and produced no crash artifact,
  every family (E1-E8 including E7a and the E8 bridge) reached a Success
  termination under its own inputs at HEAD. The prior P0 mechanism (outer
  restricted request dying in `validate_inputs`) cannot recur silently.
- Lane header bytes 1-5 (`family_gate`, `family_selector`, `extended_lane`,
  `map_order_lane_flag`, `canonical_inputs`) are consumed before any
  variable-length construction (:50-55), so family selection is
  offset-stable. `limits_profile` is drawn *after* the inputs (:67), so it
  is not a fixed offset; the seed generator only pins it through a uniform
  filler byte (run_vm_persistent_fuzz.py:350-363). Selection is correct;
  the header comment's "limit_selector" label is imprecise (P4 below).

## 5. Per-item analysis of the 2026-09-04 findings

| # | Sev | Prior finding | Status at HEAD | Evidence |
|---|-----|---------------|----------------|----------|
| 1 | P0 | Extended-family lane reused the outer restricted request; E2/E3/E4 died in `validate_inputs`, determinism compared two `Err`s, observation re-derivation never fired, no success guard. | CLOSED | `fuzz/targets/vm_canonical_inputs.rs:347-401` builds the request from the fixture's own types and panics unless the first draw is `Success` with an encodeable value; :437-453 re-derives the observation id. Checker pins both assert messages (`check_vm_persistent_fuzz_slice.py:36-38,58-62`). Live: `make vm-persistent-fuzz-smoke` 1576/1576, 162 family seeds, no crash. |
| 2 | P1 | Cross-profile refusal was a bare `is_err()`. | CLOSED | `:416-421` pins `LowerErrorCode::OpcodeUnsupported` on any `Lowering::Lower` error for every selector; only a `LoweringError::Cfg` is additionally tolerated, and only for selectors 6/7 (`:428-431`), with the Success assertion above guarding malformed fixtures. Checker pins `the restricted refusal was not the opcode judgment` and `LowerErrorCode::OpcodeUnsupported` (:63-65). Residual looseness (any `Cfg` code for E6/E7a) is documented at the audit's "Adequacy notes" and recorded as P4 below. |
| 3 | P1 | `--runs 0` could emit PASS; no executed-run count; checker verified neither. | CLOSED | `run_vm_persistent_fuzz.py:129-130` (`runs_floor = max(args.runs, corpus_file_count + 256)`), `:241` (`executed_runs` parsed from `Done N runs`, -1 if absent), `:293-306` (PASS requires `executed_runs >= runs_floor`). `check_vm_persistent_fuzz_slice.py:221-232` requires the durable proof to be PASS with integer `executed_runs >= runs_floor`. My run: 1576/1576. |
| 4 | P2 | Campaign record stale (7 fixtures / 751 seeds vs 8 / 769). | CLOSED | Docs now state 9 fixtures / 789 seeds / 1576-run floor (`S20_700_VM_INPUT_PERSISTENT_SLICE.md:6-10,24,83-85,129`); runner constants `FIXTURE_COUNT = 9`, `EXTENDED_FIXTURE_COUNT = 9`, `SMOKE_RUNS = 1576`; machine-summary `generated_seed_count 789`, checker pins 789 (:180). Corpus count re-derived as 789. One leftover count ("144 family seeds") is P4 below. |
| 5 | P2 | machine-summary `extended_family_fixture_count 8` vs lanes list of 6, no E7a; checker pinned count only. | CLOSED | Block lists 8 lanes E1-E8 including E7a and E8 with count 9 (E2 carries two fixtures); `check_vm_persistent_fuzz_slice.py:187-198` pins the exact lane list, not just the count. |
| 6 | P2 | ADR-0039 line 4 said "E1 through E6 implemented". | CLOSED | `ADR-0039:3-8` now reads "slices E1 through E6 plus E7a implemented (2026-09-05) … slice E8 … implemented". |
| 7 | P2 | Contract section 5 bound "128 repeated executions" to nothing and mixed artifacts. | CLOSED | `VM_EXTENDED_OPCODE_PROFILE_V1.md` section 5 (revision 14): the 128 belongs to the vectors ("the fuzz lane asserts determinism twice per draw instead"), per-family reachability is a lane duty with the completion assertion and corpus-coverage floor stated, per-opcode duty sits with vectors plus the rejection matrix. |
| 8 | P2 | E6 needed a separate callee inventory so per-function narrowing is exercised. | CLOSED (by design) | `call_fixture` (:713-801) now carries two callee `FunctionGraph`s (`callee_graph`, `inner_graph`) in `functions`, threads a Bool argument through the first callee, and nests a second zero-parameter call; blocks and operations stay in one flat inventory that `lower.rs:265,295,382,440` narrows per function (`owned.narrow`). Under EXTENDED a narrowing regression would break the callee's single-graph shape and fail the Success assertion; under RESTRICTED the flat inventory fails the single-graph rule first (the documented `Cfg` exception). The architectural question of fully separate inventories stays with Nabu, as the audit records; nothing here masks a defect. |
| 9 | P2 | `vulcan_review: "DEFERRED_FORGE_OAUTH_401"` hard-pinned in the checker. | CLOSED | `check_vm_persistent_fuzz_slice.py:185-186` accepts any value whose first `_`-token is PASS/REVISE/FAIL; the frontier checker (:151-153) accepts PASS*/FAIL* for the audit field, so this review's outcome can be recorded without a checker edit. |
| 10 | P3 | Negative E7 lane worth adding once the refusal code is pinned. | OPEN (advisory) | The refusal is pinned (item 2), so the lane is now meaningful, but no lane refuses an unlanded E7 opcode (145, 160, 162) under either profile; the contract's section 6 excludes E7 beyond E7a/E8, and the E8 `bridge_sublane` covers refusals only for tampered adapter rows. Carried as a P3 follow-up; not a defect. |
| 11 | P3 | Seed-layout comments assumed fixed offsets the cursor did not preserve; family byte not consumed when the gate missed. | CLOSED | Header consumed unconditionally at :50-55 before any value construction; family seeds `[fixture, 0, family, …]` select their family for every outer fixture. Verified by reading the cursor path; confirmed empirically by the family-seed replay in the smoke. Comment imprecision on the limit byte is P4 below. |
| 12 | P3 | `supported_opcodes: [102,103,104]` read as contradicting the extended status. | CLOSED | Key renamed `restricted_profile_opcodes` (machine-summary vm block). |

Net: 1 P0, 2 P1, 6 P2 closed at HEAD with code, checker, and live-smoke
evidence; 2 of 3 P3 closed; 1 P3 remains advisory.

## 6. New findings

Searched for: Section 18.5 surfaces without a landed target (none); crash
artifacts without a regression vector; slice checkers that read a PASS
string instead of the smoke running; targets whose bounded smoke does not
execute the decoder it names (none: each target calls its production entry,
the proofs record nonzero owner-rlib sancov counts, and my two live smokes
grew `ft` and executed the full floors).

**V-01 [P3] durable proof records lag their targets and only the VM checker can tell.**
`check_vm_persistent_fuzz_slice.py:218-267` validates the durable
`last_local_proof` (PASS, floor coverage, no new crashes, owner sancov,
40-hex commit, `merge-base --is-ancestor`, and `git diff --quiet <proof> HEAD --
target runner Cargo.toml Cargo.lock crates/sley-vm`). None of the other 19
slice checkers validates the proof record at all (grep: zero
`last_local_proof` references outside the VM checker; e.g.
`check_merge_persistent_fuzz_slice.py:105-115`,
`check_semantic_delta_persistent_fuzz_slice.py:99-107`,
`check_complete_root_snapshot_persistent_fuzz_slice.py:100-104` pin only
static fields). Consequence at HEAD: the tracked proofs for
`semantic_delta_decoder.rs` (machine-summary.json:2280, commit a33e1e8) and
`complete_root_snapshot_decoder.rs` (:2443, d3dee56) predate edits to those
very targets (fc98a89, 209c661), and every non-VM proof predates the runner
pin-layout change (5277791) — yet `make quick` and every slice checker pass.
The harness finals' "proof currency FRESH … re-run PASS at 209c661" for
merge / semantic-delta / complete-root-snapshot is true only of the
gitignored `evidence.json` files; the tracked records still say
a33e1e8/a33e1e8/d3dee56 and, for merge, `executed 698` versus the runtime
638. Nothing here is a wrong answer (I re-ran merge and merge-judgment at
HEAD on the canonical clang-18 layout and both PASS; semantic-delta and
snapshot are adjacent, not Section 18.5 surfaces), so this is a currency and
uniformity gap, not a coverage gap. Fix: lift the VM checker's proof-record
validation (:218-267) into every slice checker, and refresh the three
records at HEAD.

**V-02 [P3] crash-discipline is uneven: three retained crashers have no stable finding ID, and one has no tracked regression vector.**
Master goal 18.5 requires every discovered crash to receive a minimized
fixture, a stable finding ID, a regression test, and a root-cause
disposition. The VM (`S20_700_VM_001/002`), SMP1 (`S20_700_SMP1_001`) and
type-checker (`S20_700_HARNESS_001`) crashes meet all four. Three do not:
(a) `crash-35737a63…` and (b) `crash-4b1158f7…` under
`evidence/runtime/s20-700-pack-import-libfuzzer/artifacts/` are disposed
only inside a Vulcan transcript
(`evidence/review/verdicts/s20_700_pack_persistent_fuzz_slice/vulcan_review-c7fec98.md:10,18`,
"harness-oracle misexpectations") and as `retested_prior_crashes` in the
proof (machine-summary.json:2127-2140); their inputs are tracked seeds by
construction (`run_pack_persistent_fuzz.py:743-756`: selector 2 + canonical
and `(2, class 3, index 0, bit 7)` + canonical), but no
`fuzz/regressions/` record or finding ID exists and
`S20_700_PACK_IMPORT_PERSISTENT_SLICE.md` never names them. (c)
`crash-47e105f9…` (graph-cfg, `02 ef 01 10 ff ef 00…`, template 2 with five
mutations) is described at `S20_700_SEMANTIC_CHECKERS_PERSISTENT_SLICE.md:206-213`
as a "documented harness-oracle correction probe (retests clean, permanent
retest seed)", but the only retest is the gitignored artifact
(`machine-summary.json:2778`); no seed with those bytes exists in
`run_semantic_checkers_persistent_fuzz.py` (searched for every spelling),
and no `fuzz/regressions/` record or finding ID exists. A fresh clone loses
all three retests. Fix: file `S20_700_PACK_001/002` and
`S20_700_GRAPH_CFG_001` under `fuzz/regressions/` with `input_hex`, and add
the graph-cfg bytes as a tracked seed the way `ffff02`/`20456b` are.

**V-03 [P4] stale count in the VM fix record.**
`S20_700_VM_INPUT_PERSISTENT_SLICE.md:79` says "all 144 family seeds"; with
E8 the generator emits 9 x 9 x 2 = 162 family seeds
(`run_vm_persistent_fuzz.py:360-363`).

**V-04 [P4] header comment overstates the limit byte.**
`run_vm_persistent_fuzz.py:355-359` lists `limit_selector` as the seventh
fixed header byte; `limits_profile` is drawn after the inputs
(`vm_canonical_inputs.rs:67`), so it is only pinned by the uniform filler.
Lane selection is unaffected.

**V-05 [P4] target-count wording.**
`docs/WORK_PACKAGES.md:63` says "twenty persistent libFuzzer targets";
`fuzz/Cargo.toml` declares 21 `[[bin]]` entries (the semantic-checkers slice
carries two). The frontier's `scoped_target_count: 20` counts slice records.
Say "twenty slices, twenty-one binaries" or pin the binary count.

**V-06 [P4] E6/E7a restricted refusal accepts any `LoweringError::Cfg`.**
`vm_canonical_inputs.rs:428-431` tolerates every CFG code for selectors 6
and 7, not the specific single-graph failure. Guarded by the Success
assertion under EXTENDED; already listed in the audit's adequacy notes.
Pin the code when the lowerer exposes it.

## 7. Security surface

No change from the 2026-09-04 surface verdict: no raw-bytecode entry point
(checker forbids `decode_bytecode`, `execute_bytecode`, `RawBytecode`,
`load_image`, `execute_loaded_image`, :88-96), input capped at 4096 bytes,
`Cursor::byte` wraps modulo a length the `len == 0` early return keeps
nonzero, `--manual` stays flag-gated outside every Make target, builds refuse
ambient `RUSTFLAGS`, toolchain resolution is explicit-path (no PATH
fallback except the Debian `clang-18` name), and the fuzz profile keeps
overflow checks and debug assertions on. The E8 bridge sub-lane's tamper
oracle asserts every mutant actually differs from the frozen rows (:1035-1039)
after `S20_700_VM_002`.

## 8. Disposition

Every report-grade item from the 2026-09-04 round is closed at HEAD by
code I read, checker pins I ran, and two live smokes I executed on the
pinned toolchain; the frontier claim "every Section 18.5 surface has a
landed target" is re-derived from the binaries and their production calls.
The row folds from FAIL to PASS with P3/P4 follow-ups. Owner actions: set
`s20_700_remaining_surface_audit.vulcan_review` to a PASS-prefixed string
carrying `0_P0_0_P1_0_P2_3_P3_4_P4` (the frontier checker accepts any
PASS*), and treat V-01/V-02 as the next target-round items.

```
VERDICT: PASS
SECTION: s20_700_remaining_surface_audit
FIELD: vulcan_review
SCOPE_SHA: 43f2f5ba738b587a9a0bb365a55f3096527e3e3d
FINDINGS:
[P3] [evidence] scripts/check_merge_persistent_fuzz_slice.py:105 - only check_vm_persistent_fuzz_slice.py:218-267 validates the durable last_local_proof (PASS, floor, crashes, sancov, ancestry, target diff); the other 19 slice checkers pin static fields only, so the tracked proofs for semantic_delta_decoder.rs (machine-summary.json:2280, a33e1e8) and complete_root_snapshot_decoder.rs (:2443, d3dee56) predate edits to those targets (fc98a89, 209c661) and every non-VM proof predates runner change 5277791 while make quick passes; the harness finals' "re-run at 209c661" lives only in gitignored evidence.json (merge tracked 698 at a33e1e8 vs runtime 638 at HEAD)
[P3] [tests] fuzz/regressions/ - three retained crashers lack a stable finding ID and a fuzz/regressions record: pack crash-35737a63 and crash-4b1158f7 (disposed only in vulcan_review-c7fec98.md:10,18 and machine-summary.json:2127-2140; inputs are tracked seeds via run_pack_persistent_fuzz.py:743-756) and graph-cfg crash-47e105f9 (S20_700_SEMANTIC_CHECKERS_PERSISTENT_SLICE.md:206-213 calls it a "permanent retest seed" but no tracked seed carries 02 ef 01 10 ff ef; retest depends on the gitignored artifact), against master goal 18.5's per-crash finding-ID and regression-test requirement
[P3] [tests] fuzz/targets/vm_canonical_inputs.rs:404-435 - prior item 10 (advisory): no lane refuses an unlanded E7 opcode under either profile now that the OpcodeUnsupported pin makes such a lane non-vacuous; contract section 6 excludes E7 beyond E7a/E8, so this stays a follow-up
[P4] [editorial] docs/audits/S20_700_VM_INPUT_PERSISTENT_SLICE.md:79 - "all 144 family seeds" is stale; the generator emits 162 (9 x 9 x 2) since E8
[P4] [editorial] scripts/run_vm_persistent_fuzz.py:355-359 - header comment lists limit_selector as a fixed byte; limits_profile is drawn after the inputs (vm_canonical_inputs.rs:67) and is pinned only by the uniform filler
[P4] [editorial] docs/WORK_PACKAGES.md:63 - "twenty persistent libFuzzer targets" vs 21 [[bin]] entries in fuzz/Cargo.toml (semantic-checkers slice has two binaries)
[P4] [tests] fuzz/targets/vm_canonical_inputs.rs:428-431 - E6/E7a restricted refusal accepts any LoweringError::Cfg code rather than the single-graph failure; guarded by the EXTENDED Success assertion and listed in the audit's adequacy notes
SUMMARY: HEAD 43f2f5b verified. The 2026-09-04 FAIL (1 P0, 2 P1, 6 P2, 3 P3) is re-derived closed at HEAD: the extended-family lane now builds each fixture's own request and requires Success termination with a result-canonical value, the restricted refusal pins VM_LOWER_OPCODE_UNSUPPORTED, the runner enforces a corpus-covering run floor with a parsed executed count, the records (docs, ADR-0039, contract section 5 rev 14, machine-summary lanes list and key names, checker pins) match the code, and the seed header is offset-stable. I ran the frontier checker and all 21 slice checkers (PASS), and executed make vm-persistent-fuzz-smoke (1576/1576) and make merge-persistent-fuzz-smoke (638/638 and 512/512) at HEAD on the canonical Arch clang-18.1.8 + nightly-2026-02-27 layout with no new crashes and the prior VM crasher retested clean; all eleven Section 18.5 surfaces have binaries that call their production entries. Remaining items are P3/P4: proof-record validation exists only in the VM checker so two adjacent slices' tracked proofs predate their target edits, three retained crashers lack finding IDs (one lacks a tracked regression seed), the advisory negative-E7 lane, and four editorial counts. PASS with follow-ups.
```
