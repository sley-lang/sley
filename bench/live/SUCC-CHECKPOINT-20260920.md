# Succession sley_2_0 live-arm checkpoint — 2026-09-20

Branch: `work/succession-sley20-arm` on top of `acbc65f0`
(base: batch-14 fold/order repairs; main stays at `8966da2e`, untouched).

## Scope of this checkpoint

- Live-arm tool boundary: `bench/live/sley2_tool.py`, `bench/live/sley2_codecs.py`.
- Live judge: `bench/fixtures/sley2_live_judge.py` (per-task `live_oracle.py`
  entries under `bench/fixtures/sley2/S2B-*/`).
- Frozen base packs + manifests: `bench/fixtures/sley2/S2B-*/base.pack`,
  `task_manifest.json` (plus MERGE `ours.pack`/`theirs.pack`).
- Emitters/drivers: `crates/sley-repo/tests/succ_live_emit.rs`,
  `succ_live_judge_cases.rs` (+ debug-only `succ_debug_commit.rs`,
  to be deleted after DEAD diagnosis).
- Boundary wiring: `bench/live/{oracle,taskpacks,tooling}.py` +
  tests (`test_taskpacks.py`, `test_tooling.py`,
  `bench/live/tests/test_sley2_{e2e,tool}.py`).
- Raw scripted-trial logs: `bench/live/succ-trials-20260920/`.

## Scripted-trial state (current subset, 12 attempted)

Accepted (10): REPAIR, CORRUPT (via `test_sley2_e2e.py`), SIG, TEST,
STALE, MODULE, MERGE, PERF, ADVERSARY, CONTEXT.

Blocked (2):

- DEAD positive: rejected `ORACLE_COMMIT_REJECTED`
  (`CANDIDATE_VALIDATION_UNRESOLVED_REFERENCE`; single-record deletion of
  the private helper is not admissible through the current tool).
- TYPE positive: not yet scripted (needs multi-create composition the
  single-record tool cannot express).

## Negatives observed (raw logs in `succ-trials-20260920/`)

- MERGE overlap → `ORACLE_MERGE_CONFLICT`.
- MODULE stale → `ORACLE_STALE_IMPORT`.
- TEST impl-touch, SIG missing-caller → `ORACLE_COMMIT_REJECTED`
  (server control-flow refusal; fault-specific codes `ORACLE_IMPL_TOUCHED` /
  frozen S3 families remain covered by the static suites, not these logs).

## Explicitly NOT claimed

- Agent-side bounded-read acceptance is not evidenced (judge self-inspection
  only; no tool-boundary transcript yet).
- No live-model succession result follows from scripted acceptance.
- The succession arm and judge coverage are NOT complete: CREATE, EFFECT,
  CAP have no scripted positive; TYPE/DEAD positives are blocked.
- Packs changed in this slice (MODULE exports vs binding, PERF redundant-op
  deletion, CONTEXT closure shape) have NOT had targeted
  semantic-equivalence checks against the original tasks yet.

## Next (authorized, in order)

1. `candidate.append` bounded adapter + budgets; TYPE positive; multi-phase
   CONTEXT (keep TYPE negatives/structural checks).
2. DEAD via composition if contract-valid; else minimal production repair
   under the required design/review sequence (separate from this checkpoint).
3. Tool-boundary read transcript + confinement tests; no zero-default
   `whole_store_reads`.
4. Targeted pack-equivalence checks; 15-task coverage matrix; regressions;
   `make lint` / `make quick`; further checkpoints.

Holds preserved: `ga_claimed=false`; AR-02 / R2 / C1 / lab / operator-release
holds unchanged; no mint, release, main fast-forward, or batch-14 action here.
