# Succession sley_2_0 live-arm coverage — 15 frozen corpus tasks

Status as of the current slice (wt-succ branch). "Scripted" = deterministic
witness script driving only the frozen agent tool surface (no model);
witnesses live in `bench/live/succ_witness_*.py`. No live-model
succession result follows from scripted acceptance. `ga_claimed=false`.

Current evidence logs: `bench/live/succ-trials-20260921/` (this pass).
Historical logs: `bench/live/succ-trials-20260920/` (superseded —
pre-repair judge outputs, retained, never overwritten) plus the frozen
S3 suites (`crates/sley-repo/tests/s3_*`, `crates/sley-vm/tests/s3_g3_perf`).

## Current 15-task view (this pass; maps required predicates to current evidence)

| Task | Positive (fresh e2e) | Negative (fresh e2e) | Remaining blocker |
|---|---|---|---|
| CREATE | ACCEPTED `trial_create.log` — genesis pack stages empty state; 4 checked primitives (subtotal/tax_mul/tax_div/total) authored via propose/compose/finish; judge discovers roles behaviorally, executes all checks + overflows natively, verifies composition 2500+181=2681 and wiring entry | wrong-op variants → ORACLE_CREATE_MISMATCH (`trial_create_neg_*.log`); overflow/value classifiers unit-pinned (UNCHECKED/WRONG_CENTS specified rules) | live-model trial |
| REPAIR | (A) superseded (pre-repair judge); judging path unchanged by this pass | static S3 families | re-proof pending (unchanged since 9e155305) + live-model trial |
| SIG | (A) superseded; judging path unchanged by this pass | static S3 | re-proof pending + live-model trial |
| MODULE | ACCEPTED `trial_module.log` — export grant on new_package via surface; observation held on 6 fixed inputs, reference_count 6, no-duplicate-impl extras | target-respecting no-op → ORACLE_STALE_IMPORT (`trial_module_neg.log`) | live-model trial |
| TYPE | BLOCKED (structural; see TYPE section below) — typedef + Failed(7) status migration validates (`trial_type_migration.log` ends MISSING_CASE); neg → ORACLE_BOOL_COMPAT_FIELD (`trial_type_neg.log`) | missing_case demonstrated live; bool_compat live | task-encoding change: 6d/6e in targets (fixture-design owner) |
| EFFECT | none (deterministic E7 refusal VM_LOWER_OPCODE_UNSUPPORTED, excluded.json pending rev16) | S3 pin s3_g3_effect_refusal_pin | adapter work + design/review gate + live-model trial |
| CAP | none (deterministic E7 refusal VM_LOWER_OPCODE_UNSUPPORTED, excluded.json pending rev16) | S3 pin s3_g3_cap_refusal_pin | adapter work + design/review gate + live-model trial |
| DEAD | BLOCKED — correct deletion inadmissible (see below) | reachable_changed → ORACLE_REACHABLE_CHANGED | review-gated production repair (tombstone proposal retained) |
| TEST | ACCEPTED `trial_test.log` — 3 submitted TestCase entities, driver-verified boundaries, impl byte-identical via root-bound gate | wrong expectation → ORACLE_TEST_MISMATCH (`trial_test_neg.log`); missing/duplicated unit-pinned | live-model trial |
| STALE | ACCEPTED `trial_stale.log` — guard flip; exact STALE_ROOT; genuine same-change-new-base rebase validates Valid | vacuous contender → ORACLE_REBASE_INVALID (`trial_stale_neg.log`) | live-model trial |
| MERGE | ACCEPTED `trial_merge.log` — union via surface (side bodies through allowed interface); root-bound union checks + observed dual-order re-validation | dropped theirs entity → ORACLE_MERGE_CONFLICT (`trial_merge_neg.log`) | live-model trial |
| PERF | ACCEPTED `trial_perf_large.log` — 5x5 governing inputs; 46-op scan → 7-op map transform via surface (58-op record); outputs identical, reduction ≥30%, effects empty, fuel non-regressing | flipped probe → ORACLE_OUTPUT_MISMATCH (`trial_perf_large_neg.log`) | live-model trial |
| CONTEXT | ACCEPTED `trial_context.log` — typedef F1 + 3-const closure; live count; bounded audit whole_store 0, targeted 4, ≤9222 B | incomplete closure refused at validation phase 6 (`trial_context_neg.log`); hidden-truncation/inconsistent-continuation/budget unit-pinned | live-model trial |
| ADVERSARY | (A) superseded; judging path unchanged by this pass | static S3 family | re-proof pending + live-model trial |
| CORRUPT | ACCEPTED `trial_corrupt.log` — constant restored (smoke) + exchange-pack rejection path: 2 bit-flips → exact EXCHANGE_DIGEST_MISMATCH, destination ref unchanged | wrong value → ORACLE_CORRUPT_UNRESTORED (`trial_corrupt_neg.log`) | live-model trial |

