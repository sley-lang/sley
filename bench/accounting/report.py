#!/usr/bin/env python3
"""S20-630 succession accounting: exact derivation from immutable claims.

The report reads one run's create-once manifest and each arm's claim chain,
verified by that arm's own runner, and nothing else. It keeps every attempt
in every denominator, uses integers and reduced ratios only, evaluates the
section 22 conditions the plan encodes only between complete arms, names the
section 22 conditions the plan does not encode as NOT_EVALUATED, derives its
evidence status from the claims' own statuses, and inherits no other status.
It executes nothing and reads no clock.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from enum import IntEnum
from fractions import Fraction
from pathlib import Path
from typing import Any, Callable, Mapping

from bench.raw.runner import (
    RawRunnerError,
    _expected_trial_pairs,
    _plan_and_corpus,
    _read_manifest_exact,
    canonical_json_bytes,
    manifest_digest,
    verify_digest_claim_directory,
)
from bench.sley2.runner import Sley2RunnerError, verify_trial_claims

ROOT = Path(__file__).resolve().parents[2]
REPORT_CONTRACT = "sley2.succession-accounting-report.v1"
REPORT_DOMAIN = REPORT_CONTRACT.encode("utf-8") + b"\0"
EVIDENCE_UNVERIFIED = "DERIVED_FROM_UNVERIFIED_CLAIMS"
EVIDENCE_VERIFIED = "DERIVED_FROM_VERIFIED_CLAIMS"
EVIDENCE_MIXED = "DERIVED_FROM_MIXED_CLAIM_STATUSES"
RAW_ARM = "raw_files"
LEGACY_ARM = "sley_1_2_0"
SLEY2_ARM = "sley_2_0"
NO_CHAIN = "NO_CLAIM_CHAIN"
STATUSES = frozenset({"accepted", "rejected", "timeout", "harness_failure"})
STATUS_FIELDS = ("evidence_status", "oracle_verification_status", "accounting_verification_status")
SUM_METRICS = (
    "total_observable_tokens",
    "model_input_tokens",
    "model_output_tokens",
    "tool_calls",
    "compile_or_check_attempts",
    "invalid_candidates",
    "invalid_committed_states",
    "stale_candidates",
    "stale_candidates_incorrectly_accepted",
    "collateral_semantic_changes",
    "human_interventions",
    "entities_inspected",
    "relationships_inspected",
    "files_inspected",
    "peak_memory",
    "canonical_storage_bytes",
    "pack_bytes",
    "execution_latency",
)
MEDIAN_METRICS = ("context_bytes", "model_input_tokens", "repair_loops", "wall_time", "peak_memory", "execution_latency")
MEDIAN_BASIS = "ALL_ATTEMPTS"
KNOWN_BUDGET_FIELDS = frozenset({"context_budget", "action_budget", "wall_time_budget"})
# Section 22 conditions the plan does not encode: named here so the omission
# is a row, never a gap. Each names its master-goal condition and the work
# package that owns evaluating it, or UNASSIGNED when none does.
NOT_EVALUATED_CONDITIONS = (
    ("section_22_1_no_required_check_bypassed", "MASTER_GOAL_22_1_NO_REQUIRED_EFFECT_OR_CAPABILITY_CHECK_BYPASSED", "S20-360"),
    ("section_22_4_collateral_semantic_comparison", "MASTER_GOAL_22_4_COLLATERAL_NO_WORSE_THAN_1_2_0_STRICTLY_LOWER_IN_ONE_MULTI_ENTITY_CLASS", "UNASSIGNED"),
    ("section_22_4_mutation_reconstructability", "MASTER_GOAL_22_4_EVERY_ACCEPTED_MUTATION_RECONSTRUCTABLE_FROM_BASE_ROOT_AND_RECEIPT", "UNASSIGNED"),
)
NOT_EVALUATED_NAMES = frozenset(name for name, _, _ in NOT_EVALUATED_CONDITIONS)


class AccountingErrorCode(IntEnum):
    RUN_INVALID = 63_000
    CHAIN_INVALID = 63_001
    ARM_UNKNOWN = 63_002
    METRIC_INVALID = 63_003
    FLOAT_FORBIDDEN = 63_004
    INCOMPLETE = 63_005
    REPORT_INVALID = 63_006
    INTERNAL_INVARIANT = 63_007


SYMBOLS = {
    AccountingErrorCode.RUN_INVALID: "ACCOUNTING_RUN_INVALID",
    AccountingErrorCode.CHAIN_INVALID: "ACCOUNTING_CHAIN_INVALID",
    AccountingErrorCode.ARM_UNKNOWN: "ACCOUNTING_ARM_UNKNOWN",
    AccountingErrorCode.METRIC_INVALID: "ACCOUNTING_METRIC_INVALID",
    AccountingErrorCode.FLOAT_FORBIDDEN: "ACCOUNTING_FLOAT_FORBIDDEN",
    AccountingErrorCode.INCOMPLETE: "ACCOUNTING_INCOMPLETE",
    AccountingErrorCode.REPORT_INVALID: "ACCOUNTING_REPORT_INVALID",
    AccountingErrorCode.INTERNAL_INVARIANT: "ACCOUNTING_INTERNAL_INVARIANT",
}


class AccountingError(ValueError):
    def __init__(self, code: AccountingErrorCode, detail: str = ""):
        super().__init__(f"{SYMBOLS[code]}:{detail}" if detail else SYMBOLS[code])
        self.code = code
        self.detail = detail

    @property
    def symbol(self) -> str:
        return SYMBOLS[self.code]


def _fail(code: AccountingErrorCode, detail: str = "") -> None:
    raise AccountingError(code, detail)


# ---------------------------------------------------------------------------
# Exact arithmetic
# ---------------------------------------------------------------------------


def _require_int(value: Any, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        if isinstance(value, float):
            _fail(AccountingErrorCode.FLOAT_FORBIDDEN, name)
        _fail(AccountingErrorCode.METRIC_INVALID, name)
    return value


def ratio(numerator: int, denominator: int) -> dict[str, int] | None:
    """A reduced exact ratio, or `None` when the denominator is zero."""

    _require_int(numerator, "numerator")
    _require_int(denominator, "denominator")
    if denominator == 0:
        return None
    value = Fraction(numerator, denominator)
    return {"denominator": value.denominator, "numerator": value.numerator}


def as_fraction(value: Mapping[str, int] | None) -> Fraction | None:
    if value is None:
        return None
    return Fraction(_require_int(value["numerator"], "numerator"), _require_int(value["denominator"], "denominator"))


def from_fraction(value: Fraction | None) -> dict[str, int] | None:
    if value is None:
        return None
    return {"denominator": value.denominator, "numerator": value.numerator}


def median(values: list[int]) -> dict[str, int] | None:
    """The exact median: the middle value, or the two middle values' mean."""

    ordered = sorted(_require_int(value, "median") for value in values)
    if not ordered:
        return None
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ratio(ordered[middle], 1)
    return ratio(ordered[middle - 1] + ordered[middle], 2)


