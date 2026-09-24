# Mediated campaign input accounting — 2026-09-21 (work branch only)

Production path: `bench/live/mediated_attempt.py::execute_mediated_attempt`
(sley_2_0 arm). Production staging
(`stage_mediated_scratch`): documented tooling
(`.sley-live/TOOLING.md`), the generic frame-forwarding shim
(`.sley-live/sley-tool`), and the generic frame transport
(`mediated_transport.py`: Gateway, op envelopes, no task content).
Deterministic test-only adapter: `bench/live/mediated_client.py`
(scripted sequences, witness literals, task construction, synthetic
provider events) — never staged by the production path; tests inject
it at the provider boundary via `extra_scratch_files` /
`stage_test_adapter` AFTER production staging is verified, exercising
the real confinement, mediation, capture, oracle, append, and
verification machinery.

## Removed privileged hint (this pass)

`stage_trial_inputs` (private manifest role map, 6c-prefix parameter
selection, conventional variant-member literals staged as
`scratch/trial_inputs.json`) is deleted from the production path and
from `capture_demo.py`. No `trial_inputs.json` is staged anywhere.
`ADAPTER_MEMBERS`/`MEMBERS` constants are deleted; new typedef member
ids are agent-authored locally (`CLIENT_MEMBERS` in the client — the
judge checks typedef shape and case coverage, never those literals).

## Every agent-visible input and its governing contract

| # | Agent-visible input | Governing contract / frozen identity | Kind | Fairness accounting |
|---|---|---|---|---|
| 1 | Prompt bytes (frozen task JSON + seed) | `bench/live/tooling.py::build_prompt`; corpus `bench/corpus/v1/tasks.json` (frozen; `corpus_digest` + `task_statement_digest` pinned in the run manifest) | Task text | `context_budget`: `model_input_tokens` enforced vs manifest; prompt digest stored (`prompt_sha256`) |
| 2 | `.sley-live/TOOLING.md` (SLEY2 surface) | `bench/live/tooling.py::_files` (frozen `SLEY2_TOOLING`); `tool_description_digests` pinned in the run manifest | Documented training material | Workspace file, identical every trial; reads are agent file IO, not gateway exchanges |
| 3 | Served repository state (base pack content) | `bench/live/taskpacks.py::stage_initial` (frozen `bench/fixtures/sley2/<TASK>/base.pack`); `arm_fixture_digests` in manifest; `pack_sha256` in capture `start.json` | Starting state | Not directly visible (protected_ws masked); visible only via gateway responses below, all counted |
| 4 | Gateway responses (inventory/read/side/open/revision/caps/budgets/raw/propose/append/compose/inspect/validate/finish + mechanical `resolve`; `revision` takes an agent-supplied tx since trial-runner contract revision 5) | `SLEY2_TOOLING` documented surface; `bench/live/mediated_sley.py::ALLOWED_COMMANDS`; every frame inside `TrustedCapture.exchange` | Context through the allowed interface | `action_budget` → `trial_max_exchanges`; `wall_time_budget` → `trial_max_wall_ms`; response caps; cumulative ledger; `tool_calls == completion exchanges` asserted in tests; denied/failed responses counted, never zeroed |
| 5 | `.sley-live/sley-tool` shim + `mediated_transport.py` (generic frame transport) | `bench/live/mediated_attempt.py::stage_mediated_scratch` (shim forwards the documented surface over the socket; transport moves frames only, no task content) + `assert_production_staging_clean` (permitted set + no solver markers) + `production_staging_digest` (every legitimate input digested) | Harness scaffolding, task-agnostic | No task content; identical every trial; no special production branches; test-only `mediated_client.py` (seq_type/seq_stale, CLIENT_MEMBERS, emit_provider_stream, and the route-neutral scripted CONTEXT agent `context_discover_and_repair` with its root-query codec) never staged in production, injected only by tests on the mediated route and imported by the direct witness `succ_witness_context.py` |
| 6 | Confinement env (`HOME=/scratch`, `PATH`/`LANG`, `$SLEY2_GATEWAY_SOCK`) | Explicit mapping in `execute_mediated_attempt` | Harness scaffolding | Not task content; identical every trial |
| 7 | TYPE/CONTEXT/STALE role identities | Discovered via input 4 (`discover_type_roles`, `seq_stale` guard scan: inventory kinds + read bodies, structural criteria only; CONTEXT since 2026-09-23: `open` snapshot binding + bounded class-4/class-14/class-2 `raw` root queries with explicit continuation + reads, `context_discover_and_repair`) | Context through the allowed interface | Discovery exchanges are captured and counted like any agent action (TYPE proof: 13 exchanges; STALE proof: 5) |

No input comes from the private `task_manifest.json` role map in the
production path. The run manifest (`repo_commit`, digests, budgets,
seeds) and capture `start.json` frozen bindings (pack, manifest,
tool version, binary) bind the exact starting state before any agent
interaction.

## Proven through the integrated path (real oracle + verify)

