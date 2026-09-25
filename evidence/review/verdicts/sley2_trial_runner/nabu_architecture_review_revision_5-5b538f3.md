<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 5b538f36eaee0574027539e512760a05e6d3ae29; role: nabu; field: nabu_architecture_review_revision_5; dispatched: 2026-09-23T06:38:23Z; duration_s: 1473; process_exit_code: 0 -->
# Nabu Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 5b538f36eaee0574027539e512760a05e6d3ae29

What I verified myself. I made no tree edits. All checks below were run by me at HEAD 5b538f36:

- **Scope:** `git rev-parse HEAD` returned `5b538f36eaee0574027539e512760a05e6d3ae29`, which matches the scope SHA.
- **Git reads:**
  - `git log --oneline ab42a3a9..HEAD`: 7 commits.
  - `git diff --stat ab42a3a9..HEAD`: 37 files, +2472/−362.
  - Full diffs of `crates/`, `bench/live/{sley2_tool,mediated_sley,mediated_shim,mediated_attempt,tooling,succ_witness_context,succ_witness_sig}.py`, `bench/sley2/runner.py`, `scripts/check_sley2_trial_runner.py`, `bench/fixtures/sley2_live_judge.py`, the docs and records, `machine-summary.json`, `finding-register.json`, `ga-acceptance-report.json` and `secret-scan.json`.
  - `git diff --stat` for `44f18e2b..8db44154`, `8db44154..e0146f8d` and `e0146f8d..HEAD`. The last range touches records and counters only.
- **Checkers:**
  - `python3 scripts/check_sley2_trial_runner.py`: exit 0, `"result": "PASS"`, revision 5, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, problems `[]`.
  - `python3 scripts/check_finding_register.py`: exit 0, PASS, problems `[]`.
- **A4 two-direction replication.** I extracted `git archive HEAD` into temp directories outside the tree and edited only the summary in each:
  - `contract_revision` 4: exit 1, `machine-summary:contract_revision:4!=spec:5`.
  - Aligned: exit 0, PASS.
  - COMPLETE with base fields only: exit 1, `completion-unbound-review` ×3.
  - COMPLETE with `_revision_5` PASS: exit 0, PASS.
  - COMPLETE with only `_revision_4` PASS: exit 1, `completion-unbound-review` ×3. A stale binding fails closed.
- **Python suites:**
  - `unittest discover -s bench/sley2/tests`: Ran 23, OK.
  - `bench.live.tests.test_tooling`: Ran 12, OK. Includes `Sley2CommandSurfacePinTests` ×4 and `RootQueryContractTests` ×5.
  - `test_sley2_tool.Sley2ToolSurfacePinTests`: Ran 2, OK.
  - With `SLEY2_SLEY_BINARY` built from HEAD and `TMPDIR` on `/var/tmp` (removed afterwards):
    - `test_agent_access`: Ran 15, OK, 391.5 s.
    - `test_mediated_access`: Ran 15, OK.
    - `test_mediated_gateway`: Ran 10, OK.
    - `test_sley2_tool`: Ran 22, OK.
  - Integrated `test_mediated_context`, two tests (`test_context_discovery_and_repair_accepted_end_to_end`, `test_context_incomplete_discovery_rejects`): Ran 2 in 382.6 s, OK, exit 0. For the other five integrated tests I rely on the record (§3, §10.3).
- **Cargo (preset `CARGO_TARGET_DIR`):**
  - `cargo test --locked --offline -p sley-repo --lib index_cache`: 9 passed, including `probe_reports_only_a_materialized_snapshot_and_never_builds`.
  - `-p sley-protocol --lib workspace_open`: 1 passed.
  - `-p sley-cli --test cli handshake_identity_does_not_depend_on_the_transport_flag`: 1 passed. I ran this only to build the HEAD `sley` binary.
  - `-p sley-repo --test succ_live_judge_cases -- --list`: built the judge-case driver `…-42d77a02eb446c77`.
- **Hashes (`sha256sum`):**
  - rev8 transcript: `e617e0c1db180d38db658cf288b49804ebd360a21da89dc914e5c8d539b4b13b`.
  - rev9 packet: `dbfe12f3ac5429e1187eb315c81ac2d6539df51f000cc4d79de292eeca1a9aa5`.
- **Own sweep for no-argument `revision` callers** (`git grep` over `bench`, `scripts`). The only remaining hits are intended negatives or synthetic labels:
  - `mediated_client.py:476`
  - `test_mediated_gateway.py:78`
  - `test_sley2_tool.py:53`
  - `test_mediated_context.py:224`
  - `test_mediated_access.py:137`
  - `test_acceptance_repairs.py:537,538,552`
