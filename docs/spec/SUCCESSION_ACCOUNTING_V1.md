# Succession Accounting v1

Status: S20-630 contract draft, revision 4 (2026-09-14), with the live-input section 11 of revision 5 (2026-09-24); the Ariadne
contract review (2 P0, 8 P1), Nabu architecture review (4 P0, 7 P1), and
Vulcan surface review (1 P0, 5 P1) all returned FAIL against revision 2,
and every P0 and every P1 lands in revision 3. Revision 4 exercises the
failing-threshold path end to end through `derive_report` and writes down
the residual P2/P3 precision (empty-chain collapse, `ARM_UNKNOWN`
reachability, ratio legibility, smoke-checker boundary). The
implementation is `bench/accounting/report.py`; implementation state is
tracked in the machine summary.

## Boundary

S20-630 freezes how Accepted Change Tokens, context, repair, precision, and
correctness accounting are derived from the immutable evidence of one
benchmark run: the S20-610 run manifest and the per-arm digest-chained
trial claims (`sley2.raw-trial-digest-claim.v1` under `raw_files`,
`sley2.sley2-trial-digest-claim.v1` under `sley_2_0`, and the legacy arm's
chain when S20-600 produces one). It reads nothing else: no trace body, no
model, no provider, no oracle, no clock. It performs exact integer and
rational arithmetic only, keeps every attempted trial in every denominator,
and evaluates the section 22 conditions the plan encodes as recorded in
`bench/benchmark-plan.json` (`thresholds`). The three section 22
conditions the plan does not encode are not evaluated and are carried as
explicit `NOT_EVALUATED` rows (section 4), so the omission is a field,
never a gap. It makes no succession claim: with zero trials every arm is
`NO_CLAIM_CHAIN` and every threshold is `UNDETERMINED` (master goal
sections 21.4 through 21.6, 22.1 through 22.4; dossiers 16 through 18).

## 1. Inputs

- the run directory's create-once manifest, verified exactly as S20-610
  verifies it;
- each required arm's claim chain, verified by that arm's own runner
  through the registry `ARM_VERIFIERS` (`raw_files` by
  `bench.raw.runner.verify_digest_claim_directory`, `sley_2_0` by
  `bench.sley2.runner.verify_trial_claims`); a chain the arm's verifier
  rejects is `ACCOUNTING_CHAIN_INVALID` and the whole report fails closed;
- the plan's metric names, thresholds, required arms, and arm
  `fixture_status` values, and the corpus' task identifiers and classes.

An arm whose chain is absent is reported as `NO_CLAIM_CHAIN`; an arm whose
chain does not cover the manifest's full task and seed product is
`PARTIAL`; a covered arm is `COMPLETE`. A verifier-accepted empty chain
carries no head digest and no trial product, so accounting cannot form
denominators from it: it reads as `NO_CLAIM_CHAIN` by construction, and
the collapse is deliberate and fail-closed in the direction that matters
(a chain the verifier rejects is `ACCOUNTING_CHAIN_INVALID`, never
absence). A claim loaded under a required arm that names a different arm
is `ACCOUNTING_ARM_UNKNOWN`: the code guards a claim filed under the
wrong required arm, while a chain for a non-required arm never reaches
accounting at all, because discovery iterates required arms only
(section 8). Chain
discovery covers required arms only; chains for non-required plan arms
(such as `zerolang`) are not discovered until a verifier is registered
for them (section 8). The legacy arm has no verifier until S20-600
supplies a chain producer, and the registry records that fact as data
rather than code: a legacy chain that appears at the reserved path
`legacy/claims.jsonl` before its verifier is registered is
`ACCOUNTING_CHAIN_INVALID`, never silent absence. Each loaded arm records
the verifier used (`chain_verifier`) and its plan `fixture_status`, and
the report records every required arm's `fixture_status`, so a threshold
row lifted out of the report still carries its provenance.

## 2. Arithmetic

