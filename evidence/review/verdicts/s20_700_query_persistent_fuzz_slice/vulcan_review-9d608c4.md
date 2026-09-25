# Vulcan final-round review — s20_700_query_persistent_fuzz_slice (SCOPE_SHA 9d608c4)

**Verification notes before the block.** Execution of `python3` (checker) was permission-denied again in this session, so checker PASS is from static verification of every marker against files at HEAD: 20 target markers present (the 10 new ones at `restricted_query_request.rs:45,66/72,90,100,142,147,152,187/196,201,215`), 2 forbidden absent, 3 manifest, 27 runner, 3 Makefile, 9 machine-summary keys plus a `REVISE`-prefixed disposition, 4 doc markers. The proof was not re-run; it is assessed from on-disk `evidence.json` (mtime 00:09:15) and the register's `last_local_proof`, and I bound it to source independently rather than from the runner's claim: the fuzzer binary and the single `libsley_query-04c8a4b9bb7e73f8.rlib` in the target dir date 23:56:33, the target source's mtime is 23:56:06 and its worktree content equals HEAD (`git diff --quiet HEAD` clean), the proof's own build step was a cargo cache hit (`Finished in 0.02s`, so no source newer than the binary existed at 00:09), and `git diff 14904d3..4c30d1b` / `4c30d1b..HEAD` touch nothing under `crates/`, `fuzz/Cargo.toml`, `Cargo.lock`, this target, or this runner. The proof at `source_commit 4c30d1b` is therefore byte-identical in source to SCOPE_SHA for this slice. The engine and spec are unchanged since the 6b12d67 verdict (`git diff 6b12d67..HEAD --stat -- crates/sley-query docs/spec/RESTRICTED_QUERY_PROFILE_V1.md fuzz/Cargo.toml Makefile` is empty).

