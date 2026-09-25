<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 5b538f36eaee0574027539e512760a05e6d3ae29; role: vulcan; field: vulcan_surface_review_revision_5; dispatched: 2026-09-23T06:38:23Z; duration_s: 694; process_exit_code: 0 -->
# Vulcan Council review — sley2_trial_runner
Harness: claude-code
Reviewed checkpoint: 5b538f36eaee0574027539e512760a05e6d3ae29

What I verified myself, independent of what the records claim:

- **Scope.** `git rev-parse HEAD` returned `5b538f36eaee0574027539e512760a05e6d3ae29`, which matches the scope. `git log --oneline ab42a3a9..HEAD` shows 7 commits (c2e37384, 44f18e2b, 754a140d, 8db44154, e0146f8d, 20b423f3, 5b538f36). `git diff --stat ab42a3a9..HEAD` shows 37 files, +2472/−362.
- **Diffs read.** I read `git diff ab42a3a9..HEAD` for: the judge, `sley2_tool.py`, `mediated_sley.py`, `mediated_shim.py`, `mediated_attempt.py`, `runner.py`, `tooling.py`, `server.rs`, `index_cache.rs`, `server_tests.rs`, `test_sley2_tool.py`, `test_mediated_gateway.py`, `test_mediated_access.py`, `test_agent_access.py`, the spec, the checker, and `succ_witness_context.py`. I also ran `git show HEAD:bench/live/tests/test_mediated_context.py`, `git show ab42a3a9:bench/fixtures/sley2_live_judge.py` (loaded in memory as the base judge), `git show --stat 5b538f36 20b423f3`, and `git diff 20b423f3..5b538f36` (T54 counters only).
- **Checkers.**
  - `python3 scripts/check_sley2_trial_runner.py`: exit 0, `"result": "PASS"`, revision 5, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, `problems []`.
  - `python3 scripts/check_supply_chain_audit.py`: `"result": "DEFERRED"`, `t54_high_confidence_scan: PASS`, `t52_local_lock_inventory: PASS`. The exit code is 0 but was observed through a grep pipe.
- **Tests I ran.**
  - `cargo test --locked --offline -p sley-repo --lib index_cache`: exit 0, 9 passed, including `probe_reports_only_a_materialized_snapshot_and_never_builds`.
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: exit 0, 1 passed (`workspace_open_discloses_only_the_materialized_head_snapshot`).
  - `python3 -m unittest discover -s bench/sley2/tests -t .`: Ran 23, OK.
  - `python3 -m unittest -v bench.live.tests.test_tooling bench.live.tests.test_sley2_tool.Sley2ToolSurfacePinTests`: Ran 14, OK. This covers the 5 `RootQueryContractTests`, the 4 `Sley2CommandSurfacePinTests` and both surface pins.
- **Not run.** The binary-gated suites (`test_mediated_context` with its 7 integrated proofs, `test_mediated_access`, `test_agent_access`, `Sley2ToolTests`, `test_mediated_gateway`) did not run. `SLEY2_SLEY_BINARY` is unset and the build target directory is outside this session's allowed paths. For their pass status I rely on gate record §3/§10.3 ("Ran 219 tests … OK" at e0146f8d); I did not reproduce it.
- **In-memory probes (executed).**
  - I ran 19 hand-built captures through `judge._audit_mediated_access`, using a `CaptureBuilder` subclass and a stubbed `_resolve_binary`.
  - I ran the base judge (ab42a3a9) against the HEAD judge on three continuation cases.
  - I sent four frames through a real `MediatedSleyEndpoint` + `TrustedCapture` to observe the capture labels.