Every quantity is an integer or an exact ratio `{ "numerator": integer,
"denominator": integer }` reduced by the greatest common divisor with a
positive denominator. Floats never appear in inputs, intermediate values, or
outputs (`ACCOUNTING_FLOAT_FORBIDDEN`). A median over an even count is the
exact ratio of the two middle values' sum to two. Every ratio already
travels as an exact integer numerator/denominator pair, which subsumes
basis-point legibility without rounding: the report carries no second
rounded representation beside the exact pair, so no rounded figure can be
mistaken for the comparison basis. A ratio whose denominator
would be zero is `null` with a named reason, never an error and never a
substituted value. Dossier display may render a ratio as a decimal string
by explicit formatting of the exact numerator and denominator; that
rendering is display-only and never enters a comparison.

## 3. Arm accounting

```text
ArmAccounting {
  "status": "COMPLETE" | "PARTIAL",
  "claims": integer, "chain_head_digest": hex[64],
  "chain_verifier": string, "fixture_status": string,
  "attempted": integer,            // every claim, whatever its status
  "accepted": integer, "rejected": integer, "timeouts": integer,
  "harness_failures": integer,
  "strict_correctness": Ratio,     // accepted / attempted
  "total_observable_tokens": integer, "model_output_tokens": integer,
  "accepted_change_tokens": Ratio | null,   // total_observable_tokens / accepted
  "accepted_change_tokens_reason": "no_accepted_change" | null,
  "context_bytes": Median3, "model_input_tokens": Median3,
  "repair_loops": Median3, "wall_time": Median3,
  "peak_memory": Median3, "execution_latency": Median3,
  "tool_calls": integer, "compile_or_check_attempts": integer,
  "invalid_candidates": integer, "invalid_committed_states": integer,
  "stale_candidates": integer, "stale_candidates_incorrectly_accepted": integer,
  "collateral_semantic_changes": integer, "human_interventions": integer,
  "entities_inspected": integer, "relationships_inspected": integer,
  "files_inspected": integer, "canonical_storage_bytes": integer,
  "pack_bytes": integer,
  "median_excluded_harness_failures": integer,
  "by_task": { task_id: { "attempted": integer, "accepted": integer } },
  "by_seed": { seed: { "attempted": integer, "accepted": integer,
                       "strict_correctness": Ratio } },
  "by_class": { class: { "attempted": integer, "accepted": integer,
                         "strict_correctness": Ratio,
                         "collateral_semantic_changes": integer } },
  "claim_statuses": { "evidence_status": [string],
                      "oracle_verification_status": [string],
                      "accounting_verification_status": [string] },
  "every_attempt_in_denominator": true
}
// Median3 = { "sum": integer, "median": Ratio | null,
//             "median_non_harness_failure": Ratio | null }
```

`accepted_change_tokens` follows master goal section 21.5 exactly: total
observable model tokens over accepted correct changes, `null` with reason
`no_accepted_change` when no change was accepted. A lower ACT with lower
correctness is not a win, and the report never ranks arms by ACT alone.
The claim-level plan metric of the same name must be null on every claim:
both runners refuse a pre-derived ACT at the claim layer, and accounting
restates that refusal with `ACCOUNTING_METRIC_INVALID`, so the two
meanings of the name can never mix.

Every median is taken over every attempt, including timeouts and harness
failures, and every threshold names that basis (`ALL_ATTEMPTS`). This is
deliberate: section 21.6 keeps every failed trial in the denominator, and
one rule governs denominators and efficiency medians alike rather than a
per-metric population choice that could be tuned per comparison. Each
median travels with its companion over non-harness-failure attempts and
the excluded count, so the sensitivity is visible beside the verdict. A
duplicated trial id or a duplicated (task, seed) pair fails the report
with `ACCOUNTING_INTERNAL_INVARIANT`: set-equality `COMPLETE` is blind to
multiplicity, so the boundary restates one-claim-per-slot locally rather
than inheriting divergent producer guarantees. `every_attempt_in_
denominator` is derived, not asserted: the status partition, every median
series, and every grouping must account for exactly the claims seen, else
`ACCOUNTING_INTERNAL_INVARIANT`.

