# Vulcan final-round review — s20_700_schema_persistent_fuzz_slice (SCOPE_SHA 9d608c4)

**Verification notes before the block.** `python3` execution (checker) was permission-denied in this session; `cargo`/`nm` were not attempted because the runner rewrites `evidence.json`, the corpus, and the target dir. Checker PASS is therefore static verification of every marker against files at HEAD: 8 target markers (`schema_bootstrap_decoder.rs:8,11,25,29,31,33,36,39`), 3 manifest (`fuzz/Cargo.toml:14,15,104`), 28 wrapper markers (all located in the full runner read, e.g. `--locked` :113, `-Zhost-config` :117, `trace-compares` :121, `must cover the corpus` :58, `-minimize_crash=1` :379, `corpus_file_count` :60, `-timeout=30`/`-rss_limit_mb=2048` :176-177), fixture contract + `SLEYEP01` magic (`bootstrap.json:2,13`), 3 Makefile (`:236-238`), 6 register fields incl. a `REVISE`-prefixed disposition (`machine-summary.json:1994-2003`), 4 doc markers (`14-property…md:27,111`, `25-evidence-gaps.md:36`, audit `:48`). Every marker is present.

**Source-change check since the second-round review (6b12d67):** `fuzz/targets/schema_bootstrap_decoder.rs` (last change 29bab98), `crates/sley-schema` (841d141), `fuzz/Cargo.toml`/`Cargo.lock` (9ec3ac3), `scripts/check_schema_persistent_fuzz_slice.py` (9ec3ac3), and the Makefile lane (cd3864a) are all untouched. **The runner did change**, in four family-wide commits: 800f7c7 (7i), d3dee56 (7j), 04336dd (7k), e7ad2dd. The first two are inside the proof binding; the last two are after it. I read both post-proof diffs in full: 7k deletes a duplicated `still_crashes` clause (no semantic change); e7ad2dd threads the build `env` into the cargo-JSON rlib replay, adds a minimize skip-list for clean-retested priors, replaces `-runs=1000000` with `-max_total_time`, and records `duration_seconds` on early exits. None of these touch a PASS-path gate (`executed_runs >= runs_floor`, `coverage_ok`, `new_crash_artifacts`, `still_crashes`, `unexpected_warnings`), so the d3dee56 proof's outcome is what HEAD's runner would produce on the same toolchain. The task premise "no source affecting this slice changed" is therefore false for the runner and true for everything the runner fuzzes.

