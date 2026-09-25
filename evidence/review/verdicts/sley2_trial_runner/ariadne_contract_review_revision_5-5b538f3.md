<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 5b538f36eaee0574027539e512760a05e6d3ae29; role: ariadne; field: ariadne_contract_review_revision_5; dispatched: 2026-09-23T06:38:23Z; duration_s: 725; process_exit_code: 0 -->
# Ariadne Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 5b538f36eaee0574027539e512760a05e6d3ae29

What I verified myself. The worktree was read-only throughout. I changed no tracked file and wrote no verdict field.
- Scope: `git rev-parse HEAD` returned 5b538f36eaee0574027539e512760a05e6d3ae29, which matches. `git log ab42a3a9..HEAD` lists 7 commits (c2e37384, 44f18e2b, 754a140d, 8db44154, e0146f8d, 20b423f3, 5b538f36). `git diff --stat ab42a3a9..HEAD` shows 37 files. I read the full diffs of the spec, checker, WORK_PACKAGES, closeout, machine summary, `crates/`, judge, live records, runner, register and derived evidence.
- `python3 scripts/check_sley2_trial_runner.py`: exit 0, `"result": "PASS"`, revision 5, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, problems []. This includes its own run of the `bench/sley2/tests` suite.
- `python3 scripts/check_smp1_contract.py`: exit 0, PASS, revision 12, status `S20_400_COMPLETE`.
- `python3 scripts/check_finding_register.py`: exit 0, PASS, problems [].
- In-memory checker probe (the summary `read` was monkeypatched; no files written), six cases:
  - drift (4 vs 5): FAIL `machine-summary:contract_revision:4!=spec:5`
  - aligned: PASS
  - COMPLETE without `_revision_5` fields: FAIL `completion-unbound-review` ×3
  - COMPLETE with PASS `_revision_5` fields: PASS
  - COMPLETE with only `_revision_4` fields: FAIL ×3
  - COMPLETE with REVISE `_revision_5` fields: FAIL ×3
- Allowlist probe:
  - `_spec_allowlist(spec)` returns 19 names, equal to `ARM_AFFORDANCES` and in the same order.
  - The tuple is sorted, with `workspace.open` last.
  - `ARM_AFFORDANCES` (19) and `ARM_DENIED_METHODS` (24) are disjoint, and together they equal the 43-method `conformance/smp1-json-bridge/v2/methods.json`.
- `python3 -m unittest -v bench.live.tests.test_tooling`: 12 tests OK. This covers the 5 `RootQueryContractTests`, the 4 `Sley2CommandSurfacePinTests` and the 3 `ToolingTests`.
- `bench.live.tests.test_sley2_tool.Sley2ToolSurfacePinTests`: 2 tests OK.
- Independent layout probe:
  - I encoded requests using only the TOOLING layout for the 5 vectors (class-02, class-04, class-14, page-namespaces-1, page-namespaces-2).
  - The echoed fields in each vector's `record_hex` match byte for byte.
  - BLAKE3(`sley2.root-query.v1` ‖ body), computed through the oracle uv project, equals every vector's `query_id`.