## 4. Thresholds

Each plan threshold is evaluated between `sley_2_0` and `sley_1_2_0` only
when both arms are `COMPLETE`; otherwise it is `UNDETERMINED` with the
arms' statuses as its reason. The plan names no threshold over
`raw_files`, so `raw_files` is never compared; it still gates report
status (section 5). A `PASS` or `FAIL` carries the exact ratios it
compared and the evidence status it inherits. The plan's ten threshold
keys, in full, are evaluated as:

- `critical_class_correctness_not_lower_than_legacy`: the plan flag must
  read `true`, else `ACCOUNTING_METRIC_INVALID`; until the corpus names
  critical classes every class counts as critical, a deliberate
  strengthening recorded as `critical_class_selection: ALL_CLASSES`, so a
  `FAIL` is attributable to the classes named in the facts;
- `failure_rate_relative_reduction_percent_or_equal_correctness_act_
  reduction_percent`: failure rate is one minus strict correctness; at
  least 20 percent relative failure reduction, or equal strict
  correctness ratios with at least 30 percent ACT reduction; a perfect
  legacy arm leaves the reduction undefined with reason
  `legacy_failure_rate_zero`, never a pass;
- `median_context_bytes_reduction_percent_or_model_input_tokens_
  reduction_percent`: each leg passes only on its own measured reduction
  (40 percent context, 30 percent input tokens) with the other metric's
  regression inside the cap; a leg with an undefined reduction or an
  undefined other-metric regression is `UNDETERMINED`, never compliant;
  the row passes when either leg passes, reads `UNDETERMINED` when no leg
  passes and some leg is undefined, and fails otherwise;
- `other_context_metric_max_regression_percent`: the measured regressions
  of both context metrics against the 10 percent cap, computed
  independently of the threshold above; either regression undefined
  leaves the row `UNDETERMINED` with the missing metric named;
- `median_repair_loop_reduction_percent_or_accepted_changes_increase_
  percent`: 25 percent median repair reduction or 20 percent accepted
  increase, where the increase is `(accepted_sley2 - accepted_legacy) /
  accepted_legacy`, null with reason `legacy_accepted_zero` when the
  legacy arm accepted nothing; counts compare directly only under the
  manifest's single shared `action_budget`, which the row carries as a
  fact and which must be one integer, while any other budget-named
  manifest field fails the report with `ACCOUNTING_RUN_INVALID`;
- `invalid_candidates_leave_state_unchanged_percent` and
  `stale_preconditions_rejected_percent`: the exact good ratio against
  the plan value (100); a zero denominator is `UNDETERMINED` with reason
  `no_invalid_candidates` / `no_stale_candidates` and the denominator in
  the facts, never a vacuous pass;
- `invalid_committed_states`, `stale_conflicting_candidates_accepted`,
  and `human_interventions_sley2`: exact equality with the plan value
  (all zero).

The report must contain exactly the plan's threshold names plus the three
rows below, else `ACCOUNTING_METRIC_INVALID`. The section 22 conditions
the plan does not encode are carried as `NOT_EVALUATED` rows with the
condition and its owning package, and no all-`PASS` report may be read as
section 22 satisfied:

- `section_22_1_no_required_check_bypassed` (owner `S20-360`): no
  required effect or capability check is bypassed;
- `section_22_4_collateral_semantic_comparison` (owner `UNASSIGNED`):
  collateral semantic changes no worse than Sley 1.2.0 and strictly
  lower in at least one multi-entity task class; per-class collateral
  sums are carried above so the future comparison has its inputs;
- `section_22_4_mutation_reconstructability` (owner `UNASSIGNED`):
  every accepted mutation reconstructable from its base state root and
  canonical transaction receipt.

## 5. Report