```
VERDICT: PASS
SECTION: s20_700_query_persistent_fuzz_slice
FIELD: vulcan_review
SCOPE_SHA: 9d608c439e12e303423b293ace09c97a893c6a63
DISPOSITION: PASS_0_P0_0_P1_0_P2_2_P3
ROUND: final (folds round-1 REVISE at e464ed4, round-2 REVISE at a12fcfb, revision-2 PASS at 6b12d67; all three transcripts preserved, none superseded as history)

VERIFIED AT HEAD (the five REQ-08 item-5 asks):
1. Sorted/dedup lanes reach binary_search and the step 3->5 transition: CONFIRMED.
   generated_kinds (:207-219) sorts+dedups on an even selector byte, yielding a
   strictly increasing set of <=12 kinds that passes validate_query_shape
   (query.rs:535) and, with a valid limit arm, builds; execute then resolves the
   entity (query.rs:630, step 5 UnresolvedEntity for id(4) on the narrow arm and
   for the raw-u32 arm) or enters select_edges where kinds.binary_search
   (query.rs:670) is the filter path, charging one unit per direct edge
   (:664, step 6 under the max_work=1 arm) and RequiredFactOmitted at :674-675
   (step 7 under max_returned_edges=1). sorted_entity_set (:196-205) does the
   same for closure seeds: strictly increasing seeds pass :541-549, resolve
   per-seed at :634-641, then reverse_closure (:680-716) charges per dequeue and
   per reverse edge and reports depth, with build_response (:753-758) applying
   entity/depth limits. The odd-selector lane keeps the unsorted tamper path,
   which is exactly the "sorted kind/seed sets with a separate tamper lane" the
   round-1 P3 asked for. Empty sets still die at step 3 on both lanes (correct).
2. Five-entity fixture arm widens closure with still-valid shapes: CONFIRMED.
   QueryFixture::wide (:393-426) adds Function id(4) (no parameters, entry and
   sole block id(5)) and Block id(5) (Branch to itself). Against
   ImpactIndex::build (lib.rs:461-496) the entity list is raw-ID sorted
   (1<2<3<4<5 by big-endian prefix, :471), all kinds are restricted
   (snapshot.rs:325-330), and every edge the collector derives resolves with the
   expected kind: F4->B5 ControlFlow+Ownership (lib.rs:738-751), B5->F4
   Ownership (:770-775), B5->B5 ControlFlow via target_edge (:1061,:1092-1103).
   Net: inventory 3->5, direct edges 7->11, a second disconnected component, so
   closure fanout reaches 5 entities (vs 3) and id(4) flips from unresolved
   (narrow) to resolved (wide) for the same generated request. Both the proof's
   1050 runs (half selecting wide) and the absent expect-panic at :441 confirm
   the builder accepts the shape.
3. Dual-defect probes pin the section-6 ladder deterministically: CONFIRMED.
   precedence_probes (:72-102) runs on every input after observe. Probe 1:
   all five limits over ceiling + kinds [Call, Ownership] (Ownership < Call in
   the derived Ord, lib.rs:246-271) on resolvable id(1) must be ResourceLimit;
   build_restricted_query_request calls validate_limits (:407) before
   validate_query_shape (:408), so step 1 beats step 3. Probe 2: same unsorted
   pair + entity 0xFFFF_FFFF under profile_maximum must be RequestNotCanonical;
   shape fails at :408 and resolution is never reached, so step 3 beats step 5.
   Both are constant, input-independent, and asserted with assert_eq on the
   mapped code, so any ladder reorder fails every run.
4. No change to canonicality/ceilings/ladder/modeled kinds: CONFIRMED.
   crates/sley-query and RESTRICTED_QUERY_PROFILE_V1.md are untouched since
   6b12d67; the target's caps (4096/16/16/4 kinds) and impact_kind mapping are
   unchanged; the diff is confined to the target (+164/-22), 7 checker markers,
   and the audit paragraph.
5. Fresh proof PASS with NEW coverage: CONFIRMED.
   evidence.json at 4c30d1b: result PASS, 1050/1050 runs against floor 1050
   over 531 files, counters 3883 (was 3510), ft 1316 INITED -> 1328 DONE with
   NEW at #698 and #997 (new_events=true; the 14904d3 commit records the same
   corpus going 1237->1316, i.e. the 955==955 plateau flagged at 6b12d67 is
   broken in two consecutive proofs), owner_lib_sancov=111 via cargo-json on
   the only libsley_query rlib present, zero crash/new/retested artifacts,
   unexpected_warnings empty, -len_control=0/-timeout=30/-rss_limit_mb=2048
   in argv, corpus persisted to 533 files. Worktree was dirty only with the two
   untracked .forge/slices files, which are not source.

RESOLVED FROM e464ed4 (round 1) — all closed, verified at HEAD:
- P1 zero-mutation smoke: closed at a12fcfb (floor = files+256); at HEAD 1050
  runs over 531 files with 519 mutated executions and NEW events.
- P2 oracle swallows InternalInvariant / builder-verifier disagreement: closed
  at 6b12d67 (:139-153), now checker-pinned (:29-31).
- P2 bin-only sancov: closed (host-config target rustflags, 111 owner symbols).
- P2 corpus wipe: closed (sync_seed_corpus :291-304, 533 files persist).
- P3 no minimize / crash-to-regression: closed (-minimize_crash=1, retest_prior_crashes).
- P3 adversarial thinness (unsorted sets, 3-entity fixture): CLOSED by this
  repair, see asks 1-2 above; the residual (limit arms 2/3 die at step 1 by
  design; empty sets die at step 3) is now a stated, not accidental, boundary.
- P3 shared timeout, P3 --locked, P3 checker hard-pin, P4 gitignored-only
  record: closed (a12fcfb), unchanged.
RESOLVED FROM a12fcfb (round 2): both P2s (linked-rlib gate, oracle asserts)
  closed at 6b12d67 and unchanged; P3 tail truncation, P3 proof-not-at-SHA,
  P4 minimize marker, P4 WARNING allowlist, P4 minimize is_file, P4
  len_control: closed.
RESOLVED FROM 6b12d67 (revision 2) P3/P4 list:
- P3 mtime fallback fail-open: CLOSED. linked_sley_rlibs (:549-552) returns
  ([], "cargo-json-failed"); owner_lib_sancov then sums to 0 and :143-151 FAILs.
- P3 adversarial thinness / ft plateau: CLOSED (ask 5).
- P3 checker pins none of the 7h repairs: PARTIALLY CLOSED — target side
  pinned by 7 new markers; runner side still unpinned (carried below).
- P3 audit doc stale: PARTIALLY CLOSED — target-closure paragraph appended
  (:109-126) records the ceiling and its repair; header section unchanged
  (carried below).
- P3 register carries first-round disposition: acknowledged as the
  register-builder rule (vulcan_review_note at machine-summary.json:2784):
  the round-1 row stays PENDING until this final-round filing; not a defect.
- P4 dead assert_ne on cross_code: CLOSED (removed, comment at :129-130).
- P4 parse_coverage docstring: CLOSED (:436-439).
- P4 warnings scanned on truncated stream: CLOSED (warning_lines over full
  streams :563-577, gate at :210-214).
- P4 retest without -timeout/-rss: CLOSED (:349-350).
- P4 replay fresh flag/duration unrecorded: MOOT — build was a 0.035 s cache
  hit and total duration 0.453 s leaves ~0.37 s for the cargo-json replay
  plus nm over 17 rlibs, nothing unattributed.
- P4 build-arm InternalInvariant guard: OPEN (carried below).

FINDINGS:
[P3] [contract] scripts/check_query_persistent_fuzz_slice.py:76-97 - the runner-side repairs that make the gates fail-closed are still not pinned: no marker for "cargo-json-failed" (mtime-fallback deletion), "new_crash_artifacts", "retest_prior_crashes", "KNOWN_BENIGN_WARNINGS"/"warning_lines", "ft_done", or "-len_control=0". Reintroducing the newest-mtime glob at run_query_persistent_fuzz.py:549-552, or dropping -len_control=0 at :248, passes the checker unchanged, so survival of the 7i-7m gate hardening is unenforced. Target-side pins are now complete; this is the remaining half of the 6b12d67 [contract] P3.
[P3] [record] docs/audits/S20_700_QUERY_PERSISTENT_SLICE.md:5-6,12-22,35-36 - the header section is not superseded in place: "A fixed valid three-entity function snapshot" (:5-6) is now one of two arms (the doc's own :119 says "Fixture count 1->2"); the oracle bullet list (:12-22) omits the engine-invariant/disagreement asserts and the two precedence probes; "Independent Vulcan review remains deferred because the local Forge OAuth session returns 401" (:35-36) is still asserted after three filed verdicts. The append-only round sections are accurate; the lead section contradicts them and is what a reader sees first. Machine-summary vulcan_review (:2802) and finding-register.json:2967 must carry this final-round disposition at filing, superseding the PENDING round-1 row per the builder rule.
[P4] [implementation] fuzz/targets/restricted_query_request.rs:111 - build arm still has no code != InternalInvariant guard. Latent only: I re-confirmed the builder's error paths (validate_limits ResourceLimit, validate_query_shape RequestNotCanonical, completeness Unsupported at :412, encode_query_preimage ResourceLimit only) are unchanged since 6b12d67. One assert makes "InternalInvariant nowhere" structural.
[P4] [implementation] fuzz/targets/restricted_query_request.rs:72-102 - the probes are input-independent, so 1050 runs execute the same two assertions 1050 times and contribute no fuzz-driven reach; the step-3-beats-5 probe asserts at build, where step 5 is never evaluated, so it pins call order rather than a live contention between two failing checks (the generated sorted lane already exercises step 5 at execute). They are correct deterministic pins; they also belong in crates/sley-query/src/query.rs tests, which have no dual-defect step-1-vs-3 fixture (spec §7 "error precedence have exact negative fixtures").
[P4] [record] docs/audits/S20_700_QUERY_PERSISTENT_SLICE.md:117-118 + fuzz/targets/restricted_query_request.rs:388-392 - "multi-depth reach ... become representable" overstates the arm: depth 2 was already reachable on the narrow fixture (seed B3 -> F1 at depth 1 -> P2 at depth 2 via reverse groups) and the wide arm's maximum depth is also 2. What the arm adds is a second component (closure size 5, 11 edges, id(4) resolvable). Record wording only; the widening itself is real.
[P4] [implementation] scripts/run_query_persistent_fuzz.py:129-130,145,149,210-214,234 - hygiene introduced in 7i-7m: duration_seconds assigned twice in the build-fail and sancov-fail branches; the unexpected_warnings comprehension and the problems f-string closing paren carry stray indentation (valid Python, but a governed script the checker reads by substring).
[P4] [record] scripts/run_query_persistent_fuzz.py:262-271 - the hand-built structured seeds encode the pre-repair byte layout (byte 0 = root flag, byte 1 = kind); the new leading fixture-selector byte (:44) shifts them, so they no longer target the kind/entity combinations they were written for. Harmless (the corpus replay alone lifted INITED ft 955 -> 1237 and libFuzzer found the lanes), and the 525 count the checker pins is unchanged, but the "constructors v1" seed grammar label no longer matches the target's layout.

SUMMARY: The fixture-depth repair does what REQ-08 item 5 asked and nothing else: two even/odd selector lanes hand canonical kind and seed sets to the engine so select_edges' binary_search filter, reverse_closure's depth and charging, and the step-5 UnresolvedEntity transition are reached instead of dying at validate_query_shape, while the odd lane keeps the tamper path; a second self-looping function (ids 4/5) doubles the fixture without changing any shape the builder accepts, verified against every edge the collector derives; and two constant probes assert the §6 ladder's step 1 > step 3 > step 5 ordering on every input. The engine, spec, ceilings, and modeled kinds are byte-identical to the 6b12d67 review. The proof is a genuine fresh PASS at source identical to HEAD (bound by mtimes, cache hit, and empty diffs, not by the runner's claim), and the 955==955 plateau the revision-2 verdict called the deferred ceiling is broken in two consecutive proofs (1237->1316->1328 with NEW events). Every blocking item from rounds 1 and 2 is closed and independently re-verified at HEAD; of the five revision-2 P3s, three are closed, two are narrowed to their remaining halves (runner-side checker pins; the audit doc's unsuperseded header), neither of which affects what the slice proves. Disposition: PASS_0_P0_0_P1_0_P2_2_P3, final round; this filing supersedes the PENDING round-1 REVISE row under the register-builder rule and stands beside, not in place of, the three prior transcripts. Assumptions: checker PASS is static marker verification (python3 denied); the proof was assessed from on-disk evidence.json and last_local_proof, not re-run; source-to-binary binding rests on cargo's mtime fingerprinting (a source file newer than the 23:56:33 binary would have forced a rebuild at 00:09, and none occurred) plus the empty git diffs between 14904d3, 4c30d1b, and HEAD; the wide-fixture edge set was derived by reading EdgeBuilder::collect rather than dumping the snapshot; the proof ran under the clang-22 override and the pinned clang-18 default remains unproven on this host, as the register states. No files were written.
```