- **Machine summary:** the `sley2_trial_runner` section has no `_revision_*` keys. The three base lane fields are byte-equal to `ab42a3a9`. The only changed keys are `contract_revision`, `implementation_complete`, `status` and `status_note`.
- **Files read, with line ranges:**
  - Full: the rev9 verdict, the rev9 packet, the rev4 packet, and the gate record (1–764).
  - REQ-10 rev2 packet :40–74.
  - `index_cache.rs` :60–151 and :150–410.
  - `server.rs` :2436–2475 and :2675–2734.
  - `sley2_live_judge.py` :2975–3200, :3405–3493 and :3895–3940.
  - `sley2_tool.py` :165–295 and :1140–1178.
  - `mediated_client.py` :1–125 and :423–637.
  - `test_mediated_context.py` :52–231.
  - `test_tooling.py` :64–96 and :285–435.
  - `test_sley2_tool.py` :30–80.
  - `sley-txn/src/maintenance.rs` :40–110.
  - `tooling.py` :82–111, plus its diff.
  - `SMP1.md` :640–680.
- **Environment notes:**
  - The sandbox refused two commands that used `$?`. I re-ran both through a python3 wrapper.
  - While I was working, two untracked files appeared: `evidence/review/verdicts/sley2_trial_runner/{ariadne_contract_review,vulcan_surface_review}_revision_5-5b538f3.md`. They are other lanes' output. I did not write them and did not read them. Tracked files are otherwise clean.

## Evidence checked

- **rev9 edit 4.2 (TOOLING).** `tooling.py:87-89` now has `sig ENTITY_HEX`, `open` and `revision TX_HEX` on separate lines; the fused line is gone. The `open`/`revision` sentence is at `:107-112`. The pin at `test_tooling.py:72-78` asserts set equality between `ALLOWED_COMMANDS` and the documented command lines, which is stricter than the substring pin rev9 asked for. It passes.
- **rev9 P2 (caller inventory and no-argument negative).**
  - The record's §2b table matches my own sweep.
  - `sley2_tool.py:765-768`: `revision` takes exactly one `_hex(rest[0], 64)` and never reads `session.head`.
  - Negatives present and passing: `test_dispatch_arity_refuses_before_any_server_contact` (no argument, 31 and 33 bytes, non-hex, two arguments, `open 00`) and `test_revision_without_tx_is_a_recorded_refusal`. The integrated `noarg_revision` test I rely on from the record.
- **rev9 P3 (terminator literal).** Kept verbatim at spec `:302`, with a parenthetical after it. The checker ends its region at `:139`. The checker passes.
- **rev9 P3 (shim).** `mediated_shim.py:24` adds `open` to `READ_COMMANDS`. The pin at `test_tooling.py:86-90` checks it.
- **rev9 P3 (field-9 constraint).**
  - The spec bullet at `:309` states that absence is structural and is never `bounds.omitted` or `bounds.truncated`.
  - `server.rs:2899-2903` still calls `counted(summary, 1)`.
  - `server_tests.rs:619` asserts omitted 0 and not truncated on both the cold and warm paths, and asserts that `revision.read` stays byte-identical.
  - `head_open_summary` (`:3274`) is kept separate from `revision_summary`, so the rev4 shared-encoder leak is closed.
- **rev9 P3 (hash).** Gate record §9 cites the full rev8 value, and it matches my `sha256sum`. The committed rev9 packet is left as history.
- **rev9 P4 (module docstring).** `sley2_tool.py:25-29` lists `open` and `revision <tx-hex>`. There is a test for it.
- **Probe.**
  - `index_cache.rs:256-270` is `read_record` + `accept_cached(..).ok()`, with no `fresh_snapshot` and no `write_record`.
  - The guard-mismatch check is the only error path.
  - The test at `:569` shows: cold returns `None` with no file; the hit survives object-store removal; a corrupted record returns `None` and is not rewritten.
  - The server test shows the cold `workspace.open` leaves no cache file.
- **Dependency direction.**
  - Rust: `sley-protocol → sley-repo` is an existing edge; only a new symbol is used. No new crate edges.
  - Python: `test_sley2_tool → bench.sley2.runner` is the edge rev9 Item 1 sanctioned. One new edge, from a witness into the test adapter, is P4-2 below.