```text
AccountingReport {
  "contract": "sley2.succession-accounting-report.v1",
  "run_id": string, "run_manifest_digest": hex[64],
  "benchmark_plan_digest": hex[64], "corpus_digest": hex[64],
  "corpus_version": integer, "required_arms": [arm_id],
  "arm_fixture_status": { arm_id: string },
  "arms": { arm_id: ArmAccounting | "NO_CLAIM_CHAIN" },
  "claim_statuses": { "evidence_status": [string],
                      "oracle_verification_status": [string],
                      "accounting_verification_status": [string] },
  "thresholds": { name: { "result": "PASS" | "FAIL" | "UNDETERMINED" | "NOT_EVALUATED", ... } },
  "status": "NO_TRIALS" | "PARTIAL" | "COMPLETE",
  "evidence_status": string,     // derived, see below
  "report_digest": hex[64]        // SHA256("sley2.succession-accounting-report.v1\0" || canonical report without this field)
}
```

The report is canonical JSON. `evidence_status` is derived from the
claims' own statuses: `"evidence_status": "DERIVED_FROM_UNVERIFIED_CLAIMS"`
when every observed claim status starts with `UNVERIFIED`
(`DERIVED_FROM_VERIFIED_CLAIMS` when every observed status starts with
`VERIFIED`, `DERIVED_FROM_MIXED_CLAIM_STATUSES` otherwise), with the
distinct observed statuses recorded in `claim_statuses`; vacuous over
zero claims. `verify_report` re-derives the recorded status from the
recorded claim statuses instead of checking a module constant, so
already-written reports keep verifying after any future vocabulary
change. The dossier's succession fields (`machine-summary.json`
`succession`) stay `null` until a report with status `COMPLETE` over
verified claims exists. Report status is `COMPLETE` only when every
required arm is complete, including `raw_files`, even though thresholds
compare legacy and Sley 2 only. Every threshold row carries the report's
evidence status, so a row lifted out of the report is still marked as
derived, and every arm carries the plan's `fixture_status`, so a report
over scripted claims is distinguishable from one over real trials. The
`derive` command prints `DERIVED` for a successful derivation, never
`PASS`: threshold verdicts live on the rows, not in the log.

## 6. Stable failures

| Numeric | Symbolic code |
|---:|---|
| 63000 | `ACCOUNTING_RUN_INVALID` |
| 63001 | `ACCOUNTING_CHAIN_INVALID` |
| 63002 | `ACCOUNTING_ARM_UNKNOWN` |
| 63003 | `ACCOUNTING_METRIC_INVALID` |
| 63004 | `ACCOUNTING_FLOAT_FORBIDDEN` |
| 63005 | `ACCOUNTING_INCOMPLETE` |
| 63006 | `ACCOUNTING_REPORT_INVALID` |
| 63007 | `ACCOUNTING_INTERNAL_INVARIANT` |

`ACCOUNTING_INCOMPLETE` is raised only when a caller demands a complete
report (`--require-complete`) over partial chains.

## 7. Required evidence

- offline tests: exact ratios and medians (odd and even counts, zero
  denominators), every-attempt denominators with timeouts and harness
  failures plus the non-harness-failure companions, per-class
  correctness with collateral sums, per-seed grouping, threshold
  evaluation on real `arm_accounting` outputs over synthetic complete
  chains (both PASS and FAIL cases), a complete report through
  `derive_report` over two full chains with a passing threshold row, a
  failing threshold row through `derive_report` over two full chains
  (the failure-rate row on a Sley 2 arm worse than legacy), `UNDETERMINED` on absent and
  partial arms and on undefined regressions and empty denominators,
  `NOT_EVALUATED` rows, threshold-coverage and criticality-flag failure
  closure, duplicate-slot and pre-derived-ACT refusal, the legacy
  reserved-path gate, evidence-status derivation and mismatch refusal, a
  tampered chain failing closed, and the report digest;