- `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 1 passed.
- `cargo test --locked --offline -p sley-repo --lib index_cache`: 9 passed.
- Not run: the 7 integrated execute_attempt proofs. They need the `SLEY2_SLEY_BINARY` and `SUCC_JUDGE_TEST_BINARY` environment variables, and this sandbox refuses the variable expansion. For them I rely on gate record §3 (7 tests OK in 1010.8 s) and §10.3 (219 live tests OK).
- Files read (line ranges):
  - Specs:
    - `docs/spec/SLEY2_TRIAL_RUNNER_V1.md` 1-347 (full)
    - `docs/spec/SMP1.md` 1-60, 290-822
    - `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` 60-449
    - `docs/spec/RESTRICTED_QUERY_PROFILE_V1.md` 40-169
    - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` 1-225
    - `docs/spec/SESSION_HANDLE_PROFILE_V1.md` 160-179
  - Checker and bench code:
    - `scripts/check_sley2_trial_runner.py` 1-318
    - `bench/live/tooling.py` 1-297
    - `bench/live/sley2_tool.py` 1-130, 265-304, 405-464
    - `bench/sley2/runner.py` 80-149
    - `bench/fixtures/sley2_live_judge.py` 2985-3044, 3120-3189, 3410-3492
    - `bench/live/tests/test_tooling.py` (HEAD)
  - Rust:
    - `crates/sley-protocol/src/server.rs` 1235-1294, 2681-2715, 2887-2916, 3172-3180, 3265-3300, 3441-3531
    - `crates/sley-repo/src/index_cache.rs` 237-270 (diff)
  - Records and reviews:
    - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md` 1-763
    - REQ-10 rev3 and rev4 in full; rev7, rev8 and rev9 excerpts (Fix 1-4, Items 1-5)
    - Nabu rev9 verdict (full)
    - `docs/adr/ADR-0036-sley2-trial-runner-boundary.md` (full)
    - closeout headings
    - `bench/live/MEDIATED-INPUT-ACCOUNTING.md` 84-107
    - `bench/live/SUCCESSION-COVERAGE.md` :29

## Evidence checked

- **Contract revision 5, §9.**
  - `SLEY2_TRIAL_RUNNER_V1.md:296-302` holds exactly nineteen names in the frozen order, with `workspace.open` appended last.
  - `bench/sley2/runner.py:95-115` and `bench/live/sley2_tool.py:94-114` match that order. The checker pins the first; `test_tool_methods_equal_runner_allowlist_in_order` pins the second.
  - The terminator literal `Those two entity names` is kept verbatim at `:302`. The antecedent is clarified by a parenthetical without backticks, so `_spec_allowlist` still stops there (the rev9 P3 is closed).
  - The Status line at `:3` reads revision 5, and `:40` says nineteen.
- **Checker.**
  - `:182-186` has one anchored, converted extraction of the revision, placed after `section` is bound, plus the drift assertion.
  - `:298-301` binds `f"{key}_revision_{spec_revision}"`. That yields `ariadne_contract_review_revision_5`, as rev7 Fix 2 and rev8 Fix 3 require.
  - `:306` reads the payload from the same hoisted value.
  - The in-memory runs above confirmed the gate works in both directions.
- **Machine summary** (`sley2_trial_runner`):
  - `contract_revision` is 5, `status` is `S20_620_IMPLEMENTED_REVIEW_PENDING`, and `implementation_complete` is false.
  - The three base fields are byte-exact `PASS_0_P0_0_P1_0_P2_0_P3`, with no `_revision_4` or `_revision_5` fields (as rev8 requires).
  - `status_note` is accurate, and `finding_register.complete_packages` is 36.
  - The register's three S20-620 lane rows carry `package_status` `S20_620_IMPLEMENTED_REVIEW_PENDING`.
- **Other records.** The WORK_PACKAGES row (`:59`) and the closeout Revision 5 paragraph (`:3-14`) agree with the summary.
- **TOOLING.md layouts.** I compared `bench/live/tooling.py:123-148` field by field against the protocol specs:
  - Request (ROOT_BACKED §5 `:320-337`):
    - Magic, format and profile, the four ids in the right order, completeness 2 and limits profile 1.
    - `query_limits` widths and ranges match RESTRICTED `:62-84, :109-111`.
    - `allow_continuation` is 1/2, and the option cursor is `u32 1 | u32 2 ‖ u32 1 ‖ EntityId`.
    - Class bodies 2, 4 and 14 are correct: list = `u64 count ‖ items`, and kinds 4 = TypeDef and 9 = Constant per `sley-query/src/lib.rs:131,136`.
  - Response (ROOT_BACKED §6 `:348-373`): the echo, total, returned, truncated 1/2, next cursor, depth, work, bytes and result tag are all correct. The class-2 option fingerprint and the class 4/14 list payloads are correct too.
  - The layouts match exactly, and the vector probe above confirms it independently.
- **SMP1 and the owner profiles.** For findings 1, 2 and 6 I read SMP1 §4 row 201 (`:316`) and appendix A (`:647`, `:676-680`), the revision 12 statements (`:53-58`, `:813-814`), the authority rule (`:30-34`), the body rule (`:377-379`), and S20-300 §5 and §9. I compared them against `server.rs` `workspace_open` (`:2899-2904`), `materialized_head_snapshot` (`:2908-2916`), `head_open_summary` (`:3274-3283`), the dispatch at `:1283`, and `index_cache.rs:256-270`.

## Findings

[P1] [contract-composition] docs/spec/SMP1.md:316,647,676-680 (with :30-34, :53-58, :813-814) - Ruling on the deliberately unedited row: it cannot stay as it is. `server.rs:2899-2904,3274-3283` now answers `workspace.open` with a 9-field record on a warm cache (the 8-field `revision_summary` plus field 9 `IndexSnapshotId`), and does so under every negotiated version, with no version gate. SMP1 still says four things that are now false: appendix A says the response is the 8-field `revision_summary`; revision 12 says version 1 bytes and the appendix A/C bodies are unchanged; the authority rule says every payload is an owner's frozen record; and §4 row 201 still says request "repository path digest", which disagrees with appendix A's "empty". The only normative definition of the new record is `SLEY2_TRIAL_RUNNER_V1.md:309-329`. That is a consumer contract, and its own Boundary (`:21-30`) says S20-620 "alters none of" the endpoint contracts it composes. - Closure evidence: an SMP1 revision (13) that defines the `workspace.open` response record (fields 1-8 owned by S20-390; optional field 9 owned by S20-300, taken from the metadata-only probe, and structurally absent when not materialized). It must reconcile §4 row 201 with appendix A, state which negotiated versions carry field 9 (or gate it to version 2, with a test), and amend the revision-12 "unchanged" statements. It needs `check_smp1_contract.py` PASS and the SMP1 owner-lane review. S20-620 §9 must then cite that definition instead of defining it, and the Boundary sentence must be made true.
[P2] [contract-composition] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:146-151,217-220 - S20-300 allows only S20-310 root-backed query surfaces to consume a cache hit, and says the hit path "still has no readers beyond those queries". The new public probe `crates/sley-repo/src/index_cache.rs:256-270` is a second hit-path reader that feeds `workspace.open`, which is not one of those surfaces. It accepts a cached record under the four rules, never rebuilds or writes back, and treats a discard as absence, unlike §5's discard-rebuild-write rule at `:135-137`. `SLEY2_TRIAL_RUNNER_V1.md:314-315` attributes the probe to "S20-300, `sley-repo`", but the S20-300 profile does not define it. REQ-10 rev3 (`:34-37`) sent exactly this cross-domain placement note to this lane. - Closure evidence: an S20-300 profile revision that defines `cached_complete_root_snapshot_id`: its cost class, the acceptance rules, the absence semantics and the guard rule. It must admit the `workspace.open` identity disclosure as a hit reader, with its authority limits, and correct §9. The S20-300 checker must pass.
[P3] [spec-precision] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:312-314,323-326 - Two statements in §9 are too strong. (1) "When and only when that snapshot is already materialized" overstates the "when" direction. `materialized_head_snapshot` (`server.rs:2908-2916`) and the probe return absence when the maintenance guard fails or a record is unreadable; this fail-open behavior is gate record §7.6. (2) The cold warm-up query is refused `QUERY_SNAPSHOT_MISMATCH` only when the engine answers the rebuilt request. `run_root_query` runs before the preimage comparison (`server.rs:2686-2700`), so owner failures such as `QUERY_REQUIRED_FACT_OMITTED` or `QUERY_RESOURCE_LIMIT` win. Materialization also holds only when the cache write succeeds (S20-300 §5 `:135-138`). With an unwritable index directory, the open→query→open loop §9 prescribes never discloses a binding. - Closure evidence: §9 reworded to "only when …; any probe failure is absence" and "refused (`QUERY_SNAPSHOT_MISMATCH` when the engine answers, else the owner code), materializing the snapshot when the cache write succeeds", or tests that pin whichever behavior is stated.
[P3] [agent-contract] bench/live/tooling.py:155-162 - On the documented one-request-per-invocation route, two things always reject the trial. Every `query.continue` is an inconsistent continuation (`sley2_live_judge.py:3152-3156`, and `:3464-3467` for the mediated route). Every truncated page (paging = 2), and every page sent with a cursor, leaves its scope pending, which rejects the trial (`:3166-3169,3182-3184`; mediated `:3475-3484`). TOOLING still presents paging = 2 and `query.continue` as usable and only advises sizing `max_entities`. It never states the ROOT_BACKED_QUERY_PROFILE_V1.md:269-272 rule: a consumer without continuation authority uses `allow_continuation = false` and treats `QUERY_REQUIRED_FACT_OMITTED` (a counted, recoverable refusal) as the answer. The byte layouts themselves are exact. - Closure evidence: TOOLING states that paging = 1 with no cursor is the only non-rejecting form on this route, and a sentence pin is added in `test_documented_layout_is_pinned`. Alternatively, the multi-invocation continuation decision (gate record §10.1) lands with its own review.
[P3] [record] bench/live/SUCCESSION-COVERAGE.md:29; bench/live/MEDIATED-INPUT-ACCOUNTING.md:102-104 - Both records still say TOOLING.md lacks, or does not document, the root-query request layout. That has been false since 8db44154 (`tooling.py:120-148`, pinned by `RootQueryContractTests`). The CONTEXT row also still cites "`test_mediated_context.py` 3 tests", but the file now holds 7 integrated proofs. - Closure evidence: both records corrected. Live-model usability can remain "documented, not demonstrated", as gate record §9 says.
[P4] [conformance] crates/sley-protocol/src/server.rs:1283 - The server ignores the `workspace.open` request body, so `raw workspace.open 00` succeeds. This contradicts `SLEY2_TRIAL_RUNNER_V1.md:310` ("takes no body"), `SMP1.md:647` (empty request) and `SMP1.md:377-379` (`PROTOCOL_PAYLOAD_INVALID` for a body that is not the owner's record). The behavior predates this change and affects a whole class of methods: `session.capabilities`, `session.budgets`, `exchange.export`, `refs.recover` and `recovery` ignore bodies too. It is newly reachable through the revision 5 affordance. - Closure evidence: an empty-body check with a test, or the SMP1 amendment from the P1 finding states the leniency.
[P4] [record] docs/adr/ADR-0036-sley2-trial-runner-boundary.md:3-5; docs/spec/SLEY2_TRIAL_RUNNER_V1.md:273 - The ADR's Status line still reads "draft at revision 2 … five offline tests", while the package is at revision 5 with 23 tests. The revision 5 normative bullet sits under the heading "9. Revision 2 clarifications". Both are pre-existing staleness that carried into the revision the checker binds. - Closure evidence: the ADR status refreshed, and the §9 heading made revision-neutral while keeping the `holds nineteen names` anchor and the `Those two entity names` terminator verbatim.

## Assessment

The trial-runner side of revision 5 conforms, and I checked it against the contract text, the checker output, and in-memory probes rather than the gate record:
- the nineteen names, their frozen order and the terminator literal;
- the drift gate and `completion-unbound-review`, both directions;
- the machine-summary `contract_revision`, status and `implementation_complete`;
- the WORK_PACKAGES row, the closeout and the finding-register chain;
- the TOOL_METHODS and ALLOWED_COMMANDS pins;
- the TOOLING byte layouts, which match RESTRICTED §3-4 and ROOT_BACKED §5-6 exactly and reproduce the conformance request identities.

The field-9 wire behavior is tested and matches what §9 describes: structural absence when cold, no omission signal, `revision.read` bytes unchanged, and the disclosed identity equal to the one `query.root` binds.

The defect is where that behavior is written down. A change to a server response body of a COMPLETE protocol contract (SMP1, revision 12) is specified only inside the S20-620 consumer contract, which contradicts SMP1's own "unchanged" and authority statements and S20-620's own Boundary. The S20-300 profile likewise does not admit the new hit-path reader. My ruling on the SMP1 row is that it must be amended in an SMP1 revision under its owner lane; §9 of the trial-runner spec cannot stand in for it. Revision 5 therefore cannot return to COMPLETE on this lane until the P1 closes.

The 7 integrated execute_attempt proofs were not re-run here. For those I rely on the gate record, and nothing in this verdict depends on them.

VERDICT: REVISE_0_P0_1_P1_1_P2_3_P3_2_P4
SECTION: sley2_trial_runner
FIELD: ariadne_contract_review_revision_5
SCOPE_SHA: 5b538f36eaee0574027539e512760a05e6d3ae29
FINDINGS: [P1] [contract-composition] docs/spec/SMP1.md:316,647,676-680 (with :30-34, :53-58, :813-814) - workspace.open row must be amended: server.rs:2899-2904,3274-3283 emits revision_summary plus optional field 9 under every negotiated version, but SMP1 appendix A still names the 8-field revision_summary, revision 12 says version-1 bytes and appendix A/C bodies are unchanged, the authority rule requires an owner's frozen record, and §4 row 201's request contradicts appendix A; the only definition sits in SLEY2_TRIAL_RUNNER_V1.md:309-329, whose Boundary (:21-30) says it alters none of the endpoint contracts - an SMP1 revision 13 defining the record, its owners and its version applicability, reconciling row 201, check_smp1_contract PASS plus owner-lane review, and S20-620 §9 citing it | [P2] [contract-composition] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:146-151,217-220 - S20-300 restricts hit consumers to S20-310 queries and says the hit path has no other readers, but index_cache.rs:256-270 adds a hit-reading probe feeding workspace.open that the profile does not define, although S20-620 §9:314-315 cites it to S20-300 - an S20-300 revision defining the probe and admitting the disclosure reader, checker PASS | [P3] [spec-precision] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:312-314,323-326 - "when and only when" overstated against fail-open absence (server.rs:2908-2916), and the warm-up refusal symbol and materialization overstated against owner-failure precedence (server.rs:2686-2700) and write-back failure - §9 reworded or the stated behavior test-pinned | [P3] [agent-contract] bench/live/tooling.py:155-162 - on the one-request-per-invocation route any query.continue, truncated page or cursor-bearing page rejects the trial (sley2_live_judge.py:3152-3169,3182-3184,3464-3484), yet TOOLING omits the ROOT_BACKED §3:269-272 paging = 1 rule - state paging = 1 with no cursor as the only non-rejecting form, with a pin, or land the multi-invocation continuation decision | [P3] [record] bench/live/SUCCESSION-COVERAGE.md:29; bench/live/MEDIATED-INPUT-ACCOUNTING.md:102-104 - both say TOOLING lacks the root-query layout (false since 8db44154, tooling.py:120-148), and the CONTEXT row cites 3 tests where there are now 7 - correct both records | [P4] [conformance] crates/sley-protocol/src/server.rs:1283 - workspace.open ignores a non-empty body, contrary to S20-620 §9:310 and SMP1 :377-379,647; pre-existing and class-wide, newly reachable by the agent - an empty-body check with a test, or the leniency stated in the SMP1 amendment | [P4] [record] docs/adr/ADR-0036-sley2-trial-runner-boundary.md:3-5; docs/spec/SLEY2_TRIAL_RUNNER_V1.md:273 - ADR status still says revision 2 with five tests, and the revision 5 bullet sits under "Revision 2 clarifications" - refresh the ADR status and neutralize the heading while keeping the checker anchors verbatim
SUMMARY: The trial-runner side of revision 5 conforms: the nineteen names, their frozen order and the terminator literal, the drift and completion-binding gates (both directions, verified in memory), the machine-summary, WORK_PACKAGES and closeout records, and TOOLING layouts that match RESTRICTED §3-4 and ROOT_BACKED §5-6 exactly and reproduce the conformance RootQueryIds. The field-9 change to the workspace.open response body of a COMPLETE SMP1 method is specified only in the S20-620 consumer contract, contradicting SMP1's revision-12 and authority text and S20-620's own Boundary, so the SMP1 row must be amended by the SMP1 owner lane (P1); the S20-300 profile also needs to admit the new hit-path probe (P2). Three P3 precision and record items and two P4 notes remain, and the status must stay S20_620_IMPLEMENTED_REVIEW_PENDING on this lane.