- **Judge continuation change (deviation 7.3).** The logic is correct against the server. On a continuation page, `omitted = total_count - returned` (`server.rs:2710`), so only `truncated` means more to fetch. The new rule (`sley2_live_judge.py:3165`, `:3474`) tightens a stop on a truncated continuation page. Its regressions pass in my runs. The record routes the independence question to Vulcan, and I leave it there.
- **Other deviations.**
  - No `_revision_4` fields: consistent with rev8 Fix 3.
  - Direct witness reads in a single page: consistent with the per-invocation scope; see P3-2.
  - Corpus task-input amendment not made: P4-4.

## Findings

[P2] [ownership] docs/spec/SMP1.md:647,676-680 - The wire spec that owns method 201 still says `workspace.open` returns `revision_summary` of the accepted head, and the grammar at :678-680 is eight fields. The server now emits field 9 on warm opens (crates/sley-protocol/src/server.rs:2899-2916,3274-3283). The only normative statement of that wire body is in a consumer contract (docs/spec/SLEY2_TRIAL_RUNNER_V1.md:309-329), so S20-620 now defines another package's response body, and the owning package's record and status did not move. Gate record §7.5 (GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:351-355) defers this to "the Ariadne composition lane". But REQ-10-rev4:36-37 says Ariadne "receives only the resulting profile text", which means Ariadne reviews text someone else writes, and the rev4 verdict held that placement is resolved in this lane. No edit list from rev4 to rev9 named this file, including mine. - Closure: amend SMP1.md row 201 and Appendix A to show fields 1-8 plus an optional `9: IndexSnapshotId`, workspace.open only, structurally absent when not materialized, counting one item, with `revision.read` unchanged. Name the owning package and give its status disposition, and hand that text to the Ariadne lane. Alternatively, an owner-ratified record that explicitly keeps SMP1 divergent, with the owning section showing that.

[P3] [fail-closed] crates/sley-protocol/src/server.rs:2906-2916 (with :2443-2447) - The probe reuses `self.maintenance()`, so every `workspace.open` now runs `initialize_repository_maintenance`: create_dir plus three fsyncs (crates/sley-txn/src/maintenance.rs:58-98). It then takes a blocking shared lock (:107-111). Before this change `workspace.open` touched no lock. Three consequences: (1) it now waits behind a GC exclusive owner (crates/sley-repo/src/gc.rs:488), (2) a concurrent `exchange.import` exclusive try-lock (crates/sley-repo/src/exchange.rs:1899) can now fail WouldBlock because of an opener, and (3) the harness's own open in every tool session (bench/live/sley2_tool.py:271-283) pays this cost. This contradicts the docstring's "fail-open … any probe trouble is absence" (server.rs:2906-2907) and the spec's "mutates nothing" and "metadata-only" (SLEY2_TRIAL_RUNNER_V1.md:310,314). No test covers contention. - Closure: on this path use `acquire_shared_repository_maintenance_nonblocking` (maintenance.rs:142) without the initializer, reporting absence on WouldBlock or a missing boundary, with a test that holds an exclusive guard and gets the eight-field body back without blocking. Alternatively, state the initializing and blocking behavior, and why it is acceptable, in spec §9 and the docstring.

[P3] [authority] bench/fixtures/sley2_live_judge.py:3106-3107,3179-3184,3435-3437,3466-3484 - The judge defines continuation scope per tool invocation on the direct route, and per gateway `session_id` on the mediated route. The documented shim sets that id per process (bench/live/mediated_shim.py:65), while server continuation is stateless and cursor-based. So on the documented agent surface a protocol-correct `query.continue` can never pass. TOOLING documents `query.continue`, then tells the model to size pages so it never needs it (bench/live/tooling.py:156-162). The integrated positive proof gets its three continuations (bench/live/tests/test_mediated_context.py:118-127) through one persistent `mediated_transport.Gateway` (bench/live/mediated_client.py:976), which the agent contract does not document. The settled clause "actual continuation where needed" is therefore evidenced only for the stand-in. This is disclosed in gate record §10.1 and does not block the current 10-entity corpus. - Closure: either a scope the documented route can carry (for example a cursor-bound audit: a continue is consistent if it repeats a prior truncated page's request body with that page's next cursor, whatever the invocation), or spec §9, TOOLING and the acceptance text state that continuation is unavailable on the documented route and that the continuation leg is stand-in-only evidence.

