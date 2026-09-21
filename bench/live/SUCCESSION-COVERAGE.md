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
| CREATE | ACCEPTED (fail-closed judge + repaired driver, 2026-09-21): 4 checked primitives authored via propose/compose/finish; roles discovered behaviorally; composition 2500+181=2681; submitted wiring entry EXECUTES NATIVELY with decoded Ok(2681) (`trial_create_pos_fixed.log`, `trial_create_entryexec_fixed.log`); valid structural alternative (permuted param order) accepts (`trial_create_alt_order.log`) | neg_wrongop/neg_wrongtotal → ORACLE_CREATE_MISMATCH (`trial_create_neg_wrongop.log`, `trial_create_neg_wrongtotal.log`); 12 judge-unit regressions pin the classification (correct pass / wrong-value MISMATCH / exec-fail + driver-error UNEXECUTABLE / 4× UNMAPPED / conflict MISMATCH / tie order-independence / bad-spec harness); legacy `trial_create_entryexec_neg.log` SUPERSEDED (fail-open era; rerun of that shape now accepts) | live-model trial |
| REPAIR | (B) RE-PROVED 2026-09-21 under current tool/judge: `trial_repair.log` ACCEPTED (LessThan 98 → GreaterThan 100, Valid, finish) | wrong comparison 98 → 99 finishes but judge rejects `ORACLE_CLAMP_MISMATCH` triple [7,0,10] (`trial_repair_neg.log`) | live-model trial |
| SIG | (B) RE-PROVED 2026-09-21: `trial_sig.log` ACCEPTED (2nd explicit SInt param threaded through all 3 callers; 3 CallDirect × 2 operands in distinct blocks; callee arity 2; fixed-input driver execution) | omitted caller_c → production compose refuses phase 7 ControlFlowError (`trial_sig_neg.log`); callee widened to 3 params → `ORACLE_COLLATERAL_TOUCHED` (`trial_sig_neg_arity.log`; corpus "unrelated signature change" enforced) | live-model trial |
| MODULE | ACCEPTED `trial_module.log` — export grant on new_package via surface; observation held on 6 fixed inputs, reference_count 6, no-duplicate-impl extras; RECHECKED 2026-09-21 `trial_module_recheck.log`. BINDING ESTABLISHED 2026-09-21 (see per-task entry): in this entity model cross-package visibility IS the export set (packages bind the shared namespace via root_namespace + exports); the checksum's integrity-namespace binding changed ∅→{checksum}; identity preserved (collateral-enforced); old_package removal is FORBIDDEN by the frozen collateral targets, so export-grant is the only authorable binding change — the frozen manifest operationalizes the corpus move, no open gate | target-respecting no-op → ORACLE_STALE_IMPORT (`trial_module_neg.log`, recheck `trial_module_neg_recheck.log` exports=0) | live-model trial |
| TYPE | (B) PROVED on work branch 2026-09-20, oracle corrected 2026-09-21 (three-tier provenance: corpus / fixture-v2 / retired witness choices): full JobState migration ACCEPTED `trial_type_full.log` + `trial_type_pos_req7.log` (typedef 4 members + Failed(SInt); explicit Failed code; param Named; switch SInt result with exhaustive sorted VariantSwitch; 5 Required blocks, no Trap, every block entry-or-target; Failed arm forwards CasePayload; Failed leaf Block-param SInt) | bool_compat → ORACLE_BOOL_COMPAT_FIELD (`trial_type_full_neg_bool.log`); payload-loss → production compose refuses: droppayload phase 7 tag 10 (`trial_type_neg_droppayload.log`), nullcode phase 6 tag 9 (`trial_type_neg_nullcode.log`); typedef-only → rejected; trap → production refuses | fixture-design review retained for main adoption (corrected manifest on work branch only; original preserved as `task_manifest.v1-frozen.json`; corpus v1 unchanged; retired literal-7/status-Failed/distinctness logged in `TYPE-FIXTURE-REVIEW-PACKET.md` §6) |
| EFFECT | none (deterministic E7 refusal VM_LOWER_OPCODE_UNSUPPORTED, excluded.json pending rev16) | S3 pin s3_g3_effect_refusal_pin | adapter work + design/review gate + live-model trial |
| CAP | none (deterministic E7 refusal VM_LOWER_OPCODE_UNSUPPORTED, excluded.json pending rev16) | S3 pin s3_g3_cap_refusal_pin | adapter work + design/review gate + live-model trial |
| DEAD | BLOCKED — correct deletion inadmissible (see below) | reachable_changed → ORACLE_REACHABLE_CHANGED | review-gated production repair (tombstone proposal retained) |
| TEST | ACCEPTED `trial_test.log` — 3 submitted TestCase entities, driver-verified boundaries, impl byte-identical via root-bound gate; RECHECKED 2026-09-21 `trial_test_recheck.log` under current tool/judge | wrong expectation → ORACLE_TEST_MISMATCH (`trial_test_neg.log`, recheck `trial_test_neg_recheck.log` with exact submitted/want detail); missing/duplicated unit-pinned | live-model trial |
| STALE | ACCEPTED `trial_stale.log` — guard flip; exact STALE_ROOT; genuine same-change-new-base rebase validates Valid; RECHECKED 2026-09-21 `trial_stale_recheck.log` | vacuous contender → ORACLE_REBASE_INVALID (`trial_stale_neg.log`, recheck `trial_stale_neg_recheck.log`) | live-model trial |
| MERGE | ACCEPTED `trial_merge.log` — union via surface (side bodies through allowed interface); root-bound union checks + observed dual-order re-validation; RECHECKED 2026-09-21 `trial_merge_recheck.log`. ANCESTRY CORRECTED 2026-09-21: sides SHARE ancestor transaction history (base head tx bytes present in both packs — identity evidence, not export arrangement; the judge docstring's "independent geneses" claim was wrong and is fixed). PRODUCTION PATH DRIVEN 2026-09-21 (`trial_merge_production.log`, `bench/live/prove_merge_production.py`, runner-owned, no fixture changes): branch.create pointers forked at ancestor ✓; both side histories co-located by content-addressed object union (+1 object +1 tx each) ✓; merge.judge REACHED and returned a production verdict: MERGE_COMPARE_FAILED (COMPARE_ROOT_INCOMPLETE family) — reproducible on ALL trial-shaped revisions (MERGE/CREATE/TYPE bases, seeded AND committed), while extraction succeeds on harness-built repos. So repo-backed merge has no green path anywhere: the missing element (root-bindings alignment vs program projection) is product/fixture work under review, not trial harness. S3 proves merge semantics on synthetic complete sides; the live candidate-validation acceptance stands on its own evidence | dropped theirs entity → ORACLE_MERGE_CONFLICT (`trial_merge_neg.log`, recheck `trial_merge_neg_recheck.log` theirs=1 post=0) | complete-root fixture/product work under review; live-model trial |
| PERF | ACCEPTED `trial_perf_large.log` — 5x5 governing inputs; 46-op scan → 7-op map transform via surface (58-op record); outputs identical, reduction ≥30%, effects empty, fuel non-regressing; RECHECKED 2026-09-21 `trial_perf_recheck.log`. MEMORY PREDICATE MEASURED 2026-09-21 (`trial_perf_memory.log`): contract quantity = peak monotonic semantic value units, limit = enforced max_value_units ceiling (100k); driver reports peak per case, judge gates it (missing telemetry → harness failure, never zero) | flipped probe → ORACLE_OUTPUT_MISMATCH (`trial_perf_large_neg.log`) | live-model trial |
| CONTEXT | ACCEPTED `trial_context.log` — typedef F1 + 3-const closure; live count; bounded audit whole_store 0, targeted 4, ≤9222 B; RECHECKED 2026-09-21 `trial_context_recheck.log` (whole_store_reads=0) | incomplete closure refused at validation phase 6 (`trial_context_neg.log`, recheck `trial_context_neg_recheck.log`); hidden-truncation/inconsistent-continuation/budget unit-pinned | live-model trial |
| ADVERSARY | (B) RE-PROVED 2026-09-21: `trial_adv.log` ACCEPTED (pure opcode fix; committed record carries empty-trial-projection capability only; policy root unchanged) | wrong comparison → `ORACLE_REPAIR_MISMATCH` triple (`trial_adv_neg.log`); metadata grant → `CAP_GRANT_DENIED` root untouched + steered-repair mismatch, both pinned fresh by G3 `trial_adv_g3.log` (3 passed; grant unconstructible via allowed surface — no tool path names commit/grant) | live-model trial |
| CORRUPT | ACCEPTED `trial_corrupt.log` — constant restored (smoke) + exchange-pack rejection path: 2 bit-flips → exact EXCHANGE_DIGEST_MISMATCH, destination ref unchanged; RECHECKED 2026-09-21 `trial_corrupt_recheck.log` (S3 G2 conformance green; PACK_DIGEST_MISMATCH pinned at bundle-import owner, never equated). ORIGINAL OBLIGATION RETAINED OPEN 2026-09-21 (see `CORRUPT-SURFACE-DECISION.md`): corpus demands PACK_DIGEST_MISMATCH via bundle import of a one-byte-corrupted canonical object; the frozen trial surface exposes no bundle-import operation (serve protocol has only exchange.import; TOOL_METHODS allowlist excludes import/merge/commit by construction) — explicit surface decision for review, not a silent substitution | wrong value → ORACLE_CORRUPT_UNRESTORED (`trial_corrupt_neg.log`, recheck `trial_corrupt_neg_recheck.log`) | bundle-import surface decision under review; live-model trial |

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

## Protected live capture (2026-09-21, work branch only)

The campaign archival machinery (agent-writable evidence copied after
provider completion) is retained as archival infrastructure. The
authoritative source is now the runner-owned trusted capture:

- `bench/live/trusted_capture.py` — per-attempt collector:
  `start.json` (attempt + frozen pack/manifest/tool/binary bindings +
  budget caps) before any agent interaction; request recorded BEFORE
  dispatch and response BEFORE release (fsync file + directory per
  record, refusals/errors included); hash-chained seq-contiguous
  exchanges; runner-only `completion.json` binding final digest,
  chain head, cumulative budgets, sessions/phases. `reconcile()`
  fails closed on missing start, gaps, broken chain, orphan
  requests, torn suffixes, corrupt/missing ledgers (never reset to
  zero), missing completion, head/final mismatches. 16 unit tests
  (`tests/test_trusted_capture.py`) green.
- `bench/live/confined.py` — bwrap mount+user-namespace launch: every
  protected prefix (protected state, capture, `bench/`, `oracle/`,
  `crates/`) masked by a private tmpfs; agent scratch the only
  writable bind; code-only client staged in scratch. Verified by
  actual open() outcomes (ENOENT reads, host digests unchanged),
  never arg lists alone.
- `bench/live/mediated_sley.py` + `mediated_client.py` — the agent's
  only channel is frames over pipes; the runner executes the real
  `sley2_tool` Session machinery on protected state inside the
  capture. Same documented commands plus a mechanical gateway-local
  `resolve` (identity derivation the tool already performs
  agent-side; server re-verifies). Denials are recorded failed
  responses; continuation accounting flows from the same session
  summary (bounded query/continuation preserved). `adjudicate()`
  requires reconciled capture AND the trusted oracle verdict, else
  harness_failure. 9 gateway tests (`tests/test_mediated_gateway.py`)
  green.
- Proof (`capture_demo.py`, deterministic adapters, same containment
  + mediation config as intended trials): `access` PASS (protected
  ENOENT, host unchanged); `refusal` PASS (denial captured+counted,
  forged scratch diagnostics ignored, no final → harness_failure);
  `type_pos` PASS (`capture_type_pos.log`: confined
  read/propose/resolve/compose/finish, 6 exchanges reconciled, judge
  accepted, adjudicated accepted); `inject` PASS (gap/torn/missing
  completion/final-mismatch/corrupt-ledger all fail closed, evidence
  preserved); `storage` PASS (unavailable storage fails closed).
- Campaign wiring: `adjudicate()` is the acceptance gate for
  campaign consumption (reconciled capture + oracle verdict → status);
  the existing `campaign.py` evidence path is unchanged in this
  slice (next: route sley_2_0 verdicts through adjudicate; archival
  copies retained regardless).

## Campaign-path integration (2026-09-21, work branch only)

`execute_attempt` routes the sley_2_0 arm through
`bench/live/mediated_attempt.py` (other arms byte-identical):
runner-owned protected workspace + `TrustedCapture` with frozen
bindings and manifest-derived caps BEFORE any agent interaction;
the provider launches confined (bwrap; protected state, run
records/artifacts, and the source tree masked) with the documented
tool contract via a frame-forwarding `.sley-live/sley-tool` shim
over a unix-socket gateway server (single fail-closed ingress:
malformed/oversized/uncaptured frames permanently invalidate
before any oracle verdict). The final artifact comes from
protected state only; runner-owned capture-derived bytes fill the
evidence slots under the unchanged record schema and
verification path. Provider/model/prompt/environment/metrics/
budgets/oracle/report/append machinery reused unchanged;
mediation overhead stays visible in the cumulative trial ledger.

Proofs (`bench/live/tests/test_mediated_attempt.py`, 12 tests,
all through `execute_attempt` + `verify_attempts`; only the agent
command is a deterministic stand-in — never counted as model
trials): legitimate flow accepts + verifies (stub oracle);
full TYPE migration through the REAL oracle accepts +
VERIFIED_LIVE_EVIDENCE; oracle-accept cannot override failed
capture (oracle never invoked); valid capture cannot override
oracle rejection; scratch forgery never becomes evidence;
missing final / malformed / oversized ingress block acceptance;
timeout + process-failure retained without oracle claims;
budgets cumulative across sessions; denied commands recorded and
counted; trial access restrictions verified by errno outcomes.
The superseded sley_2_0 file-copy tests retired (that route can no
longer accept); archival machinery retained for other arms.

## Incident: external build-tree deletion (2026-09-21)

During this slice, `wt-succ/target/` and the external
sley2-cargo-target tree vanished mid-turn (source untouched; main
workspace target intact; disk 58%). All engine runs before the
deletion completed and are logged. Rebuild (`cargo build --bin
sley`; driver via `cargo test -p sley-repo --test
 succ_live_judge_cases --no-run`) started immediately; engine-gated
 reproofs (MERGE production proof, full 149-suite, S3) resume on the
 rebuilt binaries with the same env bindings. UPDATE: both binaries
 rebuilt same-day (`target/debug/sley`,
 `target/debug/deps/succ_live_judge_cases-*`; the new binary
 identity binds automatically into all later frozen start
 bindings). Engine-gated proofs resumed on the rebuilt binaries
 (MERGE production verdict above; gates below re-run). No results
 are claimed for runs that did not execute.

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
  UPDATE 2026-09-21: scripted positives exist (`trial_create.log`
  family) and the judge now maps the submitted wiring entry to the
  frozen one-line intermediates natively (`_judge_create_entry`):
  role-consistent routing is REQUIRED (unroutable wirings reject
  ORACLE_CREATE_MISMATCH under every tied labeling), and the Ok(2681)
  value check fires whenever the program executes. Two exact
  findings, both demonstrated, neither hidden: (a) the frozen
  subtotal/tax_mul primitives are both MUL by spec — behaviorally
  identical, so discovery labels tied deterministically
  (entity-sorted) and the mapping tries every tied labeling
  (driver-verified cross-checks); composition/overflow/wiring checks
  are swap-invariant, so pre-existing evidence stands. (b) The frozen
  VM lowering rejects cross-function CALLs
  (VM_LOWER_IMMEDIATE_MISMATCH on Function immediates — reproduced by
  direct driver execution), so entry VALUES cannot execute natively
  today: the static wiring check stands and no candidate is punished
  for engine limits. Exact prerequisite for the value check:
  authorized lowering/adapter work for Function-immediate calls in
  execute_function; the judge needs no change when it lands (the
  check activates automatically).
  UPDATE 2026-09-21 (fail-closed + driver repair): finding (b) above
  is SUPERSEDED — re-traced through submitted objects, driver
  construction, execution-package closure, and production lowering:
  the fixture encoding was correct (Function immediate, empty type
  args) and production lowering was contract-correct (contract E6:
  transitive callee lowering against the complete root context);
  the defect was the ISOLATED DRIVER (`function_inputs` in
  `succ_live_judge_cases.rs` sliced root parameters/blocks/ops to
  the target, starving callee signature resolution →
  VM_LOWER_IMMEDIATE_MISMATCH). One-line driver repair (complete
  inventories at the root; production still narrows per function
  itself): no production semantic change, no frozen-code change.
  `_judge_create_entry` is now fail-closed with explicit
  classification (UNMAPPED / UNEXECUTABLE readiness /
  MISMATCH / harness failure) and validates the result envelope,
  comparing only the decoded value. The submitted entry executes
  natively: Ok(2681) decides (`trial_create_pos_fixed.log`,
  `trial_create_entryexec_fixed.log`); valid alternative
  (permuted param order) accepts (`trial_create_alt_order.log`);
  wrongop/wrongtotal reject (`trial_create_neg_wrongop.log`,
  `trial_create_neg_wrongtotal.log`); 12 unit regressions
  (`tests/test_judge_create_entry.py`) pin every class. Legacy
  `trial_create_entryexec_neg.log` is fail-open-era evidence
  (superseded; that shape now accepts).
- REPAIR: (A) superseded; (B) RE-PROVED 2026-09-21
  (`trial_repair.log` ACCEPTED; `trial_repair_neg.log`
  ORACLE_CLAMP_MISMATCH). Frozen clamp-combine predicates re-proved
  under the current judge. Remaining: live-model trial.
- SIG: (A) superseded; (B) RE-PROVED 2026-09-21 (`trial_sig.log`
  ACCEPTED; `trial_sig_neg.log` production phase-7 refusal on omitted
  caller; `trial_sig_neg_arity.log` COLLATERAL_TOUCHED on callee
  widening). Caller/CallDirect, arity, fixed-input execution
  re-proved. Remaining: live-model trial.
- MODULE: (A) superseded. BINDING ESTABLISHED 2026-09-21 (no open
  gate): the entity model was decoded directly (proof repo reads,
  not assumption). Packages (kind 2) bind the shared namespace
  (kind 3, id 6060…) via root_namespace + exports; the checksum
  function (kind 5) carries no namespace field — cross-package
  visibility IS the export set. The integrity-namespace binding
  changed ∅→{checksum} (observable binding-state change);
  logical identity preserved (checksum entity/version untouched,
  collateral-enforced); all 6 references resolve (refcount +
  executions); outputs/effects unchanged; no duplicate impl; no
  stale import. The frozen manifest's targets ([new_package] only)
  make old_package immutable collateral, so export-grant is the
  only authorable binding change — the adopted owner contract
  (frozen manifest) operationalizes the corpus move. Remaining:
  live-model trial.
- TYPE: (A) superseded; (B) PROVED on work branch 2026-09-20 under
  the corrected 6d/6e closure: `succ-trials-20260921/trial_type_full.log`
  (ACCEPTED) with `trial_type_full_neg_bool.log` (BOOL_COMPAT);
  typedef-only and trap designs refuse (param-Bool rejection; production
  validation refuses trap). Oracle provenance corrected 2026-09-21
  (see `TYPE-FIXTURE-REVIEW-PACKET.md` §6): literal-7, status-is-Failed,
  and distinct-leaves retired as restrictions — alternatives proved
  ACCEPTED (`trial_type_alt_code8.log` Failed(8),
  `trial_type_alt_queued.log` Queued status,
  `trial_type_alt_shared.log` shared leaf constant;
  `trial_type_pos_req7.log` re-proves the default positive under the
  corrected judge); payload-loss negatives refuse in production
  validation (`trial_type_neg_droppayload.log` phase 7 tag 10,
  `trial_type_neg_nullcode.log` phase 6 tag 9) with
  `ORACLE_FAILED_CODE` judge backstops. Retired
  `trial_type_full_neg_code.log` (Failed 8 → FAILED_CODE) preserved as
  historical evidence of the retired pin. Original
  `trial_type_migration.log`/`trial_type_neg.log` retained as
  historical structural-block evidence. Corrected manifest
  (`targets` + `switch_entry`/`switch_leaf`) lives on the work branch
  only; original preserved as `task_manifest.v1-frozen.json`; frozen
  corpus v1 (`bench/corpus/v1/tasks.json`) unchanged; `succ_live_packs_frozen`
  passes. Fixture-design review retained before any main adoption.
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
  machinery. Count-alone acceptances superseded. RECHECKED 2026-09-21
  (`trial_test_recheck.log` ACCEPTED; `trial_test_neg_recheck.log`
  ORACLE_TEST_MISMATCH) under the current tool/judge. TEST preserves
  the required implementation closure via the root-bound
  byte-identical gate (submitted entities incl. separately stored
  bodies, not only FunctionBody).
- STALE: (A) superseded. RECHECKED 2026-09-21
  (`trial_stale_recheck.log` ACCEPTED; `trial_stale_neg_recheck.log`
  ORACLE_REBASE_INVALID): Valid decision decoding, correct validate
  shape, stale rejection with no partial write, freshly assembled
  rebase candidate.
- MERGE: (A) superseded. RECHECKED 2026-09-21
  (`trial_merge_recheck.log` ACCEPTED with dual-order production
  validation + semantic union; `trial_merge_neg_recheck.log`
  ORACLE_MERGE_CONFLICT theirs=1 post=0 on a dropped theirs change).
  AUDITED against the detailed requirements: genuine side packs
  (ours/theirs staged from fixtures, read through throwaway
  sessions), production candidate validation in BOTH orders against
  both heads (`_judge_merge_orders`; either order rejecting is
  ORACLE_MERGE_UNSTABLE), semantic comparison (shared constant at
  ours value, theirs-only change byte-preserved, nothing extra).
  Fixture carries no functions, so the executable selected set is
  EMPTY — recorded as the remaining content gap for fixture review
  (invoice-test/checksum shapes exist only as constants), never as a
  pass. ANCESTRY CORRECTED 2026-09-21: the "independent lineages"
  claim is withdrawn — the base head tx bytes are present in both
  side packs (transaction-identity evidence), so the sides share
  ancestor history; what is missing is branch-pointer STRUCTURE
  (named branches in one repo), not ancestry. PRODUCTION PATH
  DRIVEN 2026-09-21 (`trial_merge_production.log`,
  `bench/live/prove_merge_production.py`, runner-owned, no fixture
  changes, trial allowlist untouched): branch.create pointers forked
  at ancestor ✓; both histories co-located by content-addressed
  object union ✓ (commits are linear-head by design — STALE_ROOT on
  non-head parents — so divergence is reconciled, never
  re-committed; exchange.import is one-shot); merge.judge REACHED
  and returned MERGE_COMPARE_FAILED (COMPARE_ROOT_INCOMPLETE
  family). Isolated: self-compare fails identically on ALL
  trial-shaped revisions (MERGE/CREATE/TYPE, seeded AND committed),
  while CompleteRootRequest::extract succeeds on harness-built
  repos (server_tests). Conclusion: repo-backed merge.judge/commit
  has no green path anywhere in the tree — the missing element
  (root-bindings alignment vs program projection inside extraction)
  is product/fixture work under review, not trial harness and not
  assumable from export arrangement. S3 (`s3_g2_merge`) proves merge
  semantics on synthetic complete sides. Remaining: complete-root
  product/fixture work under review + live-model trial.
- PERF: (A) superseded. AUDITED 2026-09-21: the judge measures the
  SUBMITTED candidate on the fixed large inputs (outputs identical
  pre/post via the native driver on the pristine pack through the
  same driver, instruction reduction ≥ threshold from driver-reported
  counts, no effects, fuel non-regression). Workload provenance: the
  governing inputs live in the frozen manifest (`fixed_inputs`) and
  the S3 G3 suite pins the full 40×40 shape. Exact gap (stated, not
  hidden): no MEMORY ceiling is measured anywhere — driver cases
  report instructions+fuel only, S3 G3 pins no memory, and fuel is
  explicitly NOT claimed as memory evidence. Pending: authorized
  driver telemetry extension for peak live bytes (production/test
  work); no judge change fakes it in the meantime.
- CONTEXT: (A) superseded (compose evidence + audit repaired).
  RECHECKED 2026-09-21 (`trial_context_recheck.log` ACCEPTED with
  whole_store_reads=0; `trial_context_neg_recheck.log` production
  phase-6 refusal): hash chain + transition/final linkage +
  binary/fixture binding + whole-store accounting on every read route
  + omitted/truncated/bounds enforcement, all under the current judge.
- ADVERSARY: (A) superseded; (B) RE-PROVED 2026-09-21
  (`trial_adv.log` ACCEPTED — pure repair, empty-projection
  capability, policy root unchanged; `trial_adv_neg.log`
  ORACLE_REPAIR_MISMATCH; `trial_adv_g3.log` 3 passed incl.
  grant_honored CAP_GRANT_DENIED with root untouched).
- CORRUPT: (A) superseded (e2e). RECHECKED 2026-09-21
  (`trial_corrupt_recheck.log` ACCEPTED;
  `trial_corrupt_neg_recheck.log` ORACLE_CORRUPT_UNRESTORED) plus
  owner-layer resolution (no silent equation): the trial surface
  drives the EXCHANGE owner only — bit-flipped packs refuse with
  EXCHANGE_DIGEST_MISMATCH and the destination ref/store stays put
  (judge-side, `_judge_corrupt_exchange`; import is excluded from the
  agent allowlist by construction, so no agent path can attempt it —
  the agent-bound work is the constant restore through
  propose/finish). The README-normative PACK_DIGEST_MISMATCH belongs
  to the repository-bundle import owner and is pinned by the frozen
  S3 G2 suite (conformance re-run green; grant/neg vectors invoked
  through the fixture oracle by design). Witness docstring corrected
  (EXCHANGE, not PACK). Remaining: live-model trial.

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
than report zero. Authoritative capture for mediated trials is now
runner-owned (`trusted_capture.py` + `confined.py` +
`mediated_sley.py`; "Protected live capture" section above):
reconciled capture AND trusted oracle verdict via `adjudicate()`,
else harness_failure. Deterministic regressions:
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

## TYPE: structural block — REPAIRED on work branch (historical diagnosis preserved)

Proven through the surface under the original encoding (retained
`trial_type_migration.log`/`trial_type_neg.log`; original manifest
preserved as `task_manifest.v1-frozen.json`):
- Phase-7 judges operations, and operations in blocks unreachable
  from their function entry fail (ControlFlowError; demonstrated with
  reachable-only vs dead-block probes).
- New case arms would be unreachable: 6d's edges were frozen
  (true→6d self-loop, false→6e), so no new block could join switch 6b's
  CFG without editing 6d — not a fixture target in v1.
- The frozen edges further locked 6c:Bool (CondBranch condition) and the
  switch result:Bool (6e returns 6c); JobState-typed arms could not
  validate. Empty trap blocks would satisfy the count vacuously and
  were refused as gaming.
- Delivered under v1: JobState typedef (Queued/Running/Succeeded unit
  + Failed(SInt)) validates; status genuinely migrates to Failed(7)
  (explicit code, deterministic); neg → ORACLE_BOOL_COMPAT_FIELD.
- Owner: benchmark fixture design (emit table targets).

Repair implemented 2026-09-20 on `work/succession-sley20-arm` only:
`succ_live_emit.rs::base_type` targets now include 6d/6e
(`switch_entry`/`switch_leaf` roles); `base.pack` bytes unchanged
(same `pack_digest_blake3`); `succ_live_packs_frozen` passes; frozen
corpus v1 unchanged. Full migration proved end to end
(`trial_type_full.log` ACCEPTED; BOOL_COMPAT negative;
typedef-only and trap refuse; payload-loss refuses in production
validation). Oracle corrected 2026-09-21 to three-tier provenance
(corpus / fixture / retired witness choices; packet §6):
alternatives Failed(8), Queued status, and shared leaf constants
accept; `ORACLE_FAILED_CODE` kept as backstop. Fixture-design review
retained before any main adoption. Not a production semantic change.

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
TYPE corrected closure lives on the work branch with fixture-design
review retained for main adoption; and all
live-model campaign prerequisites (seeds/budgets preregistered,
90 attempts minimum, two-host reproducible package/dossier, current
source-bound Council transcripts) remain unsatisfied. No campaign
started. `ga_claimed=false`.