## Historical table (pre-repair judge; superseded, retained for provenance)

| Task | Positive (old, superseded) | Negative (old) |
|---|---|---|
| CREATE | none (blank-repo start; missing manifest → harness_error by design) | — (static S3 s3_g1_create) |
| REPAIR | accepted (e2e, clamp fix) | static S3 families |
| SIG | accepted (caller + CallDirect fix) | missing_caller → tool-level refusal (valid False); static S3 |
| MODULE | accepted (exports fix; 6 refs resolve; observation held) | stale_import → ORACLE_STALE_IMPORT |
| TYPE | accepted (3-phase compose: 2 block creates + switch/status migration) | missing_case → ORACLE_MISSING_CASE; bool_compat → ORACLE_BOOL_COMPAT_FIELD |
| EFFECT | none (statically pinned refusal) | VM_LOWER_OPCODE_UNSUPPORTED (S3 pin s3_g3_effect_refusal_pin) |
| CAP | none (statically pinned refusal) | VM_LOWER_OPCODE_UNSUPPORTED (S3 pin s3_g3_cap_refusal_pin) |
| DEAD | BLOCKED — correct deletion inadmissible (see below) | reachable_changed → ORACLE_REACHABLE_CHANGED |
| TEST | accepted (3 TestCase creates) | impl_touch → tool-level refusal; case_missing → ORACLE_CASE_MISSING |
| STALE | accepted (guard flip) | static S3 (guard_disabled) |
| MERGE | accepted (semantic union) | overlap → ORACLE_MERGE_CONFLICT |
| PERF | accepted (scan→ordered-map; 7→4 instr, 42.9% ≥ 30%; outputs identical) | faster_but_wrong → ORACLE_OUTPUT_MISMATCH (valid proposal, flipped probe changes digest) |
| CONTEXT | accepted (typedef F1 + 3-const closure, 3-phase compose) + access evidence whole_store_reads=0, 4 targeted reads, responses ≤ 9222 B | unbounded_read → QUERY_REQUIRED_FACT_OMITTED (live inventory test + static S3) |
| ADVERSARY | accepted (opcode fix; label ignored) | wrong_repair: operand-swap dual is a correct fix (accepted, correctly); static S3 family |
| CORRUPT | accepted (e2e restore) | static S3 (unflipped) |

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

## Acceptance-correctness repair pass (2026-09-20, supersedes judge outputs above)

The 11 scripted acceptances in the table above are outputs of the
pre-repair judge, not proof that frozen task contracts were satisfied.
Historical logs in `bench/live/succ-trials-20260920/` are retained as
superseded evidence and are not overwritten. New evidence goes to
`bench/live/succ-trials-20260921/`. Three tiers stay separated:

- (A) Scripted fixture acceptance: deterministic tool-surface script,
  judged by the repaired judge.
- (B) Full frozen-task satisfaction: every frozen predicate proved
  (see per-task below). Only TEST is newly proved in this pass.
- (C) Live-model campaign evidence: none claimed. `ga_claimed=false`.
  No acceptance campaign started; readiness prerequisites unsatisfied.

Per-task frozen predicates (proved vs still missing):

- CREATE: no scripted positive. Blank-repo staging exists
  (`stage_initial` blank, no pack); live judging path returns
  harness_error on missing manifest by design. Missing: blank-program
  setup as harness work (not a model limitation), judging path for a
  blank start, deterministic positive/negative witnesses without
  seeding a completed solution. Static S3 only.
- REPAIR: (A) superseded (old judge). Frozen clamp-combine predicates
  not re-proved under repaired judge in this pass. Pending re-run.
- SIG: (A) superseded. Caller/CallDirect, arity, fixed-input execution
  not re-proved here. Pending re-run.
- MODULE: (A) superseded. Frozen predicates require namespace-binding
  change and resolving references, not only an export-list change.
  Current export-grant embodiment states the residual gap; no adopted
  contract establishes export-list equivalence. Pending: binding-level
  fix or adopted equivalence contract under review gate.
- TYPE: (A) superseded. Frozen predicates require full tagged-state
  migration, exhaustive handling, explicit Failed(error_code), and
  deterministic values. Four Required blocks reachable through the old
  Boolean dispatch is not sufficient by itself. Open question whether
  restrictive fixture targets are an erroneous task encoding; they are
  not treated as superior to the frozen task. Pending re-proof.
- EFFECT/CAP: no scripted positive (deterministic E7 refusal
  VM_LOWER_OPCODE_UNSUPPORTED, excluded.json pending rev16
  handle-model schema-epoch decision). This is an unimplemented
  adapter/lowering gap, not an intentional production-profile
  exclusion and not merely "awaiting an unscripted trial". Pending:
  authorized independent adapter work; any semantic/profile change
  requires the existing design/review gate (exact change prepared
  separately, gate retained).