def _percent(value: Any) -> Fraction:
    return Fraction(_require_int(value, "threshold"), 100)


def _require_status_string(claim: Mapping[str, Any], name: str) -> str:
    value = claim.get(name)
    if not isinstance(value, str) or not value:
        _fail(AccountingErrorCode.METRIC_INVALID, name)
    return value


def derive_evidence_status(statuses: Mapping[str, list[str]]) -> str:
    """The report's evidence status, derived from the claims' own statuses.

    Every observed status starting with UNVERIFIED means unverified input;
    every observed status starting with VERIFIED means verified input;
    anything else, including a mixture, is mixed. Vacuously unverified
    when no claim was observed.
    """

    observed = [value for values in statuses.values() for value in values]
    if all(value.startswith("UNVERIFIED") for value in observed):
        return EVIDENCE_UNVERIFIED
    if observed and all(value.startswith("VERIFIED") for value in observed):
        return EVIDENCE_VERIFIED
    return EVIDENCE_MIXED


# ---------------------------------------------------------------------------
# Arm accounting
# ---------------------------------------------------------------------------


def _task_classes(corpus: Mapping[str, Any]) -> dict[str, str]:
    return {task["id"]: str(task["class"]) for task in corpus["tasks"]}


def arm_accounting(claims: list[Mapping[str, Any]], manifest: Mapping[str, Any], corpus: Mapping[str, Any]) -> dict[str, Any]:
    """Every claim is an attempt; every quantity is exact (contract section 3)."""

    if not claims:
        _fail(AccountingErrorCode.INTERNAL_INVARIANT, "empty arm")
    classes = _task_classes(corpus)
    counts = {status: 0 for status in sorted(STATUSES)}
    sums = {name: 0 for name in SUM_METRICS}
    series: dict[str, list[int]] = {name: [] for name in MEDIAN_METRICS}
    clean_series: dict[str, list[int]] = {name: [] for name in MEDIAN_METRICS}
    by_task: dict[str, dict[str, int]] = {}
    by_class: dict[str, dict[str, Any]] = {}
    by_seed: dict[int, dict[str, int]] = {}
    pairs: set[tuple[str, int]] = set()
    trial_ids: set[str] = set()
    observed: dict[str, set[str]] = {name: set() for name in STATUS_FIELDS}
    for claim in claims:
        status = claim.get("status")
        if status not in STATUSES:
            _fail(AccountingErrorCode.METRIC_INVALID, f"status {status!r}")
        counts[status] += 1
        metrics = claim.get("metrics")
        if not isinstance(metrics, dict):
            _fail(AccountingErrorCode.METRIC_INVALID, "metrics")
        for name in SUM_METRICS:
            sums[name] += _require_int(metrics.get(name), name)
        for name in MEDIAN_METRICS:
            value = _require_int(metrics.get(name), name)
            series[name].append(value)
            if status != "harness_failure":
                clean_series[name].append(value)
        if metrics.get("accepted_change_tokens") is not None:
            # Both runners refuse a pre-derived ACT at the claim layer with
            # "ACT is derived only by S20-630"; accounting restates that
            # refusal so the two meanings of the name can never mix.
            _fail(AccountingErrorCode.METRIC_INVALID, "accepted_change_tokens")
        accepted_change = _require_int(metrics.get("accepted_correct_changes"), "accepted_correct_changes")
        if accepted_change != (1 if status == "accepted" else 0):
            _fail(AccountingErrorCode.METRIC_INVALID, "accepted_correct_changes")
        task_id = str(claim.get("task_id"))
        if task_id not in classes:
            _fail(AccountingErrorCode.METRIC_INVALID, f"task {task_id}")
        seed = _require_int(claim.get("seed"), "seed")
        trial_id = claim.get("trial_id")
        if not isinstance(trial_id, str) or not trial_id:
            _fail(AccountingErrorCode.METRIC_INVALID, "trial_id")
        pairs.add((task_id, seed))
        trial_ids.add(trial_id)
        slot = by_task.setdefault(task_id, {"accepted": 0, "attempted": 0})
        slot["attempted"] += 1
        slot["accepted"] += accepted_change
        klass = by_class.setdefault(classes[task_id], {"accepted": 0, "attempted": 0, "collateral_semantic_changes": 0})
        klass["attempted"] += 1
        klass["accepted"] += accepted_change
        klass["collateral_semantic_changes"] += _require_int(metrics.get("collateral_semantic_changes"), "collateral_semantic_changes")
        sprinkle = by_seed.setdefault(seed, {"accepted": 0, "attempted": 0})
        sprinkle["attempted"] += 1
        sprinkle["accepted"] += accepted_change
        for name in STATUS_FIELDS:
            observed[name].add(_require_status_string(claim, name))
    for klass in by_class.values():
        klass["strict_correctness"] = ratio(klass["accepted"], klass["attempted"])
    seeds = {str(seed): {"accepted": slot["accepted"], "attempted": slot["attempted"], "strict_correctness": ratio(slot["accepted"], slot["attempted"])} for seed, slot in sorted(by_seed.items())}
    attempted = len(claims)
    accepted = counts["accepted"]
    # A duplicated trial slot would double-count in attempted, every sum,
    # and every median while set-equality COMPLETE stayed blind to
    # multiplicity, so the invariant is restated locally: one claim per
    # trial id and one claim per (task, seed) pair.
    if len(trial_ids) != attempted or len(pairs) != attempted:
        _fail(AccountingErrorCode.INTERNAL_INVARIANT, "duplicate trial slot")
    # every_attempt_in_denominator is derived, not asserted: the status
    # partition, every median series, and every grouping must all account
    # for exactly the claims seen, else INTERNAL_INVARIANT.
    grouped = sum(slot["attempted"] for slot in by_task.values())
    classed = sum(klass["attempted"] for klass in by_class.values())
    seeded = sum(slot["attempted"] for slot in by_seed.values())
    partitioned = counts["accepted"] + counts["rejected"] + counts["timeout"] + counts["harness_failure"]
    seriesed = all(len(values) == attempted for values in series.values())
    every_attempt = attempted == partitioned == grouped == classed == seeded and seriesed
    if not every_attempt:
        _fail(AccountingErrorCode.INTERNAL_INVARIANT, "every attempt in denominator")
    act = ratio(sums["total_observable_tokens"], accepted)
    excluded = counts["harness_failure"]
    medians = {name: {"median": median(series[name]), "median_non_harness_failure": median(clean_series[name]), "sum": sum(series[name])} for name in MEDIAN_METRICS}
    return {
        "accepted": accepted,
        "accepted_change_tokens": act,
        "accepted_change_tokens_reason": None if act is not None else "no_accepted_change",
        "attempted": attempted,
        "by_class": dict(sorted(by_class.items())),
        "by_seed": seeds,
        "by_task": dict(sorted(by_task.items())),
        "chain_head_digest": claims[-1]["record_digest"],
        "claim_statuses": {name: sorted(observed[name]) for name in STATUS_FIELDS},
        "claims": attempted,
        "collateral_semantic_changes": sums["collateral_semantic_changes"],
        "compile_or_check_attempts": sums["compile_or_check_attempts"],
        "canonical_storage_bytes": sums["canonical_storage_bytes"],
        "context_bytes": medians["context_bytes"],
        "entities_inspected": sums["entities_inspected"],
        "every_attempt_in_denominator": True,
        "execution_latency": medians["execution_latency"],
        "files_inspected": sums["files_inspected"],
        "harness_failures": excluded,
        "human_interventions": sums["human_interventions"],
        "invalid_candidates": sums["invalid_candidates"],
        "invalid_committed_states": sums["invalid_committed_states"],
        "median_excluded_harness_failures": excluded,
        "model_input_tokens": medians["model_input_tokens"],
        "model_output_tokens": sums["model_output_tokens"],
        "pack_bytes": sums["pack_bytes"],
        "peak_memory": medians["peak_memory"],
        "rejected": counts["rejected"],
        "relationships_inspected": sums["relationships_inspected"],
        "repair_loops": medians["repair_loops"],
        "stale_candidates": sums["stale_candidates"],
        "stale_candidates_incorrectly_accepted": sums["stale_candidates_incorrectly_accepted"],
        "status": "COMPLETE" if pairs == _expected_trial_pairs(manifest) else "PARTIAL",
        "strict_correctness": ratio(accepted, attempted),
        "timeouts": counts["timeout"],
        "tool_calls": sums["tool_calls"],
        "total_observable_tokens": sums["total_observable_tokens"],
        "wall_time": medians["wall_time"],
    }