**Evidence artifacts consulted (all read-only):**
- `fuzz/targets/restricted_query_request.rs` at HEAD (full) and `git diff 6b12d67..HEAD` on it
- `crates/sley-query/src/query.rs:402-783` (builder, executor, ladder, select_edges, reverse_closure, build_response), `lib.rs:246-271,461-496,720-793,1043-1108` (ImpactKind Ord, ImpactIndex::build, edge collection), `snapshot.rs:319-383`
- `docs/spec/RESTRICTED_QUERY_PROFILE_V1.md:194-224` (§6 ladder)
- `scripts/run_query_persistent_fuzz.py` (full) and `scripts/check_query_persistent_fuzz_slice.py` (full), plus their diffs since 6b12d67
- `docs/audits/S20_700_QUERY_PERSISTENT_SLICE.md` (full)
- `evidence/runtime/s20-700-query-request-libfuzzer/evidence.json` (mtime 00:09:15, source_commit 4c30d1b), corpus (533 files), single `libsley_query-04c8a4b9bb7e73f8.rlib` (23:56:33), binary (23:56:33)
- `machineresearch/sley-2.0/machine-summary.json:2782-2845`, `evidence/review/finding-register.json:2964-2993`
- Prior verdicts `vulcan_review-{e464ed4,a12fcfb,6b12d67}.md`; `REQ-08-next-wave-reviews.md`
- Git: commit list 6b12d67..HEAD, `git show --stat 14904d3`, diff stats 14904d3..4c30d1b and 4c30d1b..HEAD, file mtimes, `git diff --quiet HEAD` on the slice sources

**Handoff note (not written anywhere):** filing this verdict requires the records lane (merlin/argus) to create `evidence/review/verdicts/s20_700_query_persistent_fuzz_slice/vulcan_review-9d608c4.md` with the block above verbatim and to update `machine-summary.json:2802` and `finding-register.json:2967` to `PASS_0_P0_0_P1_0_P2_2_P3`, marking the round-1 row superseded. I did not touch any of those files.