- DEAD: BLOCKED (preserved diagnosis below). No benchmark exception,
  no orphan workaround, no unreviewed semantic landing. Tombstone
  proposal + regression spec in `bench/live/DEAD-TOMBSTONE-PROPOSAL.md`
  (proposal only, review gate retained).
- TEST: (A) proved under repaired judge + (B) proved 2026-09-20:
  `succ-trials-20260921/trial_test_repaired.log` — three submitted
  TestCase entities covering success/div0/overflow with exact expected
  outcomes, impl byte-identical, driver-verified through native
  machinery. Count-alone acceptances superseded.
- STALE: (A) superseded. Repaired judge requires Valid decision
  decoding, correct validate shape, stale rejection with no partial
  write, and a freshly assembled rebase candidate (outer-binding-only
  resubmission never counts). Pending re-run under repaired judge.
- MERGE: (A) superseded. Frozen predicates require branch changes and
  reverse-order semantic equivalence through the production merge
  path. "Holds by construction" and same-result reread do not
  substitute for an observed commutativity check. Pending re-proof.
- PERF: (A) superseded. Frozen predicates require judging the
  submitted transformation on the fixed large input (outputs, effects,
  instruction reduction, memory ceiling). The 2x2 scan->ordered-map
  result is retained as a development smoke test only; static evidence
  does not validate the submitted candidate. Pending large-input proof.
- CONTEXT: (A) superseded (compose evidence + audit repaired).
  Trusted access evidence now requires hash chain + transition linkage
  + final linkage + binary/fixture binding + whole-store on every read
  route + omitted/truncated/bounds enforcement. Old transcripts lack
  compose in/out and binary binding and are unverifiable under the new
  judge. Pending re-run.
- ADVERSARY: (A) superseded. Pending re-run.
- CORRUPT: (A) superseded (e2e). Frozen predicates require rejection
  of the corrupted exchange pack with digest failure and unchanged
  destination ref. Restoring a constant's value is a different
  operation and cannot substitute. Pending re-proof through the
  exchange path.

Trusted access-evidence status: hash chain proves order/tamper only.
Completeness via durable-before-release (tool records before printing;
evidence loss is terminal exit 2 with no success released) plus
transition/final linkage, response bounds, omitted/continuation
accounting. No-unrecorded-route via provider confinement
(`CodexExecAdapter`: ephemeral, workspace-write sandbox, no user
config/rules, no env inherit) plus CLI allowlist (no commit/merge/
execute/export/import/report/session paths), repo-untouched
enforcement, and side-scope marking. Totals derived from complete
trusted evidence across every session/phase; unknowns reject rather
than report zero. Deterministic regressions:
`bench/live/tests/test_acceptance_repairs.py` (24 tests: compose
evidence, transitions, Valid decoding, TEST boundaries, provider/
whole-store/omitted/binary).

## This pass implementation notes (2026-09-20, supersedes the 2026-09-20 repair section above where they conflict)

Trusted evidence boundary (task 1): runner-owned copies of the agent
transcript, finished artifact, and usage ledger plus an independently
controlled completion binding are stored in the content-addressed
artifact store (fsync file + directory) BEFORE any oracle verdict is
acted on; the completion payload names the attempt and the exact
stored bytes and is reconciled by verify_attempts. Transcript present
but usage missing/malformed, or any sink failure, is harness_failure
(LIVE_EVIDENCE_SINK_INVALID) with the oracle skipped — never a silent
zero, never an unrecorded success. attempts.jsonl appends now fsync
the parent directory too. Ownership/completeness/durability stay
separate claims: the chain proves order/tamper only; completeness via
durable-before-release + transition/final linkage + summary/session
reconciliation; no-unrecorded-route via provider confinement (argv/env
verified through real process launch: env isolation, literal argv,
output limits, kill-group timeout) + tool allowlist. Retained
limitation: OS-level enforcement of the provider sandbox relies on
the provider binary; the runner cannot independently re-verify it, so
full access acceptance still requires the chained evidence, not flags
alone. Judge audit additionally reconciles per-entry summaries against
enumerated session items exactly (over- or under-stated accounting
rejects). Precondition reads assemble in chunks across fresh sessions
sharing the invocation transcript (frozen per-session request limit).

CONTEXT semantics (task 2): query.root/restricted/continue/refs.list
are bounded paging routes, not whole-store by name. Each bounded
response must fit the per-response cap; any omitted/truncated page
must be followed by query.continue in-scope (hidden truncation and
inconsistent continuations reject); cumulative agent-visible bytes
fit a 4 MiB trial budget; whole-store is inventory/side only.
Positive: genuinely bounded page + continue accepted. Retained
limitation: transcripts record bounds/digests, not requested limit
values or continuation tokens, so token-equality is unverified.