# ---------------------------------------------------------------------------
# Thresholds (contract section 4, plan `thresholds`)
# ---------------------------------------------------------------------------


def _reduction(before: Fraction | None, after: Fraction | None) -> Fraction | None:
    """Relative reduction from `before` to `after`; `None` when undefined."""

    if before is None or after is None or before == 0:
        return None
    return (before - after) / before


def _complete(arm: Any) -> bool:
    return isinstance(arm, dict) and arm.get("status") == "COMPLETE"


def _action_budget(manifest: Mapping[str, Any]) -> int:
    """The single shared action budget direct count comparison depends on."""

    for key in manifest:
        if "budget" in str(key) and str(key) not in KNOWN_BUDGET_FIELDS:
            _fail(AccountingErrorCode.RUN_INVALID, f"unexpected budget field {key}")
    budget = manifest.get("action_budget")
    if isinstance(budget, bool) or not isinstance(budget, int):
        _fail(AccountingErrorCode.RUN_INVALID, "action_budget")
    return budget


def evaluate_thresholds(arms: Mapping[str, Any], plan: Mapping[str, Any], manifest: Mapping[str, Any], evidence_status: str) -> dict[str, dict[str, Any]]:
    """Each plan threshold as PASS, FAIL, or UNDETERMINED with its exact facts."""

    thresholds = plan["thresholds"]
    legacy = arms.get(LEGACY_ARM)
    sley2 = arms.get(SLEY2_ARM)
    results: dict[str, dict[str, Any]] = {}
    for name, condition, owner in NOT_EVALUATED_CONDITIONS:
        results[name] = {"evidence_status": evidence_status, "reason": {"owner": owner, "section_22_condition": condition}, "result": "NOT_EVALUATED"}
    if not (_complete(legacy) and _complete(sley2)):
        reason = {
            LEGACY_ARM: legacy["status"] if isinstance(legacy, dict) else legacy,
            SLEY2_ARM: sley2["status"] if isinstance(sley2, dict) else sley2,
        }
        for name in thresholds:
            results[name] = {"evidence_status": evidence_status, "reason": reason, "result": "UNDETERMINED"}
        _check_coverage(results, thresholds)
        return dict(sorted(results.items()))

    def outcome(name: str, passed: bool, facts: Mapping[str, Any]) -> None:
        facts = dict(facts)
        facts["evidence_status"] = evidence_status
        results[name] = {"evidence_status": evidence_status, "facts": facts, "result": "PASS" if passed else "FAIL"}

    def undetermined(name: str, reason: Mapping[str, Any]) -> None:
        results[name] = {"evidence_status": evidence_status, "reason": dict(reason), "result": "UNDETERMINED"}

    # Correctness by class: the plan's criticality flag is read, and until
    # the corpus names critical classes every class counts as critical, a
    # deliberate strengthening recorded in the facts.
    if thresholds.get("critical_class_correctness_not_lower_than_legacy") is not True:
        _fail(AccountingErrorCode.METRIC_INVALID, "critical_class_correctness")
    class_facts: dict[str, Any] = {"critical_class_selection": "ALL_CLASSES"}
    not_lower = True
    for klass in sorted(set(legacy["by_class"]) | set(sley2["by_class"])):
        left = as_fraction(legacy["by_class"].get(klass, {}).get("strict_correctness"))
        right = as_fraction(sley2["by_class"].get(klass, {}).get("strict_correctness"))
        class_facts[klass] = {LEGACY_ARM: from_fraction(left), SLEY2_ARM: from_fraction(right)}
        if left is None or right is None or right < left:
            not_lower = False
    outcome("critical_class_correctness_not_lower_than_legacy", not_lower, class_facts)

    # Failure rate is one minus strict correctness; a perfect legacy arm
    # leaves the reduction undefined, which is a named null, never a pass.
    failure_percent, act_percent = thresholds["failure_rate_relative_reduction_percent_or_equal_correctness_act_reduction_percent"]
    correctness_legacy = as_fraction(legacy["strict_correctness"])
    correctness_sley2 = as_fraction(sley2["strict_correctness"])
    failure_legacy = 1 - correctness_legacy if correctness_legacy is not None else None
    failure_sley2 = 1 - correctness_sley2 if correctness_sley2 is not None else None
    failure_reduction = _reduction(failure_legacy, failure_sley2) if correctness_legacy is not None and correctness_sley2 is not None else None
    act_reduction = _reduction(as_fraction(legacy["accepted_change_tokens"]), as_fraction(sley2["accepted_change_tokens"]))
    equal_correctness = correctness_legacy is not None and correctness_legacy == correctness_sley2
    by_failure = failure_reduction is not None and failure_reduction >= _percent(failure_percent)
    by_act = equal_correctness and act_reduction is not None and act_reduction >= _percent(act_percent)
    failure_facts: dict[str, Any] = {
        "act_reduction": from_fraction(act_reduction),
        "equal_correctness": equal_correctness,
        "equal_strict_correctness_ratios": equal_correctness,
        "failure_rate": {LEGACY_ARM: from_fraction(failure_legacy), SLEY2_ARM: from_fraction(failure_sley2)},
        "failure_rate_reduction": from_fraction(failure_reduction),
        "strict_correctness": {LEGACY_ARM: legacy["strict_correctness"], SLEY2_ARM: sley2["strict_correctness"]},
    }
    if failure_reduction is None and failure_legacy == 0:
        failure_facts["failure_reduction_reason"] = "legacy_failure_rate_zero"
    outcome(
        "failure_rate_relative_reduction_percent_or_equal_correctness_act_reduction_percent",
        by_failure or by_act,
        failure_facts,
    )

    # Each leg passes only on its own measured reduction with the other
    # metric's regression inside the cap; an undefined other-metric
    # regression leaves the leg UNDETERMINED, never compliant.
    context_percent, tokens_percent = thresholds["median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent"]
    regression_cap = _percent(thresholds["other_context_metric_max_regression_percent"])
    context_reduction = _reduction(as_fraction(legacy["context_bytes"]["median"]), as_fraction(sley2["context_bytes"]["median"]))
    tokens_reduction = _reduction(as_fraction(legacy["model_input_tokens"]["median"]), as_fraction(sley2["model_input_tokens"]["median"]))

    def leg(reduction: Fraction | None, threshold: int, other: Fraction | None, other_name: str) -> tuple[str, str]:
        if reduction is None:
            return ("UNDETERMINED", "reduction_undefined")
        if reduction < _percent(threshold):
            return ("FAIL", "reduction_below_threshold")
        if other is None:
            return ("UNDETERMINED", f"{other_name}_regression_undefined")
        if -other > regression_cap:
            return ("FAIL", f"{other_name}_regressed_beyond_cap")
        return ("PASS", "reduction_met_regression_within_cap")

    context_leg, context_leg_reason = leg(context_reduction, context_percent, tokens_reduction, "model_input_tokens")
    tokens_leg, tokens_leg_reason = leg(tokens_reduction, tokens_percent, context_reduction, "context_bytes")
    context_facts = {
        "context_bytes_reduction": from_fraction(context_reduction),
        "context_leg": context_leg,
        "context_leg_reason": context_leg_reason,
        "median_basis": MEDIAN_BASIS,
        "model_input_tokens_reduction": from_fraction(tokens_reduction),
        "tokens_leg": tokens_leg,
        "tokens_leg_reason": tokens_leg_reason,
    }
    if context_leg == "PASS" or tokens_leg == "PASS":
        outcome("median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent", True, context_facts)
    elif context_leg == "UNDETERMINED" or tokens_leg == "UNDETERMINED":
        undetermined("median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent", {**context_facts, "reason": "leg_undefined"})
    else:
        outcome("median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent", False, context_facts)
    # The cap row measures the actual regressions of both context metrics
    # instead of mirroring the threshold above; an undefined regression is
    # a named null, never a pass.
    if context_reduction is None or tokens_reduction is None:
        missing = sorted(name for name, value in (("context_bytes", context_reduction), ("model_input_tokens", tokens_reduction)) if value is None)
        undetermined("other_context_metric_max_regression_percent", {"cap": from_fraction(regression_cap), "context_bytes_reduction": from_fraction(context_reduction), "model_input_tokens_reduction": from_fraction(tokens_reduction), "reason": f"regression_undefined:{','.join(missing)}"})
    else:
        outcome(
            "other_context_metric_max_regression_percent",
            -context_reduction <= regression_cap and -tokens_reduction <= regression_cap,
            {"cap": from_fraction(regression_cap), "context_bytes_reduction": from_fraction(context_reduction), "model_input_tokens_reduction": from_fraction(tokens_reduction)},
        )

    budget = _action_budget(manifest)
    repair_percent, accepted_percent = thresholds["median_repair_loop_reduction_percent_or_accepted_changes_increase_percent"]
    repair_reduction = _reduction(as_fraction(legacy["repair_loops"]["median"]), as_fraction(sley2["repair_loops"]["median"]))
    accepted_increase = Fraction(sley2["accepted"] - legacy["accepted"], legacy["accepted"]) if legacy["accepted"] else None
    repair_facts: dict[str, Any] = {
        "accepted": {LEGACY_ARM: legacy["accepted"], SLEY2_ARM: sley2["accepted"]},
        "accepted_changes_increase": from_fraction(accepted_increase),
        "action_budget": budget,
        "median_basis": MEDIAN_BASIS,
        "repair_loops_reduction": from_fraction(repair_reduction),
    }
    if accepted_increase is None and not legacy["accepted"]:
        # Conservative by contract: unbounded improvement over a legacy arm
        # that accepted nothing reads as unavailable, with its reason.
        repair_facts["accepted_changes_increase_reason"] = "legacy_accepted_zero"
    outcome(
        "median_repair_loop_reduction_percent_or_accepted_changes_increase_percent",
        (repair_reduction is not None and repair_reduction >= _percent(repair_percent))
        or (accepted_increase is not None and accepted_increase >= _percent(accepted_percent)),
        repair_facts,
    )

    # Percent rows compute the exact ratio against the plan value; a zero
    # denominator is a named null per contract section 2, never a pass.
    _percent_row(results, evidence_status, "invalid_candidates_leave_state_unchanged_percent", thresholds["invalid_candidates_leave_state_unchanged_percent"], sley2["invalid_candidates"], sley2["invalid_committed_states"], "no_invalid_candidates")
    _percent_row(results, evidence_status, "stale_preconditions_rejected_percent", thresholds["stale_preconditions_rejected_percent"], sley2["stale_candidates"], sley2["stale_candidates_incorrectly_accepted"], "no_stale_candidates")
    outcome("invalid_committed_states", sley2["invalid_committed_states"] == thresholds["invalid_committed_states"], {"value": sley2["invalid_committed_states"]})
    outcome("stale_conflicting_candidates_accepted", sley2["stale_candidates_incorrectly_accepted"] == thresholds["stale_conflicting_candidates_accepted"], {"value": sley2["stale_candidates_incorrectly_accepted"]})
    outcome("human_interventions_sley2", sley2["human_interventions"] == thresholds["human_interventions_sley2"], {"value": sley2["human_interventions"]})
    _check_coverage(results, thresholds)
    return dict(sorted(results.items()))