- **Files read, with line ranges.**
  - Gate record `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:1-580` (the remainder is the Appendix A token list).
  - Bench Python:
    - `bench/fixtures/sley2_live_judge.py:40-54,2960-3519`
    - `bench/live/mediated_sley.py:1-330`
    - `bench/live/mediated_shim.py:1-106`
    - `bench/live/mediated_transport.py:1-120`
    - `bench/live/trusted_capture.py:290-484`
    - `bench/live/sley2_tool.py:60-119,230-429,735-784,1080-1119`
    - `bench/live/mediated_client.py:90-129,250-649`
    - `bench/live/mediated_attempt.py:105-214`
    - `bench/live/tests/test_mediated_access.py:14-153`
  - Rust:
    - `crates/sley-protocol/src/server.rs:990-1089,1096-1299,2443-2467,2887-2916`
    - `crates/sley-repo/src/index_cache.rs:100-290`
    - `crates/sley-query/src/root_query.rs:1280-1360,1494-1512`
    - `crates/sley-txn/src/maintenance.rs:50-172`
  - Spec: `docs/spec/SMP1.md:300-324,630-659`.
  - Witness logs: `bench/live/succ-trials-20260923/trial_context_{pos,neg}.log`.

## Evidence checked

1. **The 19-name surface grants only read-only discovery. Verified.**
   - **Only one method was added.** `TOOL_METHODS == ARM_AFFORDANCES` in order (pin test passes). The only delta is `workspace.open`, appended last (`runner.py:114`, `sley2_tool.py:113`) and removed from `ARM_DENIED_METHODS`. Everything else stays denied: commit, merge, exchange.*, session.*, workspace.create, gc and execute.
   - **The raw path is guarded twice**, at `sley2_tool.py:773` and at `Session._request` (`:337-339`).
   - **The server handler cannot mutate state.** `workspace_open` (`server.rs:2899-2904`) is `&self`: it loads the head, runs the metadata probe, and returns `counted(summary, 1)`.
   - **The probe never builds or writes back.** `index_cache.rs:256-270` does `read_record` + `accept_cached` only. The unit test shows cold → `None` with no file written, a hit that survives object-store removal, and a discarded record → `None` with no rewrite.
   - **`revision <tx>` is not a widening.** It is the same capability as the already-allowed `raw revision.read <tx>`.
   - **Field 9 is only an identifier.** It discloses a snapshot identity derived from public head state.
   - **Witness logs:** pos discovered 10 of 10 impact entities in 1 page and was judged exit 0; neg was refused at validation phase 6 (tag 9).
2. **Judge access-audit change. Record claims confirmed; one hole remains (the P2).**
   - I executed base vs HEAD on the same captures:

     | Case | Base (ab42a3a9) | HEAD |
     |---|---|---|
     | 3-page chain | REJECTED "inconsistent continuation" | ACCEPTED |
     | Stop on a truncated continue page | ACCEPTED | REJECTED "truncated page without continuation" |
     | Truncated root → refused continue | ACCEPTED | ACCEPTED |

   - **Refused continues still close a chain.** Also accepted at HEAD: root(trunc) → cont(trunc) → cont(failed).
   - **Continues are not tied to the query they continue.** Two truncated roots → one untruncated continue is accepted.
   - **Past-the-end cursors are served.** `page_items` (`root_query.rs:1336-1345,1494-1511`) accepts any entity cursor and filters `> key`, so `after = ff…ff` returns an empty, untruncated, *successful* page. The `extra_continue` stand-in already shows the server answering a continue positioned at the last entity.
   - **Rules that hold:**
     - A cross-scope continue is rejected as inconsistent.
     - `omitted>0` with no truncation and no follow-up is rejected.
3. **Budgets. The bounds themselves hold (units issue is P3).**
   - **Enforced before release.** The capture refuses any envelope over 1048576 bytes *after durable recording and before release* (`trusted_capture.py:460-469`). `gateway_loop` ends the attempt on `CaptureError` (`mediated_sley.py:290-295`), so over-cap bytes never reach the agent.
   - **Judge boundaries are exact:**

     | Probe | Result |
     |---|---|
     | 1 MiB | accepted |
     | 1 MiB + 1 | rejected "response over bound 1048577" |
     | 4 × 1 MiB | accepted |
     | 4 × 1 MiB + 1 byte | rejected "cumulative 4194305 over budget" |

   - **Two numbers, same outcome.** The capture's trial cap is 8 MiB (`:325`) while the judge and TOOLING say 4 MiB. The judge is authoritative and rejects, so the outcome is fail-closed.