[P3] [identity] bench/live/sley2_tool.py:74 - `TOOL_VERSION` stays `"1"`, and so does `MEDIATED_TOOL_VERSION` (bench/live/mediated_attempt.py:89). This version is the only tool identity the judge binds on per chain entry (sley2_live_judge.py:45-54,3069,3329). It did not move across an incompatible agent-surface change: `revision` went from zero to one argument, `open` was added, the raw set grew to 19 methods and TOOLING.md was rewritten. Revision-4 and revision-5 tool evidence therefore carry the same identity. `tooling_digest` (bench/live/tooling.py:255) still has no production consumer, only the determinism test at test_tooling.py:41. - Closure: bump both versions; the judge imports the constant, so the change has one source of truth. Add a test that a chain stamped with the prior version is rejected. Alternatively, bind `tooling_digest` into chain entries or capture start records, and say which identity governs.

[P3] [record] bench/live/SUCCESSION-COVERAGE.md:29 - At HEAD this row says "TOOLING.md lacks the root-query request layout" and cites `test_mediated_context.py` as 3 tests; there are 7. bench/live/MEDIATED-INPUT-ACCOUNTING.md:102-104 says TOOLING.md "does not document the root-query request layout". Commit 8db44154 added that layout (bench/live/tooling.py:120-163), pinned by `RootQueryContractTests`, and gate record :413-416 says it is done. The records at HEAD contradict the tree and the gate record. - Closure: update both rows to HEAD fact: layout documented and pinned, 7 integrated tests, live-model usability still not demonstrated.

[P4] [evidence] bench/live/succ-trials-20260923/trial_context_pos.log:1, trial_context_neg.log:1 - These logs were committed in 754a140d and produced by code that 8db44154 then changed: the warm query moved from class 1 to class 4 (bench/live/mediated_client.py:484-488). The logs carry no source or commit identity. - Closure: re-run the direct CONTEXT witnesses at HEAD, or annotate the logs with the commit they ran at.

[P4] [dependency] bench/live/succ_witness_context.py:43 - The direct witness now imports `context_discover_and_repair` from a module that describes itself as the "test-only adapter for mediated trials" running inside bwrap (bench/live/mediated_client.py:1-22). MEDIATED-INPUT-ACCOUNTING.md row 5 (:35) says that module is "injected only by tests". Sharing one scripted agent is the right call, but the module's declared role no longer matches its dependents. - Closure: amend the docstring and row 5, or move the scripted CONTEXT agent and root-query codec into a route-neutral test-support module.

[P4] [authority] bench/live/tests/test_tooling.py:304-309 - The budgets TOOLING states (1048576 and 4194304) are pinned only as string literals. They are not tied to `sley2_tool.MAX_RESPONSE_BYTES` (bench/live/sley2_tool.py:77) or to the judge's `AGENT_CUMULATIVE_RESPONSE_BUDGET` (sley2_live_judge.py:2984), so the agent contract can drift silently from the judge. - Closure: assert the documented numbers equal those constants.

[P4] [record] bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:329-335 - Deviation 7.2 leaves the settled task-input amendment (REQ-10-rev2:49-51) unimplemented. The stand-in instead depends on the fixture having exactly one Record-form typedef; it fails closed otherwise (bench/live/mediated_client.py:536-547). - Closure: the corpus owner ratifies dropping or deferring the amendment (with the digest-pin change) in the REQ-10 design chain, not only in an implementation record.

## Assessment

**Rev9 remedies.** Every remedy the rev9 verdict named has landed, and I checked each against code, tests or hashes rather than the record:
- edit 4.2, with a stricter pin than asked;
- the caller inventory plus three layers of no-argument negatives;
- the terminator kept verbatim;
- the shim's `READ_COMMANDS`;
- the field-9 structural-absence constraint in the spec, enforced by a server test;
- the corrected hash;
- the module docstring.

The two pin tests rev9 asked for exist and pass, so the load-bearing couplings are now mechanical.

**Snapshot cache probe.** It is genuinely read-only: no build, no write-back, and a discarded record reads as absence. The server keeps field 9 off `revision.read` by construction. The checker's Fix 1 and Fix 2 behave in both directions, and a revision-4-only binding also fails closed. The integrated positive and incomplete-discovery proofs pass at HEAD in my run. Dependency direction is sound apart from the P4 above.

**Deviations.** No `_revision_4` fields matches rev8 Fix 3. The single-page direct witness follows from the per-invocation scope. The corpus amendment needs ratification (P4-4).

**Why REVISE.** One P2, an architecture defect of the kind that has been recurring:
- The server now has a wire behavior that its owning protocol spec (SMP1.md) contradicts, and the only normative description lives in a consumer contract. The rev4 "Ariadne receives the resulting profile text" presumes that text exists.
- Nobody's edit list named SMP1.md, mine included. This is the eighth coupled enumeration, after the seven I listed at rev9.