def _percent_row(results: dict[str, dict[str, Any]], evidence_status: str, name: str, plan_percent: Any, denominator: int, bad: int, empty_reason: str) -> None:
    facts = {"bad": _require_int(bad, name), "denominator": _require_int(denominator, name), "evidence_status": evidence_status, "plan_percent": _require_int(plan_percent, name)}
    if denominator == 0:
        facts["reason"] = empty_reason
        results[name] = {"evidence_status": evidence_status, "reason": dict(facts), "result": "UNDETERMINED"}
        return
    good = Fraction(denominator - bad, denominator)
    facts["good_ratio"] = from_fraction(good)
    results[name] = {"evidence_status": evidence_status, "facts": facts, "result": "PASS" if good >= _percent(plan_percent) else "FAIL"}


def _check_coverage(results: Mapping[str, Any], thresholds: Mapping[str, Any]) -> None:
    """Every plan threshold appears exactly once, beside the named NOT_EVALUATED rows."""

    expected = set(thresholds) | NOT_EVALUATED_NAMES
    if set(results) != expected:
        _fail(AccountingErrorCode.METRIC_INVALID, f"threshold coverage {sorted(set(results) ^ expected)}")


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------

ARM_VERIFIERS: dict[str, Callable[..., list[dict[str, Any]]]] = {
    RAW_ARM: verify_digest_claim_directory,
    SLEY2_ARM: verify_trial_claims,
}
# LEGACY_ARM has no verifier until S20-600 supplies a chain producer; the
# registry stays the single place that fact lives, so the producer plugs in
# without editing accounting. The legacy chain's reserved path is named here
# so a chain that appears before its verifier fails closed instead of
# reading as absent.
LEGACY_CHAIN_RELATIVE = Path("legacy") / "claims.jsonl"