4. **The five negative classes are asserted by status + code + detail substring** in `test_mediated_context.py` (these assertions were not re-run here; see Not run):
   - `stop_early` → "truncated page without continuation"
   - `incomplete` → `CAPTURE_GATE_NO_FINAL`
   - `extra_continue` → "inconsistent continuation"
   - `overbudget` → "over budget"
   - lost exchanges → "mediated exchanges missing" + `CAPTURE_*` `harness_failure`
   - Plus: the no-argument `revision` is refused and counted.

   The incomplete-discovery class can be bypassed by one extra exchange (see the P2 finding).
5. **Capture labels.**
   - `raw:<allowlisted>` / `raw:denied` / `open` / `revision` are recorded as designed (`mediated_sley.py:226-231`).
   - `open`, `raw:workspace.open` and `revision` are non-paging, so any omitted or truncated flag on them is rejected as hidden truncation (probes G, H, I).
   - **A denied command is captured under the agent's own string.** A frame with command `raw:query.continue` produced `captured method='raw:query.continue' failed=True`, and the judge's `inner_method` (`sley2_live_judge.py:3425-3430`) reads that as a real continue.
6. **Open decision (one-shot `sley-tool` continuation). Ruling: acceptable as documented for revision 5; cross-call continuation need not be supported now.**
   - **It fails closed.** A continue in another scope is rejected, and an abandoned truncated page is rejected. So the limitation can only cause a false rejection, never a false acceptance.
   - **The documented workaround is feasible.** `max_entities` goes up to 65535, and the fixture closure is 10 entities; the direct witness used a 16-entity page.
   - **Condition 1:** do not add a stable per-agent scope to `sley-tool` until the continuation discharge is chain-bound (the P2 finding). The audit scope is a client-chosen label (`shim-<pid>`, or any `Gateway(session_id=…)`), so a stable scope alone would only widen the unbound discharge across invocations.
   - **Condition 2:** fix the documentation mismatch (P3), because the staged transport already allows cross-call paging.

## Findings

[P2] [judge-continuation] bench/fixtures/sley2_live_judge.py:3462-3484 (same rule at :3151-3184) - Continuation discharge is unconditional: any exchange labelled `query.continue` in the scope sets `pending_truncated[scope] = trunc` and so closes every pending truncated page. This holds whether the continue was refused or malformed (a failure frame has no bounds, so trunc=False), carried a forged label (next finding), used an after-cursor past the end or skipping pages (a *successful* untruncated page), or continued a different query. Executed probes at HEAD: truncated root → refused continue ACCEPTED; root → trunc cont → refused cont ACCEPTED; two truncated roots → one continue ACCEPTED; the refused case is also ACCEPTED on the base judge. The incomplete-discovery negative class (`stop_early`) is therefore defeated by one extra exchange. Only the naive stop is closed by this revision, and the accepted detail's `continuations=` count includes refused or forged continues. The cursor-unbound part is a limitation documented in the judge (`:3022-3027`); the refusal and forged-label parts are not. The direct audit has the same code but cannot reach this (one command per invocation) - closure evidence: only non-failed, server-answered continue pages advance or close a chain. Each continue must be bound to the truncated page it continues, either by same query parameters with `after` == that page's next cursor, or by the capture carrying per-page returned-entity counts with the judge requiring Σ returned over the chain == first-page returned + omitted. Needed regressions in both audits and one integrated mode: truncated root followed by (a) a refused continue, (b) a forged `raw:query.continue` label, and (c) an `ff…ff` cursor must each reject.

[P3] [capture-labels] bench/live/mediated_sley.py:226-231,300-310 - Denied or unknown commands are captured with the agent-supplied command string verbatim (the else branch and the GatewayError path), so the label set is open, contrary to the revision-5 vocabulary note at :220-225. A frame whose command is `raw:query.continue` or `raw:query.root` is never dispatched, yet it is recorded under that label and counted by the judge as a bounded read or continuation (executed probe). - closure evidence: record denied commands under a single fixed label, have the judge reject any label outside the closed set (`raw:<TOOL_METHODS>`, `raw:denied`, the documented commands, `resolve`), and add a regression test.

