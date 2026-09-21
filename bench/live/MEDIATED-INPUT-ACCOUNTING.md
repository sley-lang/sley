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
| 4 | Gateway responses (inventory/read/side/revision/caps/budgets/raw/propose/append/compose/inspect/validate/finish + mechanical `resolve`) | `SLEY2_TOOLING` documented surface; `bench/live/mediated_sley.py::ALLOWED_COMMANDS`; every frame inside `TrustedCapture.exchange` | Context through the allowed interface | `action_budget` → `trial_max_exchanges`; `wall_time_budget` → `trial_max_wall_ms`; response caps; cumulative ledger; `tool_calls == completion exchanges` asserted in tests; denied/failed responses counted, never zeroed |
| 5 | `.sley-live/sley-tool` shim + `mediated_transport.py` (generic frame transport) | `bench/live/mediated_attempt.py::stage_mediated_scratch` (shim forwards the documented surface over the socket; transport moves frames only, no task content) + `assert_production_staging_clean` (permitted set + no solver markers) + `production_staging_digest` (every legitimate input digested) | Harness scaffolding, task-agnostic | No task content; identical every trial; no special production branches; test-only `mediated_client.py` (seq_type/seq_stale, CLIENT_MEMBERS, emit_provider_stream) never staged in production, injected only by tests |
| 6 | Confinement env (`HOME=/scratch`, `PATH`/`LANG`, `$SLEY2_GATEWAY_SOCK`) | Explicit mapping in `execute_mediated_attempt` | Harness scaffolding | Not task content; identical every trial |
| 7 | TYPE/CONTEXT/STALE role identities | Discovered via input 4 (`discover_type_roles`, `seq_stale` guard scan: inventory kinds + read bodies, structural criteria only) | Context through the allowed interface | Discovery exchanges are captured and counted like any agent action (TYPE proof: 13 exchanges; STALE proof: 5) |

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

- CONTEXT mediated proof: the judge's `_audit_agent_access` reads the
  legacy workspace chain file (`trial_ws/.sley-live-transcript.jsonl`),
  which the mediated path no longer produces (gateway uses
  `dispatch()`, not `main()`, so no chain file is written). The judge
  must consume authoritative mediated access evidence (capture
  exchanges) instead. Separately, the required member (`f1`, Bool) and
  impact closure roles live only in the private manifest — neither in
  the corpus prompt nor discoverable from the store — so a
  discovery-only agent cannot know them; that needs a contracted
  capsule/prompt decision. Next actions: (a) extend the judge with a
  mediated-evidence route (e.g. `SLEY2_MEDIATED_CAPTURE_DIR` audit of
  `exchanges.jsonl`, or runner-materialized chain from reconciled
  capture before oracle); (b) adopt the member/impact contract source
  and count it; (c) add a `context_pos` discovery sequence + real-oracle
  test. No legacy-transcript fabrication in the meantime.
- CREATE complete-task rework (typed LineItem/tax entry execution, no
  precomputed intermediates) — see the task-2 handoff note.
- MODULE/MERGE/CORRUPT/PERF semantic dispositions — recorded in
  `SUCCESSION-COVERAGE.md`; narrowed per the closure pass, no silent
  closure.