def _arm_claims(run_directory: Path, arm: str, fixture_status: Any) -> tuple[list[dict[str, Any]] | None, str | None]:
    verifier = ARM_VERIFIERS.get(arm)
    if verifier is None:
        if arm == LEGACY_ARM:
            if (run_directory / LEGACY_CHAIN_RELATIVE).exists():
                _fail(AccountingErrorCode.CHAIN_INVALID, f"{arm}:chain-present-no-verifier")
            return None, None
        _fail(AccountingErrorCode.ARM_UNKNOWN, arm)
    try:
        return verifier(run_directory) or None, verifier.__name__
    except (RawRunnerError, Sley2RunnerError) as error:
        raise AccountingError(AccountingErrorCode.CHAIN_INVALID, f"{arm}:{error}") from error


def report_digest(report: Mapping[str, Any]) -> str:
    body = {key: value for key, value in report.items() if key != "report_digest"}
    try:
        payload = canonical_json_bytes(body)
    except RawRunnerError as error:
        if "unsupported JSON value" in str(error):
            raise AccountingError(AccountingErrorCode.FLOAT_FORBIDDEN, error.detail) from error
        raise AccountingError(AccountingErrorCode.REPORT_INVALID, error.detail) from error
    return hashlib.sha256(REPORT_DOMAIN + payload).hexdigest()


