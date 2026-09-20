# Succession sley_2_0 live-arm coverage — 15 frozen corpus tasks

Status as of the current slice (wt-succ branch). "Scripted" = deterministic
script driving only the frozen agent tool surface (no model). No live-model
succession result follows from scripted acceptance. `ga_claimed=false`.

Evidence logs: `bench/live/succ-trials-20260920/` (this slice) plus the
frozen S3 suites (`crates/sley-repo/tests/s3_*`, `crates/sley-vm/tests/s3_g3_perf`).

| Task | Positive (live scripted) | Negative (live scripted) | Access/resource | Remaining |
|---|---|---|---|---|
| CREATE | none (blank-repo start; missing manifest → harness_error by design) | — (static S3 s3_g1_create) | n/a | live-model trial |
| REPAIR | accepted (e2e, clamp fix) | static S3 families | n/a | live-model trial |
| SIG | accepted (caller + CallDirect fix) | missing_caller → tool-level refusal (valid False); static S3 | n/a | live-model trial |
| MODULE | accepted (exports fix; 6 refs resolve; observation held) | stale_import → ORACLE_STALE_IMPORT | n/a | live-model trial |
| TYPE | accepted (3-phase compose: 2 block creates + switch/status migration) | missing_case → ORACLE_MISSING_CASE; bool_compat → ORACLE_BOOL_COMPAT_FIELD | n/a | live-model trial |
| EFFECT | none (statically pinned refusal) | VM_LOWER_OPCODE_UNSUPPORTED (S3 pin s3_g3_effect_refusal_pin) | n/a | live trial unscripted; live-model trial |
| CAP | none (statically pinned refusal) | VM_LOWER_OPCODE_UNSUPPORTED (S3 pin s3_g3_cap_refusal_pin) | n/a | live trial unscripted; live-model trial |
| DEAD | BLOCKED — correct deletion inadmissible (see below) | reachable_changed → ORACLE_REACHABLE_CHANGED | n/a | review-gated production repair |
| TEST | accepted (3 TestCase creates) | impl_touch → tool-level refusal; case_missing → ORACLE_CASE_MISSING | n/a | live-model trial |
| STALE | accepted (guard flip) | static S3 (guard_disabled) | n/a | live-model trial |
| MERGE | accepted (semantic union) | overlap → ORACLE_MERGE_CONFLICT | n/a | live-model trial |
| PERF | accepted (scan→ordered-map; 7→4 instr, 42.9% ≥ 30%; outputs identical) | faster_but_wrong → ORACLE_OUTPUT_MISMATCH (valid proposal, flipped probe changes digest) | n/a | live-model trial |
| CONTEXT | accepted (typedef F1 + 3-const closure, 3-phase compose) + access evidence whole_store_reads=0, 4 targeted reads, responses ≤ 9222 B | unbounded_read → QUERY_REQUIRED_FACT_OMITTED (live inventory test + static S3) | chained tool-boundary transcript verified; per-response cap 1048576 enforced | live-model trial |
| ADVERSARY | accepted (opcode fix; label ignored) | wrong_repair: operand-swap dual is a correct fix (accepted, correctly); static S3 family | n/a | live-model trial |
| CORRUPT | accepted (e2e restore) | static S3 (unflipped) | n/a | live-model trial |

## DEAD: why the positive is blocked (no benchmark exception)

Corpus demands: remove unreachable block 90 + unused private helper 8e
(+ cascade 8f/91/93), preserve reachable behavior, public identities,
tests, effect constraints.

Established (trial_dead2 + code trace):