[P3] [surface-contract] bench/live/tooling.py:158-162 (with bench/live/mediated_attempt.py:113-117,147-149; bench/live/mediated_transport.py:73-101) - TOOLING says each `sley-tool` invocation is its own scope and tells the model to size pages to complete. But production staging also ships `mediated_transport.py` ("the shared frame transport any in-sandbox helper may use"), whose `Gateway` takes a caller-chosen `session_id`. That undocumented route is exactly how the integrated positive proof performs three continuations. The surface TOOLING documents and the surface the proof uses differ, and the audit scope is a client label, not a boundary. Separately, the one-shot limitation is not pinned by any test (a cross-scope continue is rejected in my probe, but no regression test covers it). - closure evidence: either document the staged transport and its session scoping in TOOLING, or state that paged discovery is only available through it; pin the text by test; add a regression that a truncated page in scope A followed by a continue in scope B rejects.

[P3] [budget-contract] bench/live/tooling.py:132,163-165 (with bench/live/trusted_capture.py:324-327,462-469; bench/live/sley2_tool.py:773-775; bench/live/mediated_client.py:276-277) - TOOLING documents `max_response_bytes` up to 67108864 and says "no single reply may exceed 1048576 bytes". It does not say that this bound applies to the JSON envelope, in which `raw` returns the body as hex (about 2× the server body). A model that reads the budget as server-body bytes (as the stand-in does, requesting 1048576) can produce a ~2 MiB envelope. The capture then records it, refuses it (`CAPTURE_RESPONSE_OVER_CAP`) and ends the attempt. This fails closed, but the documented limit and the enforced budget are in different units. The 8 MiB capture trial cap vs the 4 MiB documented and judged budget is a second unexplained pair of numbers. - closure evidence: state the per-reply bound in envelope bytes together with the hex factor and a safe `max_response_bytes`; make the stand-in request a value that fits; add a test that a maximal page at the documented setting fits the per-reply cap.

[P4] [strict-input] crates/sley-protocol/src/server.rs:1283,2899-2904 - `workspace.open` ignores its request body, although SMP1.md:647 specifies "empty" and spec section 9 says "takes no body". `raw workspace.open <any hex>` is now agent-reachable and is answered OK instead of refused. This grants no authority (the body is unread and capture keeps its digest), and it is the same pattern as `session.capabilities` and `session.budgets`. - closure evidence: refuse non-empty bodies with `PAYLOAD_INVALID` plus a server test, or explicitly record that the body is accepted and ignored.

[P4] [resource-claims] crates/sley-protocol/src/server.rs:2906-2916,2443-2447 (with crates/sley-repo/src/index_cache.rs:256-270) - The field-9 probe goes through `maintenance()`, which initializes the maintenance lock boundary if absent and takes a *blocking* shared lock. On a hit it also decodes the whole cached record (up to `MAX_SNAPSHOT_RECORD_BYTES` = 64 MiB) while still being charged as a one-entity read. The spec's "mutates nothing" and the description "metadata-only … fail-open" are looser than this. This is harmless in trials: the boundary already exists after seeding, the agent has no exclusive-lock route, and the 512-exchange cap applies. - closure evidence: use `acquire_shared_repository_maintenance_nonblocking` (absence on contention) or state the blocking behaviour, and state the probe's cost class accurately in spec section 9.

## Assessment

The widened surface is sound. `workspace.open` is the only addition. It can only answer head facts plus the materialized snapshot identity, never builds or writes the cache, and its capture labels make any omitted or truncated signal a rejection. The no-argument `revision` fallback to the harness head is gone and refused before any server contact. The budgets hold at their exact boundaries: the capture enforces the per-reply bound before release and the judge enforces the cumulative bound.

The judge change does what the gate record says. It removes the false rejection of chains longer than two pages and closes the "stop on a truncated continue page" case, and I reproduced both deltas against the base judge.

However, continuation discharge is still a per-scope flag cleared by any `query.continue`-labelled exchange. A refused continue, a forged label, a past-the-end cursor, or a continue of another query each close a truncated chain. The "incomplete discovery rejects" negative class — the property REQ-10's paged discovery exists to evidence — can therefore be bypassed with one extra exchange. The oracle still gates repair correctness, so this is a P2 evidence-integrity defect, not a safety break. It needs closing before this lane can PASS.