def derive_report(run_directory: Path, *, require_complete: bool = False) -> dict[str, Any]:
    """Derive the accounting report of one run from its manifest and chains."""

    try:
        manifest = _read_manifest_exact(run_directory)
    except (RawRunnerError, OSError) as error:
        raise AccountingError(AccountingErrorCode.RUN_INVALID, str(error)) from error
    plan, corpus = _plan_and_corpus()
    arm_info = {arm["id"]: arm for arm in plan["arms"]}
    required = [arm["id"] for arm in plan["arms"] if arm.get("required")]
    arms: dict[str, Any] = {}
    observed: dict[str, set[str]] = {name: set() for name in STATUS_FIELDS}
    for arm in required:
        claims, verifier_name = _arm_claims(run_directory, arm, arm_info.get(arm, {}).get("fixture_status"))
        for claim in claims or []:
            if claim.get("arm_id") != arm:
                _fail(AccountingErrorCode.ARM_UNKNOWN, str(claim.get("arm_id")))
        if not claims:
            arms[arm] = NO_CHAIN
            continue
        accounting = arm_accounting(claims, manifest, corpus)
        accounting["chain_verifier"] = verifier_name
        accounting["fixture_status"] = arm_info.get(arm, {}).get("fixture_status")
        arms[arm] = accounting
        for name in STATUS_FIELDS:
            observed[name].update(accounting["claim_statuses"][name])
    statuses = {name: sorted(observed[name]) for name in STATUS_FIELDS}
    evidence_status = derive_evidence_status(statuses)
    if all(value == NO_CHAIN for value in arms.values()):
        status = "NO_TRIALS"
    elif all(_complete(value) for value in arms.values()):
        # COMPLETE needs every required arm complete, including raw_files,
        # even though the thresholds below compare legacy and Sley 2 only.
        status = "COMPLETE"
    else:
        status = "PARTIAL"
    if require_complete and status != "COMPLETE":
        _fail(AccountingErrorCode.INCOMPLETE, status)
    for key in ("benchmark_plan_digest", "corpus_digest"):
        if not isinstance(manifest.get(key), str) or not manifest[key]:
            _fail(AccountingErrorCode.RUN_INVALID, key)
    report = {
        "arm_fixture_status": {arm: arm_info.get(arm, {}).get("fixture_status") for arm in required},
        "arms": arms,
        "benchmark_plan_digest": manifest["benchmark_plan_digest"],
        "claim_statuses": statuses,
        "contract": REPORT_CONTRACT,
        "corpus_digest": manifest["corpus_digest"],
        "corpus_version": manifest["corpus_version"],
        "evidence_status": evidence_status,
        "required_arms": required,
        "run_id": manifest["run_id"],
        "run_manifest_digest": manifest_digest(manifest),
        "status": status,
        "thresholds": evaluate_thresholds(arms, plan, manifest, evidence_status),
    }
    report["report_digest"] = report_digest(report)
    return report


