# Succession Accounting v1

Status: S20-630 contract draft, revision 2 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 records the clarifications found while implementing
revision 1 (section 9). The implementation is `bench/accounting/report.py`;
implementation state is tracked in the machine summary.

## Boundary

S20-630 freezes how Accepted Change Tokens, context, repair, precision, and
correctness accounting are derived from the immutable evidence of one
benchmark run: the S20-610 run manifest and the per-arm digest-chained
trial claims (`sley2.raw-trial-digest-claim.v1` under `raw_files`,
`sley2.sley2-trial-digest-claim.v1` under `sley_2_0`, and the legacy arm's
chain when S20-600 produces one). It reads nothing else: no trace body, no
model, no provider, no oracle, no clock. It performs exact integer and
rational arithmetic only, keeps every attempted trial in every denominator,
and evaluates the master goal's section 22 thresholds as recorded in
`bench/benchmark-plan.json` (`thresholds`). It makes no succession claim:
with zero trials every arm is `NO_CLAIM_CHAIN` and every threshold is
`UNDETERMINED` (master goal sections 21.4 through 21.6, 22.1 through 22.4;
dossiers 16 through 18).

## 1. Inputs

- the run directory's create-once manifest, verified exactly as S20-610
  verifies it;
- each arm's claim chain, verified by that arm's own runner
  (`bench.raw.runner.verify_digest_claim_directory`,
  `bench.sley2.runner.verify_trial_claims`); a chain the arm's verifier
  rejects is `ACCOUNTING_CHAIN_INVALID` and the whole report fails closed;
- the plan's metric names, thresholds, and required arms, and the corpus'
  task identifiers and classes.

An arm whose chain is absent is reported as `NO_CLAIM_CHAIN`; an arm whose
chain does not cover the manifest's full task and seed product is
`PARTIAL`; a covered arm is `COMPLETE`. A claim naming an arm the plan does
not require is `ACCOUNTING_ARM_UNKNOWN`.

## 2. Arithmetic

Every quantity is an integer or an exact ratio `{ "numerator": integer,
"denominator": integer }` reduced by the greatest common divisor with a
positive denominator. Floats never appear in inputs, intermediate values, or
outputs (`ACCOUNTING_FLOAT_FORBIDDEN`). A median over an even count is the
exact ratio of the two middle values' sum to two. A ratio whose denominator
would be zero is `null` with a named reason, never an error and never a
substituted value.

## 3. Arm accounting

```text
ArmAccounting {
  "status": "COMPLETE" | "PARTIAL",
  "claims": integer, "chain_head_digest": hex[64],
  "attempted": integer,            // every claim, whatever its status
  "accepted": integer, "rejected": integer, "timeouts": integer,
  "harness_failures": integer,
  "strict_correctness": Ratio,     // accepted / attempted
  "total_observable_tokens": integer, "model_input_tokens": integer,
  "model_output_tokens": integer,
  "accepted_change_tokens": Ratio | null,   // total_observable_tokens / accepted
  "context_bytes": { "sum": integer, "median": Ratio | null },
  "model_input_tokens_median": Ratio | null,
  "repair_loops": { "sum": integer, "median": Ratio | null },
  "tool_calls": integer, "compile_or_check_attempts": integer,
  "invalid_candidates": integer, "invalid_committed_states": integer,
  "stale_candidates": integer, "stale_candidates_incorrectly_accepted": integer,
  "collateral_semantic_changes": integer, "human_interventions": integer,
  "wall_time": { "sum": integer, "median": Ratio | null },
  "by_task": { task_id: { "attempted": integer, "accepted": integer } },
  "by_class": { class: { "attempted": integer, "accepted": integer,
                         "strict_correctness": Ratio } }
}
```

`accepted_change_tokens` follows master goal section 21.5 exactly: total
observable model tokens over accepted correct changes, `null` with reason
`no_accepted_change` when no change was accepted. A lower ACT with lower
correctness is not a win, and the report never ranks arms by ACT alone.

## 4. Thresholds