- Trial-content bug fixed first: the first attempt orphaned op 93
  (block 90's operation) → CANDIDATE_VALIDATION_UNRESOLVED_REFERENCE.
  With 93 deleted, reference integrity passes.
- Next owner: test-plan selection —
  CANDIDATE_VALIDATION_TEST_PLAN_ERROR / TEST_PLAN_SELECTION_INVALID
  (24_016). Mechanism: `affected_functions` = base ∪ proposed kind-5
  entities always contains the deleted helper 8e, while the selection
  index is built from proposed-only units (`contracts.rs` select_tests
  → function_index → UnresolvedEntity). Every function deletion,
  however legitimate, fails this lookup deterministically.
- Frozen S3 (`s3_g1_dead.rs`) never validates a real DeleteEntityBinding
  candidate: its "passing deletion" validates a namespace CREATE while
  the removal lives in Rust-side model structs. No frozen vector
  demonstrates an admissible function deletion, and none pins the
  rejection as expected either.
- `candidate.append` cannot help either (proven separately): both sides
  must number ordinals from zero, so any concatenation fails closed
  with MUTATION_CANDIDATE_OPERATION_ORDINAL (tested).
- No contract-conforming formulation exists in the op set: the helper
  cannot stay (judge absent-check), cannot be orphaned (inventory), and
  deletion always trips selection. Nor may the task be weakened
  (removed_private_functions stays 1; no test deletion; no
  DEAD-specific production exception).

Verdict: admissible only via an explicit, review-gated semantic change
(e.g. tombstone-aware test-plan selection that keeps deleted functions
indexable as removed while selecting tests for survivors). The required
review provider is unavailable; DEAD stays explicitly blocked. The
debug helper (`succ_debug_commit.rs`) is retained until that diagnosis
is consumed.

## Pack-equivalence notes (this slice)

- MODULE: namespace move embodied as integrity-package export grant
  (DependencyBinding with external roots is unvalidatable in packs —
  frozen engine constraint). Preserved: function identity untouched, 6
  CallDirect callers resolve identically (reference_count + observation
  + duplicate-impl checks green). Residual gap (no literal namespace
  re-homing) is stated, not hidden.
- PERF: repacked from redundant-op deletion to the corpus
  transformation (nested SInt scan → ordered-map build + probe) at
  trial scale (2×2). Measured on fixed inputs: 7→4 instructions
  (42.9%), outputs identical, no effects, trivial memory. S3 G3 pins
  the full 40×40 shape, reduction bar, output digests, and the
  faster_but_wrong → ORACLE_OUTPUT_MISMATCH negative.
- CONTEXT: 10,011-entity store (typedef + 3 record consts + 3 globals +
  10,000 filler bool consts). Filler never names the typedef (pack scan:
  15 typedef-id occurrences, all in typedef/consts/globals/package/
  namespace); globals need no F1 update (they name the typedef, whose
  form changes uniformly). Impact closure (typedef + 3 consts) is
  completely updated; judge checks all three roles. The full 10,001-
  closure machinery stays pinned in static S3; the live trial covers
  agent-side bounded behavior with whole_store_reads=0 derived from
  chained evidence.
- TYPE: 4 Required reachable blocks via entry chaining through the
  immutable Bool dispatch (block 6d immutable ⇒ Bool dispatch
  preserved; full JobState retyping would need 6d in targets — a task
  change, not agent cleverness). Status genuinely migrated off Bool.

## Tool-trust notes (this slice)

- `valid` now reads the validation decision tag (1 == Valid) from the
  result object; delivery alone never implies acceptance.
- validate field 4 carries STORED bytes (record bytes always decide
  InvalidEncoding — found by diagnosis, fixed, tested).
- `finish` writes only on a Valid decision.
- `identities` are deterministic derivations reported on every created
  record (derivation ≠ validity claim); intermediate phases are
  honestly invalid, finals genuinely Valid.
- Agent transcript hash-chain + usage ledger at the tool boundary;
  CONTEXT derives whole_store_reads from it (never defaulted).

## Gate outcomes (this slice, wt-succ branch)

- Focused regressions green: bench.live unit tests (test_sley2_tool
  19, test_agent_access 6, test_taskpacks/test_tooling/test_oracle 14,
  e2e 2 with bound binaries); S3 G1 (7 suites), G2 (4), G3 perf;
  succ_live_packs_frozen (repacked PERF matches emitter).
- `live_case_driver` as a bare `cargo test` fails without its env
  (entry point, not a unit test — pre-existing; always driven with
  SUCC_JUDGE_* set).
- `make lint`: fmt_clean true after normalizing the three slice-owned
  test files; result FAIL on branch topology (working_tree_clean
  false) plus pre-existing pedantic clippy nits in slice headers.
  Candidate-bound lint-report.json preserved (restored after runs).
- `make quick`: 39 PASS steps, then stops at the fuzz-proof freshness
  gate (`proof-record-predates-lane-change`) — commit-topology check
  that fails on any development branch adding lane-path files after
  the proof commit (pre-existing for this branch at 3badd822, not a
  code regression). No fuzz re-run (multi-hour) on this branch.