def verify_report(report: Mapping[str, Any]) -> None:
    if report.get("contract") != REPORT_CONTRACT:
        _fail(AccountingErrorCode.REPORT_INVALID, "contract")
    statuses = report.get("claim_statuses")
    if not isinstance(statuses, dict) or set(statuses) != set(STATUS_FIELDS):
        _fail(AccountingErrorCode.REPORT_INVALID, "claim_statuses")
    recorded = {name: statuses[name] for name in STATUS_FIELDS}
    if any(not isinstance(values, list) or any(not isinstance(value, str) for value in values) for values in recorded.values()):
        _fail(AccountingErrorCode.REPORT_INVALID, "claim_statuses")
    # The recorded status is re-derived from the recorded claim statuses,
    # so verification binds the value instead of a module constant and
    # already-written reports keep verifying after any future change.
    if report.get("evidence_status") != derive_evidence_status(recorded):
        _fail(AccountingErrorCode.REPORT_INVALID, "evidence_status")
    if report.get("report_digest") != report_digest(report):
        _fail(AccountingErrorCode.REPORT_INVALID, "digest")


def write_report(report: Mapping[str, Any], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(canonical_json_bytes(dict(report)) + b"\n")


def smoke(sley2_evidence: Path, output_directory: Path) -> int:
    source = json.loads(sley2_evidence.read_text(encoding="utf-8"))
    run_directory = ROOT / source["run_directory"]
    try:
        report = derive_report(run_directory)
        verify_report(report)
    except (AccountingError, RawRunnerError, Sley2RunnerError, OSError, ValueError, KeyError) as error:
        evidence: dict[str, Any] = {
            "contract": "s20-630-accounting-smoke-v1",
            "work_package": "S20-630",
            "scope": "REPORT_OVER_THE_S20_620_SCRIPTED_SMOKE_RUN",
            "full_s20_630_complete": False,
            "trials_accounted": 0,
            "problems": [f"{type(error).__name__}:{error}"],
            "result": "FAIL",
        }
        output_directory.mkdir(parents=True, exist_ok=True)
        (output_directory / "evidence.json").write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(json.dumps({key: evidence[key] for key in ("contract", "result", "problems")}, indent=2, sort_keys=True))
        return 1
    attempted = sum(arm["attempted"] for arm in report["arms"].values() if isinstance(arm, dict))
    sley2 = report["arms"][SLEY2_ARM]
    evidence = {
        "contract": "s20-630-accounting-smoke-v1",
        "work_package": "S20-630",
        "scope": f"REPORT_OVER_THE_S20_620_SCRIPTED_SMOKE_RUN_{report['status']}_{attempted}_SCRIPTED_ATTEMPTS",
        "full_s20_630_complete": False,
        "trials_accounted": attempted,
        "problems": [],
    }
    evidence["run_directory"] = source["run_directory"]
    write_report(report, output_directory / "report.json")
    checks = {
        "status_partial": report["status"] == "PARTIAL",
        "raw_arm_absent": report["arms"][RAW_ARM] == NO_CHAIN,
        "legacy_arm_absent": report["arms"][LEGACY_ARM] == NO_CHAIN,
        "sley2_arm_partial": isinstance(sley2, dict) and sley2["status"] == "PARTIAL",
        "no_accepted_change": isinstance(sley2, dict) and sley2["accepted"] == 0 and sley2["accepted_change_tokens"] is None,
        "every_attempt_counted": isinstance(sley2, dict) and sley2["attempted"] == sley2["accepted"] + sley2["rejected"] + sley2["timeouts"] + sley2["harness_failures"],
        "thresholds_undetermined": all(value["result"] in ("UNDETERMINED", "NOT_EVALUATED") for value in report["thresholds"].values()),
    }
    evidence["checks"] = checks
    evidence["report_digest"] = report["report_digest"]
    evidence["report_status"] = report["status"]
    for name, passed in checks.items():
        if not passed:
            evidence["problems"].append(name)
    evidence["result"] = "PASS" if not evidence["problems"] else "FAIL"
    output_directory.mkdir(parents=True, exist_ok=True)
    (output_directory / "evidence.json").write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({key: evidence[key] for key in ("contract", "result", "problems")}, indent=2, sort_keys=True))
    return 0 if evidence["result"] == "PASS" else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    derive = commands.add_parser("derive", help="derive the accounting report of a run directory")
    derive.add_argument("--run", type=Path, required=True)
    derive.add_argument("--output", type=Path)
    derive.add_argument("--require-complete", action="store_true")
    smoke_parser = commands.add_parser("smoke", help="derive the report over the S20-620 smoke run")
    smoke_parser.add_argument("--sley2-evidence", type=Path, required=True)
    smoke_parser.add_argument("--output-dir", type=Path, required=True)
    arguments = parser.parse_args(argv)
    if arguments.command == "smoke":
        return smoke(arguments.sley2_evidence.resolve(), arguments.output_dir.resolve())
    try:
        report = derive_report(arguments.run, require_complete=arguments.require_complete)
        if arguments.output:
            write_report(report, arguments.output)
        # DERIVED names a successful derivation; it is never a threshold
        # verdict, which lives on the report's threshold rows.
        print(json.dumps({"evidence_status": report["evidence_status"], "report_digest": report["report_digest"], "result": "DERIVED", "status": report["status"]}, sort_keys=True))
        return 0
    except (AccountingError, RawRunnerError, Sley2RunnerError) as error:
        print(json.dumps({"problem": str(error), "result": "FAIL"}, sort_keys=True))
        return 1


if __name__ == "__main__":
    sys.exit(main())