- `scripts/check_succession_accounting.py` in `make quick`;
- a report over the S20-620 smoke run directory (`make accounting-smoke`),
  reading `PARTIAL` with two scripted attempts, no accepted change, and
  every evaluated threshold `UNDETERMINED`, regenerable at any time (the
  runtime directory is not tracked; the closeout records the smoke
  report's digest);
- Tier 1 plus Tier 2 validation, and the Ariadne, Nabu, and Vulcan reviews
  with every report-grade finding closed.

## 8. Explicit exclusions

This contract does not claim: any trial; model, provider, or oracle
execution; artifact or provenance verification; statistics beyond exact
sums, ratios, and medians (S20-640, which reads the per-seed grouping
rather than re-verifying chains); the legacy arm's claim chain (S20-600,
which must either use the reserved path or revise this contract);
discovery of non-required arm chains; a documented-reason override for
the section 22.2 cap (a reasoned exception needs a plan revision);
per-arm action budgets; corpus-declared critical classes (the corpus is
S20-610's frozen input; the plan flag is the control accounting owns);
publication; runtime, packaging, release, or GA.

## 9. Revision 3 changes

Revision 2 bullets 4 and 5 ("the cap row mirrors the threshold result",
"the legacy arm is always `NO_CLAIM_CHAIN`") canonized behavior the
Council reviews proved wrong and are superseded below; the remaining
revision 2 clarifications stand.

- The boundary evaluates the section 22 conditions the plan encodes;
  the three it does not encode are `NOT_EVALUATED` rows with owners.
- The regression cap is measured on both context metrics; undefined
  means `UNDETERMINED`, and threshold legs carry the same rule.
- The legacy arm loads through the verifier registry with its verifier
  and fixture recorded; a premature chain at the reserved path fails
  closed; the plan's non-`PENDING` fixture no longer means absence.
- `ArmAccounting` carries the seven section 21.4 metrics the claims
  already had (with medians for latency and peak memory), per-class
  collateral sums, per-seed grouping, claim statuses, a derived
  every-attempt flag, and duplicate-slot refusal.
- The evidence status is derived from the claims and re-derived on
  verify; every row carries it; every arm carries its fixture status.
- Medians stay every-attempt by stated rule with non-harness-failure
  companions; percent rows compute exact ratios with named-null empty
  denominators; the shared action budget is read and recorded; failure
  rate is one minus strict correctness with the perfect-legacy null
  named; criticality is read from the plan flag with all-classes
  recorded; threshold coverage is asserted structurally.
- The smoke report is regenerable runtime evidence with its digest in
  the closeout; `derive` prints `DERIVED`.

## 10. Revision 4 changes

- The failing-threshold path runs end to end through `derive_report`
  (section 7): a worse-than-legacy Sley 2 arm reaches `COMPLETE` with
  the failure-rate row `FAIL` beside a passing cap row.
- The verifier-empty-chain collapse is named deliberate with its
  fail-closed justification (section 1).
- `ACCOUNTING_ARM_UNKNOWN` reachability is stated: it guards claims
  filed under the wrong required arm; non-required chains never reach
  discovery (section 1).
- Ratio legibility is stated: exact integer pairs subsume basis points;
  no rounded second representation enters the report (section 2).
- The stage checker pins the recorded smoke digest rather than reading
  the gitignored runtime artifact, so it passes on clean clones without
  runtime evidence; regeneration reproduces the digest (section 7).

## 11. Live input (revision 5, 2026-09-24)

The S20-640 live campaign writes one `sley2.live-campaign-manifest.v1`
run manifest and one append-only `attempts.jsonl` (contract
`sley2.live-attempt.v1`) per model tier, all three arms together.
`bench/accounting/live.py derive_live_report` is the live entry point. It
changes no arithmetic, denominator, median, or threshold rule of sections 2
through 5: it replaces only the input verifiers.

- Verifier registry: `bench.live.live_claims.LIVE_ARM_VERIFIERS` holds one
  verifier per required arm, including the legacy arm
  (`verify_live_legacy_claims`). Each runs `verify_attempts` (chain, every
  artifact resolved by digest, environment receipt, provider-usage and
  oracle reconciliation of judged attempts, sley_2_0 completion binding),
  reconciles the usage of unjudged attempts too (a completed provider
  stream must equal the recorded metrics; a stream without a terminal usage
  record must claim zero observable tokens and tool items), and binds every
  attempt's `workspace_before` to the frozen initial state the arm stages
  for its task plus the manifest's arm fixture digest to the recomputed
  one. The legacy verifier also requires the plan's pinned 1.2.0
  `artifact_sha256` and `commit` to equal the legacy runner's frozen
  contract. A rejected verification is `ACCOUNTING_CHAIN_INVALID`.
- Claims: one per attempt with `trial_id` = attempt id and the attempt's
  status and 25 metrics unchanged. Timeouts, harness failures, and
  attempts retained over budget (`LIVE_PROVIDER_BUDGET_EXCEEDED`, which
  keep their observed usage) are claims like any other and sit in every
  denominator and every `ALL_ATTEMPTS` median. Status fields:
  `evidence_status` `VERIFIED_LIVE_EVIDENCE`;
  `oracle_verification_status` `VERIFIED_ORACLE_STDOUT_AND_REPORT_RECONCILED`
  (judged) or `VERIFIED_NO_ORACLE_VERDICT`; `accounting_verification_status`
  `VERIFIED_PROVIDER_USAGE_RECONCILED` or
  `VERIFIED_NO_PROVIDER_USAGE_REPORTED`.
- Label: the report records the manifest label and
  `counts_toward_succession` (true only for `CAMPAIGN`);
  `--require-campaign` refuses a `PILOT` run.
- The three section 22 rows of section 4 are evaluated from claims
  (`evaluate_section_22_rows`) instead of being carried `NOT_EVALUATED`;
  the offline path keeps them `NOT_EVALUATED`. No threshold is added:
  - `section_22_1_no_required_check_bypassed` and
    `section_22_4_mutation_reconstructability` read each accepted Sley 2
    claim's `section_22` value (`required_check_bypassed`,
    `mutation_reconstructable`). A measured violation is `FAIL`; any null
    is `UNDETERMINED` (`not_measured`); all measured compliant is `PASS`;
    no accepted mutation is `UNDETERMINED` (`no_accepted_mutation`), never
    a vacuous pass. The live oracle report carries both fields, but the
    runner writes them as constants (`bench/live/campaign.py`
    `_oracle_report`), so the live verifier emits null with basis
    `RUNNER_CONSTANT_NOT_A_MEASUREMENT`: at this revision both rows read
    `UNDETERMINED` for any live run. Measuring them needs the judge to
    record, per accepted mutation, the production commit's validation
    phase outcomes (phases 8 effect closure and 9 capability) and the
    base root, receipt, and result root with an independent replay. That
    is an oracle revision (new oracle digest, a preregistration amendment
    before counted attempts) and is not made here.
  - `section_22_4_collateral_semantic_comparison` compares the arms'
    collateral totals ("no worse than Sley 1.2.0") and per-class sums
    ("strictly lower in at least one multi-entity task class"). The frozen
    corpus names no multi-entity classes, so the strict leg is decided
    only where no selection could change it: worse in total is `FAIL`
    (`no_worse`); strictly lower in no class is `FAIL`
    (`strictly_lower_in_no_class`); otherwise `UNDETERMINED`
    (`multi_entity_class_selection_not_frozen`) with the classes that are
    strictly lower listed. Both arms must be `COMPLETE`.
- Tests: `bench/accounting/tests/test_live.py` over synthetic, test-only
  live-shaped runs (`bench/accounting/tests/synthetic_live.py`, run ids
  `synthetic-fixture-*`, temporary directories only): every plan row
  `PASS` and `FAIL`, every section 22 row outcome, every denominator rule,
  partial arms, forged unjudged usage, start-state and legacy artifact
  binding, the PILOT label; and the retained 2026-09-24 PILOT run read-only
  (nine harness failures, `counts_toward_succession` false).