Each plan threshold is evaluated between `sley_2_0` and `sley_1_2_0` (and
`raw_files` where the plan names it) only when both arms are `COMPLETE`;
otherwise it is `UNDETERMINED` with the arms' statuses as its reason. A
`PASS` or `FAIL` carries the exact ratios it compared. The evaluations are:

- correctness not lower in any task class (all classes are treated as
  critical);
- failure-rate relative reduction of at least 20 percent, or equal accepted
  correctness with an ACT reduction of at least 30 percent;
- median context bytes reduced by at least 40 percent, or median model
  input tokens reduced by at least 30 percent, with the other metric
  regressing by at most 10 percent;
- median repair loops reduced by at least 25 percent, or accepted correct
  changes increased by at least 20 percent under the same action budget;
- `invalid_committed_states`, `stale_candidates_incorrectly_accepted`, and
  the Sley 2 arm's `human_interventions` all zero, and every invalid
  candidate leaving accepted state unchanged.

## 5. Report

```text
AccountingReport {
  "contract": "sley2.succession-accounting-report.v1",
  "run_id": string, "run_manifest_digest": hex[64],
  "corpus_version": integer, "required_arms": [arm_id],
  "arms": { arm_id: ArmAccounting | "NO_CLAIM_CHAIN" },
  "thresholds": { name: { "result": "PASS" | "FAIL" | "UNDETERMINED", ... } },
  "status": "NO_TRIALS" | "PARTIAL" | "COMPLETE",
  "every_attempt_in_denominator": true,
  "evidence_status": "DERIVED_FROM_UNVERIFIED_CLAIMS",
  "report_digest": hex[64]        // SHA256("sley2.succession-accounting-report.v1\0" || canonical report without this field)
}
```

The report is canonical JSON. `evidence_status` is fixed: the claims it
reads are explicitly unverified (S20-610, S20-620), so the report inherits
that status and becomes evidence only after the operator-approved artifact
and provenance verification those contracts name. The dossier's succession
fields (`machine-summary.json` `succession`) stay `null` until a report
with status `COMPLETE` over verified claims exists.

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
  failures, per-class correctness, threshold evaluation on synthetic
  complete chains built through the S20-610 and S20-620 append functions
  (both PASS and FAIL cases), `UNDETERMINED` on absent and partial arms, a
  tampered chain failing closed, and the report digest;
- `scripts/check_succession_accounting.py` in `make quick`;
- a report over the S20-620 smoke run directory (`make accounting-smoke`),
  reading `PARTIAL` with two scripted attempts, no accepted change, and
  every threshold `UNDETERMINED`, retained as runtime evidence;
- Tier 1 plus Tier 2 validation, and the Ariadne, Nabu, and Vulcan reviews
  with every report-grade finding closed.

## 8. Explicit exclusions

This contract does not claim: any trial; model, provider, or oracle
execution; artifact or provenance verification; statistics beyond exact
sums, ratios, and medians (S20-640); the legacy arm's claim chain (S20-600);
publication; runtime, packaging, release, or GA.

## 9. Revision 2 clarifications

- `ArmAccounting` carries `accepted_change_tokens_reason`
  (`no_accepted_change` or null) beside the nullable ratio, so a null is
  never silent.
- The S20-620 smoke run holds two scripted claims (one rejected, one
  harness failure), so the report over it is `PARTIAL`, not `NO_TRIALS`;
  `NO_TRIALS` names a run whose every arm has no chain.
- "Accepted correct changes increased by at least 20 percent" is
  `(accepted_sley2 - accepted_legacy) / accepted_legacy`, null when the
  legacy arm accepted nothing; the shared action budget makes counts
  comparable directly.
- `other_context_metric_max_regression_percent` reports the same result as
  the context threshold it caps, with the cap as its fact.
- Each arm's chain is loaded through that arm's runner (`raw_files` by
  `bench.raw.runner`, `sley_2_0` by `bench.sley2.runner`); the legacy arm
  has no chain producer yet and is always `NO_CLAIM_CHAIN`.
- The runner's `Fraction` arithmetic is exact rational arithmetic over
  integers; no float is constructed anywhere.