Root binding (task 3): `_live_object_id` (entity.version under the
accepted head), `_bound_object_bytes`, `_entity_bound_bytes`,
`_live_entity_set` (chunked fresh sessions), `_decode_bound_body`,
`_live_object_count` (revision.read field 6). TEST impl gate, MERGE
comparisons, and CONTEXT minimum all resolve currency through live
bindings; file scans are documented non-currency inventory. Regressions
with a filename-order decoy, absent-entity non-binding, and bounded
continuation positives/negatives.

CREATE (task 4): genesis pack (workspace/policy/anchors, no program
entities — runner-owned empty-state init, never a solution) +
trusted manifest with behavioral (not identity) specs; judging path
discovers roles by execution, checks overflow codes, composition
identity over observed values, and entry wiring. Checked ops return
Result (never bare SInt), so no multi-op chaining is expressible:
single-op primitives (the S3 shape) sequenced by the caller.

PERF (task 4): re-emitted at the governing 5x5 scale (46-op scan;
the ~58-op fix record fits the 64-op surface cap); judge measures the
submitted candidate on fixed large inputs (outputs, ≥30% reduction,
empty effects, fuel non-regression). S3 40x40 stays the class pin.
Map transforms require VariantSwitch unwrapping (S3 G3 pattern);
chained SInt/Map intermediates do not lower.

STALE (task 4): exact STALE_ROOT (never substring); genuine rebase via
the new `record_operations` codec op (contender's own ops replayed on
H1 with fresh bindings; creates re-derive, vacuous contenders
rejected); outer-binding probe retained as diagnostic only.

MERGE (task 4): root-bound union checks plus observed dual-order
re-validation (contender rebases onto both side heads through
production validation; already-satisfied replaces skipped).
Production merge-judge is inapplicable (side packs are independent
geneses with no common-ancestor transaction). `side` now reports
current-entity bodies through the allowed interface (currency via
live bindings — file order once shadowed the ours value with a stale
duplicate). Admissible record order: creates before replaces.

CORRUPT (task 4): exchange-path acceptance — two bit-flips → exact
EXCHANGE_DIGEST_MISMATCH (exchange layer; bundle-layer
PACK_DIGEST_MISMATCH is a different path the surface never drives),
destination head + live count unchanged; constant restore kept as
smoke. Also fixed: `run_fixture_oracle` never selected the raw/legacy
oracle (UnboundLocalError — pre-existing).

MODULE (task 4): export grant re-proved; both packages start
unexported under one root namespace, so the frozen expectation is
satisfiable exactly by the grant — no literal second namespace
exists to re-home between, no production change needed.

## TYPE: structural block (task 4 outcome, not a deferral)

Proven through the surface (do not retry without a task-encoding change):
- Phase-7 judges operations, and operations in blocks unreachable
  from their function entry fail (ControlFlowError; demonstrated with
  reachable-only vs dead-block probes).
- New case arms would be unreachable: 6d's edges are frozen
  (true→6d self-loop, false→6e), so no new block can join switch 6b's
  CFG without editing 6d — not a fixture target.
- The frozen edges further lock 6c:Bool (CondBranch condition) and the
  switch result:Bool (6e returns 6c); JobState-typed arms cannot
  validate. Empty trap blocks would satisfy the count vacuously and
  are refused as gaming.
- Delivered instead: JobState typedef (Queued/Running/Succeeded unit
  + Failed(SInt)) validates; status genuinely migrates to Failed(7)
  (explicit code, deterministic); neg → ORACLE_BOOL_COMPAT_FIELD.
- Owner: benchmark fixture design (emit table targets); gate: add
  6d/6e to targets. Not a production semantic change.

## Gate outcomes (this pass, wt-succ branch)

- Focused suites green (see FINAL REPORT for exact counts).
- `cargo fmt --check` status recorded in the final report (not
  conflated with `make lint` PASS).
- `make quick` / `make lint` NOT claimed PASS (branch topology +
  pre-existing freshness gates; recorded honestly in the report).
- No fuzz re-run on this branch (multi-hour); freshness rules
  unchanged, no exemptions taken.

Production/review/provider/operator dependencies: DEAD tombstone
semantic change needs independent review (provider unavailable, gate
retained); EFFECT/CAP rev16 handle-model decision needs design/review;
TYPE 6d/6e target-encoding change needs fixture-design gate; and all
live-model campaign prerequisites (seeds/budgets preregistered,
90 attempts minimum, two-host reproducible package/dossier, current
source-bound Council transcripts) remain unsatisfied. No campaign
started. `ga_claimed=false`.