**Proof binding:** register `last_local_proof` at `d3dee56` (PASS, 540/540 against floor 540 over 284 files, counters 2799, ft 510→510, `owner_lib_sancov` 41 via cargo-json, family 1279, zero crash/new/retested artifacts, no unexpected warnings, dirty files = the two retained `.forge` slices only). `evidence/runtime/s20-700-schema-bootstrap-libfuzzer/` does not exist on this host, so the proof is assessed from the register transcription only. The schema slice was not among the four refreshed at 4c30d1b (432b535 touched pack/query/semantic/smp1). Seed count re-derived by hand: the fixture preimage is 84 bytes (8 magic + `01` version + `4a` length + 74 record; the prior transcripts' "85-byte" wording is off by one, with no consequence), giving 84 prefixes + 4 fixed + 4 ladder + 168 flips − 1 duplicate (prefix[8] == magic) = 259 = `EXPECTED_SEED_COUNT`.

```
VERDICT: PASS
SECTION: s20_700_schema_persistent_fuzz_slice
FIELD: vulcan_review
SCOPE_SHA: 9d608c439e12e303423b293ace09c97a893c6a63
DISPOSITION: PASS_0_P0_0_P1_0_P2_4_P3_3_P4
ROUND: final (folds round-1 REVISE at e464ed4, round-2 REVISE at a12fcfb, revision-2 PASS at 6b12d67; all three transcripts preserved, none superseded as history)

VERIFIED AT HEAD (REQ-08 item 6 asks):
1. Slice target: CONFIRMED. schema_bootstrap_decoder.rs is byte-identical to
   every prior round (last change 29bab98); it bounds input at 2048 (:21),
   imports via import_bootstrap_preimage (:25), re-encodes through
   canonical_bytes + bootstrap_preimage and asserts byte equality with the
   input (:28-33), and asserts schema_epoch_id() == the importer's derived id
   (:34-40). Fuzz profile carries overflow-checks + debug-assertions
   (fuzz/Cargo.toml:115-117), so arithmetic faults in the decoder abort.
2. Runner: CONFIRMED. --locked host-config build with sancov level 4, inline
   8-bit counters, trace-compares, pc-table on target units only (:103-127);
   dedicated 1200 s build timeout (:33); RUSTFLAGS/CARGO_ENCODED_RUSTFLAGS
   refused as BLOCKED (:622-627); owner-rlib gate on libsley_schema-* > 0 and
   family total > 0, fail-closed (:140-158); rlib provenance from a cargo-JSON
   replay of the recorded argv with the build env, no mtime fallback (:477-526);
   floor = max(--runs, on-disk files + 256) (:60-61); strict counters +
   monotonic ft gate (:200-207); -len_control=0 / -timeout=30 / -rss_limit_mb
   in argv (:171-183); new-vs-prior crash split (:208-212); prior-crash retest
   in an isolated dir (:292-342); -minimize_crash=1 with exact artifact path,
   bounded by -max_total_time, partial artifacts kept (:345-404); WARNING gate
   over full streams against a three-line allowlist (:234-238, :527-551).
3. Checker: CONFIRMED present for all 43 markers (static); prefix-only
   disposition acceptance at :116; runs before the runner in the Makefile lane
   (:236-238) and inside persistent-fuzz-all (:333).
4. Corpus discipline: CONFIRMED. generate_seed_corpus writes 259 deterministic
   seeds and sync_seed_corpus removes only stale seed-* files (:265-278,
   :649-687); libFuzzer additions persist (proof: 284 files over 259 seeds);
   artifacts dir is append-only and retested every smoke; the seed-count drift
   check fails closed (:62-65).
5. Oracle agreement: CONFIRMED at the lane level. The checker pins the two
   target assertion strings and the re-encode/identity call chain; the runner
   gates execution/coverage/crash/warning; make schema-persistent-fuzz-smoke
   runs both. The oracle itself is differential: sley-schema's importer already
   enforces magic, minimal version and length uvars (lib.rs:1215-1217), no
   trailing data (:475), and canonical_bytes()==record_bytes (:480), so the
   target's equality asserts restate the importer's invariants from the
   outside (regression detector) plus panic/overflow detection; they add no
   independent invariant. Same characterization as the prior rounds, stated
   more precisely.
6. No source change since 6b12d67: CONFIRMED for the fuzzed code (target,
   sley-schema, manifest, lock), checker, and Makefile; NOT TRUE for the
   runner (4 family-wide commits, 2 after the proof binding, none touching a
   PASS-path gate). See P3 #1.

RESOLVED FROM e464ed4 (round 1) — all closed, re-verified at HEAD:
- P1 zero-mutation smoke: closed (floor = files+256, :61; proof 540 over 284).
- P1 bin-only sancov: closed (host-config target rustflags, owner gate 41).
- P2 corpus wipe / no minimize / no crash-to-regression path: closed
  (sync_seed_corpus; retest_prior_crashes; -minimize_crash=1 exact artifact).
- P2 shared build/fuzz timeout: closed (BUILD_TIMEOUT_SECONDS=1200).
- P2 --locked absent: closed (:113, derived into build_locked :611).
- P2 checker hard-pin on DEFERRED string: closed (:116 prefix test).
- P2 marker-only checker: carried at P3 (#2 below).
- P3 evidence only in gitignored path: closed (last_local_proof in register).
- P3 thin oracle / P3 seed neighbourhood: carried at P3 (#3 below).
- P4 audit 401 narrative + "make fuzz-smoke" wording: partially closed
  (superseded note at audit:39); carried at P3 (#4 below).
RESOLVED FROM a12fcfb (round 2): both P2s (-merge=1 primitive; seed-count
  floor) closed at 6b12d67 and unchanged; P3 substring ft gate, owner-rlib
  gate, trace-compares/pc-table, ASan statement, P4 401/toolchain fields:
  closed and unchanged.
RESOLVED FROM 6b12d67 (revision 2) P3/P4 list:
- P3 register lag: acknowledged as the register-builder rule
  (vulcan_review_note at machine-summary.json:1990); the round-1 row stays
  PENDING until this filing; not a defect.
- P3 seed neighbourhood, P3 marker-only checker, P3 pinned toolchain unproven:
  carried (#3, #2, #1).
- P4 retest cannot change outcome: CLOSED (new_crash_artifacts split :209-212,
  gate :243).
- P4 minimize budget mismatch / re-minimize every prior: CLOSED
  (-max_total_time :380; skip-list for clean-retested priors :225-229,
  :364-367).
- P4 ambient RUSTFLAGS displaces sancov: CLOSED (refused as BLOCKED :622-627).
- P4 no --seed passthrough: OPEN (carried, #P4-2).

FINDINGS:
[P3] [record] machineresearch/sley-2.0/machine-summary.json:2007-2044 + scripts/run_schema_persistent_fuzz.py:477-526 - proof provenance: the recorded PASS binds to d3dee56, a runner whose cargo-JSON replay ran without the build env; per the e7ad2dd commit message and audit:101-102 that replay rebuilt (blake3-class build scripts re-run under a different CC) and relinked the fuzzed binary after the recorded build, so the instrumented binary that executed and the rlibs nm counted are the replay's, not the recorded build's (Rust units and sancov flags are argv-config and identical either way, so the numbers stand). HEAD's runner (04336dd, e7ad2dd) has produced no proof; the evidence directory is absent on this host so evidence.json, the 284-file corpus, and the empty artifacts dir could not be inspected; and the proof runs under toolchain_overridden (clang 22.1.8) with the pinned clang-18 default still unproven on any host. REQ-08's preamble asks items 4-7 to carry regenerated proofs; this slice was not refreshed at 4c30d1b. Regenerate one proof at HEAD in the filing commit and retain it; the disposition does not depend on it because no PASS-path gate changed between d3dee56 and HEAD.
[P3] [contract] scripts/check_schema_persistent_fuzz_slice.py:61-84 - checker remains substring presence and reads no evidence; none of the 7i-7l runner hardening is pinned: no marker for "cargo-json-failed" (mtime-fallback deletion), "new_crash_artifacts", "retest_prior_crashes", "KNOWN_BENIGN_WARNINGS"/"warning_lines", "ft_done", "-len_control=0", or the RUSTFLAGS refusal. Reintroducing the newest-mtime glob at :523-526 or dropping -len_control=0 at :175 passes the checker unchanged, so survival of the gate hardening is unenforced; oracle/checker agreement holds for the Makefile lane as a whole (:236-238), not for the checker alone. Carry-over from e464ed4/6b12d67 at unchanged severity; the runner side is the remaining half.
[P3] [implementation] scripts/run_schema_persistent_fuzz.py:662-676 + fuzz/targets/schema_bootstrap_decoder.rs:25-40 + crates/sley-schema/src/lib.rs:475,480,910,1215 - seed neighbourhood and oracle unchanged since 6b12d67: all 259 seeds are prefixes, single-bit flips, or zero-padded ladders of one 84-byte fixture with empty contracts/extensions/migration_contracts; the four ladder seeds are rejected at the trailing-data check (:475) carrying the original record, so no seed reaches decode_epoch_record (:910) list/limit branches with a multi-byte uvar or a non-empty descriptor set, and those branches depend on CMP-assisted mutation within 256 executions. The proof's ft 510 -> 510 with new_events=false is consistent with saturation on this neighbourhood. The oracle is differential only (importer already enforces every invariant it asserts, incl. header-uvar minimality at :1215-1217). Bounded smoke as the audit states (:70-71); carry-over, not blocking.
[P3] [record] docs/audits/S20_700_SCHEMA_FUZZ_SLICE.md:22,32-37 + machineresearch/sley-2.0/machine-summary.json:2003 + evidence/review/finding-register.json:3102,4682 - the audit header is not superseded in place: ":22 routine `make fuzz-smoke` coverage" still describes the persistent target, but fuzz-smoke (Makefile:161-167) runs only the bounded cargo-test slice (carried from e464ed4 P4); ":32-37 could not start because the local Forge OAuth session returned 401 ... remains deferred" is still asserted as lead text after three filed verdicts, with only a one-line "Superseded" note beneath (:39). At filing, machine-summary vulcan_review (:2003) and both finding-register rows (:3102, :4682) must carry this final-round disposition, superseding the PENDING round-1 row under the builder rule, and the vulcan_review_note at :1990 should be retired or updated.
[P4] [implementation] scripts/run_schema_persistent_fuzz.py:151,155,234-238,258 - hygiene introduced in 7i-7l: duration_seconds assigned twice in the sancov-fail branch; the unexpected_warnings comprehension and the problems f-string closing paren carry stray indentation (valid Python, but a governed script the checker reads by substring).
[P4] [implementation] scripts/run_schema_persistent_fuzz.py:159-169,171-183,423 - libFuzzer's seed is parsed and recorded (:423) but there is no --seed passthrough into the smoke argv, so a recorded smoke cannot be replayed without editing the script; --manual lacks -timeout/-rss_limit_mb/-len_control parity (documented as operator exploration, audit:92-94). Carry-over from 6b12d67.
[P4] [implementation] scripts/run_schema_persistent_fuzz.py:62-65,652-660 - a seed-count or fixture-structure drift raises SystemExit before evidence.json is written, so the Makefile lane fails with no evidence record for that class, whereas toolchain problems write a BLOCKED record (:94-99). Fail-closed and correct; recording the drift as a BLOCKED problem would make the two failure classes uniform.

SUMMARY: Every blocking item from rounds 1 and 2 is closed by code at HEAD and independently re-verified: the smoke executes the on-disk corpus plus 256 guaranteed mutations, the host-config build instruments the owner rlib (41 sancov symbols in libsley_schema via fingerprint-authoritative cargo-JSON linkage, fail-closed with no mtime fallback), the corpus persists with stale-seed sync, crashes are split new-vs-prior, retested in isolation, and minimized with the correct -minimize_crash=1 primitive under a bounded budget, and the checker accepts filed dispositions. The fuzzed code, target, manifest, lock, checker, and Makefile lane are byte-identical to the revision-2 PASS at 6b12d67; the runner absorbed four family-wide hardening commits that close four of the five revision-2 P4s, and the two commits after the d3dee56 proof binding touch no gate that decides PASS. What remains is provenance and adequacy, not correctness: the standing proof predates the env-threaded replay, is not inspectable on this host, and runs under the clang-22 override; the checker does not pin the runner-side hardening; the seed neighbourhood and differential oracle make this a bounded smoke of the SLEYEP01 importer, as the audit now says; and the audit's lead section still contradicts its own appended rounds. Disposition: PASS_0_P0_0_P1_0_P2_4_P3_3_P4, final round; this filing supersedes the PENDING round-1 REVISE row under the register-builder rule and stands beside, not in place of, the three prior transcripts. Assumptions: checker PASS is static marker verification (python3 denied); the proof was assessed from the register's last_local_proof only because evidence/runtime/s20-700-schema-bootstrap-libfuzzer/ is absent here; the "replay relinked the binary" reading of the d3dee56 run is taken from the e7ad2dd commit message and audit:101-102, not observed; the claim that post-proof runner changes cannot alter the PASS outcome rests on reading both diffs in full, not on re-execution; REQ-08's "regenerated proofs" rule is read as targeting the redesigned slices (pack/query/semantic), and the operator may instead read it strictly, in which case the disposition holds but filing should wait for a HEAD proof; the seed count was re-derived by hand from the generator, not by running it. No files were written.
```

**Evidence artifacts consulted (all read-only):**
- `evidence/review/requests/REQ-08-next-wave-reviews.md`; prior verdicts `s20_700_schema_persistent_fuzz_slice/vulcan_review-{e464ed4,a12fcfb,6b12d67}.md`; sibling final-round verdicts `s20_700_query_persistent_fuzz_slice/vulcan_review-9d608c4.md` and `s20_700_vm_persistent_fuzz_slice/vulcan_review-9d608c4.md` (severity calibration for the shared proof-provenance condition)
- `fuzz/targets/schema_bootstrap_decoder.rs` (full); `crates/sley-schema/src/lib.rs:355-484,1195-1238`; `fuzz/Cargo.toml:14-15,104,115-117`; `conformance/schema-epoch/v1/bootstrap.json` (full)
- `scripts/run_schema_persistent_fuzz.py` (full) and `scripts/check_schema_persistent_fuzz_slice.py` (full); `git diff 6b12d67..HEAD`, `d3dee56..04336dd`, `04336dd..e7ad2dd` on the runner; `git log` per slice file
- `Makefile:1,161-167,236-238,320-333`; `docs/audits/S20_700_SCHEMA_FUZZ_SLICE.md` (full); `machineresearch/sley-2.0/14-property-fuzz-and-adversarial-results.md:27,111`; `25-evidence-gaps.md:36`
- `machineresearch/sley-2.0/machine-summary.json:1988-2046` plus all `source_commit` bindings; `evidence/review/finding-register.json:3100-3128,4681-4690`
- Git: `rev-parse HEAD`, `status`, commit list `6b12d67..HEAD`, `git show --stat 432b535`, commit messages for `04336dd`, `e7ad2dd`, `7cf25ac`; `ls evidence/runtime/`

**Handoff note (not written anywhere):** filing this verdict requires the records lane (merlin/argus) to create `evidence/review/verdicts/s20_700_schema_persistent_fuzz_slice/vulcan_review-9d608c4.md` with the block above verbatim; update `machine-summary.json:2003` and `finding-register.json:3102` / `:4682` to `PASS_0_P0_0_P1_0_P2_4_P3_3_P4`, marking the round-1 row superseded; and retire or update the `vulcan_review_note` at `:1990`. Recommendation: run `make schema-persistent-fuzz-smoke` at HEAD in that same records commit and transcribe the fresh `last_local_proof` — it closes P3 #1 and satisfies the REQ-08 letter at the cost of one ~1-minute smoke. I did not touch any of those files.