**Remaining P3s.**
- The opener's new blocking, initializing lock (P3-1).
- A continuation-scope definition that exists only in the judge and that the documented route cannot satisfy (P3-2).
- A tool identity that did not move with an incompatible surface change (P3-3).
- Stale records (P3-4).

**Scope limits.** I make no GA or release-readiness claim, and I do not ratify the allowlist widening; that belongs to the allowlist owner.

Separately, the Google Calendar, Google Drive and Vercel connectors need authorizing in claude.ai connector settings (Vercel via `claude mcp`) before they can be used; this review did not need them.

VERDICT: REVISE_0_P0_0_P1_1_P2_4_P3_4_P4
SECTION: sley2_trial_runner
FIELD: nabu_architecture_review_revision_5
SCOPE_SHA: 5b538f36eaee0574027539e512760a05e6d3ae29
FINDINGS: [P2] [ownership] docs/spec/SMP1.md:647,676-680 - owning wire spec for method 201 still documents an 8-field revision_summary while server.rs:2899-2916,3274-3283 emits field 9 on warm opens; field-9 wire behavior is stated only in the consumer contract SLEY2_TRIAL_RUNNER_V1.md:309-329 and the owning package's record did not move; gate record §7.5 misreads rev4's "Ariadne receives the resulting profile text" as authoring - SMP1 row 201 and Appendix A amended (optional 9: IndexSnapshotId, workspace.open only, structural absence) with owning-package disposition and handed to Ariadne, or an owner-ratified divergence record | [P3] [fail-closed] crates/sley-protocol/src/server.rs:2906-2916 - opener reuses maintenance(): lock-boundary initialization plus 3 fsyncs and a blocking shared lock on every workspace.open; blocks behind GC (gc.rs:488), can make an import try-lock fail (exchange.rs:1899), contradicts "fail-open", "mutates nothing" and "metadata-only" - nonblocking shared acquire without the initializer (absence on WouldBlock) plus a contention test, or spec and docstring state the behavior | [P3] [authority] bench/fixtures/sley2_live_judge.py:3106-3107,3179-3184,3435-3437,3466-3484 - continuation scope is per invocation (direct) or per shim-pid session (mediated_shim.py:65), so a protocol-correct query.continue can never pass on the documented surface; the positive proof's 3 continuations use an undocumented persistent Gateway (mediated_client.py:976) - cursor-bound scope the documented route can carry, or spec, TOOLING and acceptance text declare continuation stand-in-only | [P3] [identity] bench/live/sley2_tool.py:74 - TOOL_VERSION and MEDIATED_TOOL_VERSION (mediated_attempt.py:89) stay "1" across an incompatible surface change; they are the judge's only per-entry tool identity (sley2_live_judge.py:3069,3329) and tooling_digest has no production consumer - bump both with a prior-version rejection test, or bind tooling_digest into chain or capture records | [P3] [record] bench/live/SUCCESSION-COVERAGE.md:29 - it and MEDIATED-INPUT-ACCOUNTING.md:102-104 say TOOLING.md lacks the root-query layout (and cite 3 integrated tests, not 7), contradicting tooling.py:120-163 and gate record :413-416 - rows updated to HEAD fact | [P4] [evidence] bench/live/succ-trials-20260923/trial_context_pos.log:1 - CONTEXT witness logs predate 8db44154's change to the code they run and carry no source identity - re-run at HEAD or annotate the commit | [P4] [dependency] bench/live/succ_witness_context.py:43 - the direct witness imports the self-declared test-only mediated adapter (mediated_client.py:1-22; accounting row 5) - docstring and row amended, or scripted agent moved to route-neutral test support | [P4] [authority] bench/live/tests/test_tooling.py:304-309 - TOOLING budget numbers pinned as literals, not to sley2_tool.MAX_RESPONSE_BYTES or judge AGENT_CUMULATIVE_RESPONSE_BUDGET - assert equality with those constants | [P4] [record] bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:329-335 - task-input amendment (REQ-10-rev2:49-51) left unimplemented; stand-in relies on a unique Record typedef (fails closed) - corpus-owner ratification recorded in the REQ-10 design chain
SUMMARY: REVISE. Every rev9 remedy is verified landed, and so are the read-only probe, the field-9 scoping and the checker's revision binding. The two integrated CONTEXT proofs I ran and the binary-backed surface suites all pass at HEAD. The one P2 is an ownership inversion: the server's new workspace.open field 9 is normatively described only in the trial-runner consumer contract, while the owning SMP1 wire spec still documents eight fields. Four P3s (opener lock behavior, continuation-scope authority, unchanged tool identity, stale records) and four P4 notes remain; none needs redesign.