On the open decision: the one-shot `sley-tool` limitation is acceptable as documented and fails closed. Cross-call continuation should not be added until continuation is chain-bound. The TOOLING text should be reconciled with the staged transport the positive proof actually uses. No GA, release-readiness or model-trial claim is made or implied.

VERDICT: REVISE_0_P0_0_P1_1_P2_3_P3_2_P4
SECTION: sley2_trial_runner
FIELD: vulcan_surface_review_revision_5
SCOPE_SHA: 5b538f36eaee0574027539e512760a05e6d3ae29
FINDINGS: [P2] [judge-continuation] bench/fixtures/sley2_live_judge.py:3462-3484 (same rule at :3151-3184) - any query.continue-labelled exchange (refused/malformed, forged label, past-the-end or page-skipping cursor, or another query's continue) sets pending_truncated=trunc and closes every pending truncated page in scope; executed probes: truncated root → refused continue ACCEPTED at HEAD and base, two truncated roots → one continue ACCEPTED; the stop_early incomplete-discovery negative class is bypassable with one extra exchange - closure evidence: only non-failed server-answered continues advance or close a chain, each bound to its truncated page (same query and after == next cursor, or Σ returned entities over the chain == first-page returned + omitted), plus regressions (refused, forged-label, ff…ff cursor) in both audits and one integrated mode; [P3] [capture-labels] bench/live/mediated_sley.py:226-231,300-310 - denied or unknown commands are captured under the agent-supplied command string, so "raw:query.continue" is recorded and counted by the judge as a continuation without dispatch (executed probe) - closure evidence: fixed label for denied commands, judge rejects labels outside the closed set, regression test; [P3] [surface-contract] bench/live/tooling.py:158-162 (with mediated_attempt.py:113-117,147-149; mediated_transport.py:73-101) - TOOLING says each sley-tool invocation is its own scope, but the staged production transport lets any client pick a session_id and is the route the positive proof pages through; the cross-scope rejection is not pinned by test - closure evidence: document the staged transport scope (or state that paging requires it), pin the text, add a cross-scope-continue rejection regression; [P3] [budget-contract] bench/live/tooling.py:132,163-165 (with trusted_capture.py:324-327,462-469; sley2_tool.py:773-775; mediated_client.py:276-277) - the per-reply 1 MiB bound applies to the hex JSON envelope (~2× the body) while TOOLING allows max_response_bytes up to 64 MiB and the stand-in requests 1 MiB, so a literal reading kills the attempt with CAPTURE_RESPONSE_OVER_CAP; the 8 MiB capture cap vs the 4 MiB documented budget is also unexplained - closure evidence: document the bound in envelope bytes with a safe max_response_bytes, fix the stand-in value, test that a documented maximal page fits; [P4] [strict-input] crates/sley-protocol/src/server.rs:1283,2899-2904 - workspace.open ignores a non-empty body although SMP1.md:647 and spec section 9 say empty/no body, and it is now agent-reachable via raw - closure evidence: PAYLOAD_INVALID refusal plus a test, or explicitly record acceptance; [P4] [resource-claims] crates/sley-protocol/src/server.rs:2906-2916,2443-2447 (with index_cache.rs:256-270) - the field-9 probe initializes the lock boundary if absent, blocks on a shared lock and decodes the full cached record (≤64 MiB) while charged as one entity, looser than "mutates nothing / metadata-only / fail-open" - closure evidence: nonblocking shared acquisition or documented blocking, plus an accurate cost statement in spec section 9
SUMMARY: The revision-5 surface widening (workspace.open, field 9, revision <tx>) grants only bounded read-only discovery, the budgets hold at their exact boundaries, and the judge change fixes both defects the gate record names. Continuation discharge is still unconditional: a refused continue, a forged label, a past-the-end cursor or another query's continue closes a truncated chain, so the incomplete-discovery negative class can be bypassed (P2). On the open decision, the one-shot sley-tool limitation is acceptable as documented because it fails closed, but cross-call continuation must wait for chain-bound auditing and TOOLING must be reconciled with the staged transport. Verdict REVISE.