- TYPE (`type_pos`, discovery, no staged inputs): confined migration
  reconciled (13 exchanges), judge ACCEPTED, adjudicated accepted —
  `capture_demo.py type_pos` PASS; `test_mediated_attempt.py::
  test_full_type_migration_real_oracle` (execute_attempt →
  run_fixture_oracle → verify_attempts → VERIFIED_LIVE_EVIDENCE) PASS.
- STALE (`stale_pos`, guard discovered, no staged guard id): 5
  exchanges reconciled, judge ACCEPTED —
  `test_full_stale_flip_real_oracle` PASS (same integrated path).
- All 12 pre-existing campaign-path failure-boundary regressions
  retained and green (ingress invalid/oversized, oracle-reject
  precedence, scratch forgery, missing final, timeout, process
  failure, cumulative budgets, denied-command counting, access
  restrictions).

These runs are deterministic stand-ins, never counted toward the
preregistered live-model campaign. `ga_claimed=false`.

## Explicitly not established here

- CONTEXT mediated proof ESTABLISHED: the judge takes a trusted
  runner-controlled reference to the protected, reconciled capture
  (`SLEY2_MEDIATED_CAPTURE_DIR`, set by the runner around the oracle
  call and forwarded by `run_fixture_oracle`; never agent-settable).
  `_audit_mediated_access` verifies capture integrity (hash chain
  from genesis, req/resp seq pairing, per-record attempt match),
  input bindings (frozen pack/manifest/tool/binary vs
  judge-computed), and completion bindings (exchange count, chain
  head, final_sha256 vs the trial final artifact), then derives
  access/budget evidence from actual captured requests/responses
  (`raw:<method>` exposes inner query methods; true session scopes;
  same bounded/continuation/budget/whole-store/commit rules as the
  legacy audit). The obsolete chain file is neither depended on nor
  fabricated on the mediated path. Proven: `test_mediated_context.py`
  pos ACCEPTED end to end (execute_attempt → real oracle → verify),
  incomplete-impact and inconsistent-continuation distinguishing
  proofs, plus 13 audit unit tests (missing/tampered evidence,
  binding mismatches, budgets).
- CONTEXT task-spec gate RETAINED: established that the corpus
  ("add a required record field", no identity/type) does not specify
  the F1/Bool manifest literal, and that no permitted bounded route
  could enumerate a typedef's users (inventory is whole-store; reads
  need ids; server queries needed an unmintable snapshot). Concrete
  patch on the work branch: de-literalized judge (any added member
  via pre-image diff + structural closure discovery) with re-emitted
  manifest. No private repair/impact set is injected anywhere in the
  acceptance path. 2026-09-23 (REQ-10, trial-runner contract revision
  5, `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`): the
  discovery route is implemented on the work branch — `workspace.open`
  is afforded and discloses the accepted head's materialized snapshot
  identity (field 9), so bounded class-4/class-14 root queries with
  explicit continuation are formable from allowed routes; the
  mediated stand-in discovers the typedef and its impact closure with
  no identity argument (`test_mediated_context.py`, 7 integrated
  proofs at the time; 8 after the revision 5 repairs). TOOLING.md
  documents the root-query request/response layouts, continuation, and
  budgets (commit `8db44154`, pinned by `RootQueryContractTests`);
  continuation is audited by query and cursor, not by invocation (revision
  5 repair), so the one-shot `sley-tool` route can page. NOT done: the
  corpus is frozen and digest-pinned, so no task-input amendment naming
  the field was made (the stand-in selects the store's unique record
  typedef by bounded listing; ratification belongs to the corpus owner);
  live-model usability is documented, not demonstrated (no live-model
  trial); the revision 5 re-review after the REVISE round is pending.
- CREATE complete-task rework DONE (typed Money/LineItem records,
  checked subtotal + merged-tax helpers, chained entry returning
  Result<Money,ArithmeticError>, no precomputed intermediates):
  genuine 60-op program authored/committed/executed through the trial
  surface (empty/one-line/overflow exact); submitted tests authored,
  validated (phase-12 ceilings), committed, and executed natively;
  full judge ACCEPTS (`trial_create_pos.log` two-round proof; 7
  witness variants discriminate with exact codes; 23 judge-unit +
  10 driver-unit regressions green). Structural findings retained:
  (a) 64-op trial-surface record cap (`sley2_tool._assemble`);
  (b) trial workspaces never advance between judge runs (judge copies
  fresh, discards scratch), so program + tests reach committed state
  in two harness commits (round-1 program, round-2 tests); (c) a
  single candidate carrying tests targeting new functions is refused
  at commit (TXN_TEST_EVIDENCE_UNSUPPORTED: validation auto-selects
  tests targeting affected functions) — single-trial model
  acceptance of CREATE-style code+tests needs that co-commit review
  gate; witness round-1 harness commit is explicitly logged, not
  agent bytes.
- MODULE/MERGE/CORRUPT/PERF semantic dispositions — recorded in
  `SUCCESSION-COVERAGE.md`; narrowed per the closure pass, no silent
  closure.
