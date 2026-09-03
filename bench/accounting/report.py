#!/usr/bin/env python3
"""S20-630 succession accounting: exact derivation from immutable claims.

The report reads one run's create-once manifest and each arm's claim chain,
verified by that arm's own runner, and nothing else. It keeps every attempt
in every denominator, uses integers and reduced ratios only, evaluates the
plan's section 22 thresholds only between complete arms, and inherits the
claims' unverified evidence status. It executes nothing and reads no clock.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from enum import IntEnum
from fractions import Fraction
from pathlib import Path
from typing import Any, Mapping

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
EVIDENCE_STATUS = "DERIVED_FROM_UNVERIFIED_CLAIMS"
RAW_ARM = "raw_files"
LEGACY_ARM = "sley_1_2_0"
SLEY2_ARM = "sley_2_0"
NO_CHAIN = "NO_CLAIM_CHAIN"
STATUSES = frozenset({"accepted", "rejected", "timeout", "harness_failure"})
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
)
MEDIAN_METRICS = ("context_bytes", "model_input_tokens", "repair_loops", "wall_time")


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
        if isinstance(value, (int, str)) is False and type(value).__name__ == "float":
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
    by_task: dict[str, dict[str, int]] = {}
    by_class: dict[str, dict[str, Any]] = {}
    pairs: set[tuple[str, int]] = set()
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
            series[name].append(_require_int(metrics.get(name), name))
        accepted_change = _require_int(metrics.get("accepted_correct_changes"), "accepted_correct_changes")
        if accepted_change != (1 if status == "accepted" else 0):
            _fail(AccountingErrorCode.METRIC_INVALID, "accepted_correct_changes")
        task_id = str(claim.get("task_id"))
        if task_id not in classes:
            _fail(AccountingErrorCode.METRIC_INVALID, f"task {task_id}")
        seed = _require_int(claim.get("seed"), "seed")
        pairs.add((task_id, seed))
        slot = by_task.setdefault(task_id, {"accepted": 0, "attempted": 0})
        slot["attempted"] += 1
        slot["accepted"] += accepted_change
        klass = by_class.setdefault(classes[task_id], {"accepted": 0, "attempted": 0})
        klass["attempted"] += 1
        klass["accepted"] += accepted_change
    for klass in by_class.values():
        klass["strict_correctness"] = ratio(klass["accepted"], klass["attempted"])
    attempted = len(claims)
    accepted = counts["accepted"]
    act = ratio(sums["total_observable_tokens"], accepted)
    return {
        "accepted": accepted,
        "accepted_change_tokens": act,
        "accepted_change_tokens_reason": None if act is not None else "no_accepted_change",
        "attempted": attempted,
        "by_class": dict(sorted(by_class.items())),
        "by_task": dict(sorted(by_task.items())),
        "chain_head_digest": claims[-1]["record_digest"],
        "claims": attempted,
        "collateral_semantic_changes": sums["collateral_semantic_changes"],
        "compile_or_check_attempts": sums["compile_or_check_attempts"],
        "context_bytes": {"median": median(series["context_bytes"]), "sum": sum(series["context_bytes"])},
        "harness_failures": counts["harness_failure"],
        "human_interventions": sums["human_interventions"],
        "invalid_candidates": sums["invalid_candidates"],
        "invalid_committed_states": sums["invalid_committed_states"],
        "model_input_tokens": sums["model_input_tokens"],
        "model_input_tokens_median": median(series["model_input_tokens"]),
        "model_output_tokens": sums["model_output_tokens"],
        "rejected": counts["rejected"],
        "repair_loops": {"median": median(series["repair_loops"]), "sum": sum(series["repair_loops"])},
        "stale_candidates": sums["stale_candidates"],
        "stale_candidates_incorrectly_accepted": sums["stale_candidates_incorrectly_accepted"],
        "status": "COMPLETE" if pairs == _expected_trial_pairs(manifest) else "PARTIAL",
        "strict_correctness": ratio(accepted, attempted),
        "timeouts": counts["timeout"],
        "tool_calls": sums["tool_calls"],
        "total_observable_tokens": sums["total_observable_tokens"],
        "wall_time": {"median": median(series["wall_time"]), "sum": sum(series["wall_time"])},
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


def evaluate_thresholds(arms: Mapping[str, Any], plan: Mapping[str, Any]) -> dict[str, dict[str, Any]]:
    """Each plan threshold as PASS, FAIL, or UNDETERMINED with its exact facts."""

    thresholds = plan["thresholds"]
    legacy = arms.get(LEGACY_ARM)
    sley2 = arms.get(SLEY2_ARM)
    results: dict[str, dict[str, Any]] = {}
    if not (_complete(legacy) and _complete(sley2)):
        reason = {
            LEGACY_ARM: legacy["status"] if isinstance(legacy, dict) else legacy,
            SLEY2_ARM: sley2["status"] if isinstance(sley2, dict) else sley2,
        }
        for name in thresholds:
            results[name] = {"reason": reason, "result": "UNDETERMINED"}
        return results

    def outcome(name: str, passed: bool, facts: Mapping[str, Any]) -> None:
        results[name] = {"facts": dict(facts), "result": "PASS" if passed else "FAIL"}

    # Correctness by class: every class counts as critical.
    class_facts: dict[str, Any] = {}
    not_lower = True
    for klass in sorted(set(legacy["by_class"]) | set(sley2["by_class"])):
        left = as_fraction(legacy["by_class"].get(klass, {}).get("strict_correctness"))
        right = as_fraction(sley2["by_class"].get(klass, {}).get("strict_correctness"))
        class_facts[klass] = {LEGACY_ARM: from_fraction(left), SLEY2_ARM: from_fraction(right)}
        if left is None or right is None or right < left:
            not_lower = False
    outcome("critical_class_correctness_not_lower_than_legacy", not_lower, class_facts)

    failure_percent, act_percent = thresholds["failure_rate_relative_reduction_percent_or_equal_correctness_act_reduction_percent"]
    correctness_legacy = as_fraction(legacy["strict_correctness"])
    correctness_sley2 = as_fraction(sley2["strict_correctness"])
    failure_reduction = _reduction(1 - correctness_legacy, 1 - correctness_sley2) if correctness_legacy is not None and correctness_sley2 is not None else None
    act_reduction = _reduction(as_fraction(legacy["accepted_change_tokens"]), as_fraction(sley2["accepted_change_tokens"]))
    equal_correctness = correctness_legacy is not None and correctness_legacy == correctness_sley2
    by_failure = failure_reduction is not None and failure_reduction >= _percent(failure_percent)
    by_act = equal_correctness and act_reduction is not None and act_reduction >= _percent(act_percent)
    outcome(
        "failure_rate_relative_reduction_percent_or_equal_correctness_act_reduction_percent",
        by_failure or by_act,
        {
            "act_reduction": from_fraction(act_reduction),
            "equal_correctness": equal_correctness,
            "failure_rate_reduction": from_fraction(failure_reduction),
            "strict_correctness": {LEGACY_ARM: legacy["strict_correctness"], SLEY2_ARM: sley2["strict_correctness"]},
        },
    )

    context_percent, tokens_percent = thresholds["median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent"]
    regression_cap = _percent(thresholds["other_context_metric_max_regression_percent"])
    context_reduction = _reduction(as_fraction(legacy["context_bytes"]["median"]), as_fraction(sley2["context_bytes"]["median"]))
    tokens_reduction = _reduction(as_fraction(legacy["model_input_tokens_median"]), as_fraction(sley2["model_input_tokens_median"]))
    context_ok = context_reduction is not None and context_reduction >= _percent(context_percent) and (tokens_reduction is None or -tokens_reduction <= regression_cap)
    tokens_ok = tokens_reduction is not None and tokens_reduction >= _percent(tokens_percent) and (context_reduction is None or -context_reduction <= regression_cap)
    outcome(
        "median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent",
        context_ok or tokens_ok,
        {"context_bytes_reduction": from_fraction(context_reduction), "model_input_tokens_reduction": from_fraction(tokens_reduction)},
    )
    results["other_context_metric_max_regression_percent"] = {
        "facts": {"cap": from_fraction(regression_cap)},
        "result": results["median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent"]["result"],
    }

    repair_percent, accepted_percent = thresholds["median_repair_loop_reduction_percent_or_accepted_changes_increase_percent"]
    repair_reduction = _reduction(as_fraction(legacy["repair_loops"]["median"]), as_fraction(sley2["repair_loops"]["median"]))
    accepted_increase = Fraction(sley2["accepted"] - legacy["accepted"], legacy["accepted"]) if legacy["accepted"] else None
    outcome(
        "median_repair_loop_reduction_percent_or_accepted_changes_increase_percent",
        (repair_reduction is not None and repair_reduction >= _percent(repair_percent))
        or (accepted_increase is not None and accepted_increase >= _percent(accepted_percent)),
        {"accepted_changes_increase": from_fraction(accepted_increase), "repair_loops_reduction": from_fraction(repair_reduction)},
    )

    outcome("invalid_candidates_leave_state_unchanged_percent", sley2["invalid_committed_states"] == 0, {"invalid_committed_states": sley2["invalid_committed_states"]})
    outcome("stale_preconditions_rejected_percent", sley2["stale_candidates_incorrectly_accepted"] == 0, {"stale_candidates_incorrectly_accepted": sley2["stale_candidates_incorrectly_accepted"]})
    outcome("invalid_committed_states", sley2["invalid_committed_states"] == thresholds["invalid_committed_states"], {"value": sley2["invalid_committed_states"]})
    outcome("stale_conflicting_candidates_accepted", sley2["stale_candidates_incorrectly_accepted"] == thresholds["stale_conflicting_candidates_accepted"], {"value": sley2["stale_candidates_incorrectly_accepted"]})
    outcome("human_interventions_sley2", sley2["human_interventions"] == thresholds["human_interventions_sley2"], {"value": sley2["human_interventions"]})
    return dict(sorted(results.items()))


# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------


def _arm_claims(run_directory: Path, arm: str) -> list[dict[str, Any]] | None:
    try:
        if arm == RAW_ARM:
            return verify_digest_claim_directory(run_directory) or None
        if arm == SLEY2_ARM:
            return verify_trial_claims(run_directory) or None
        if arm == LEGACY_ARM:
            return None
    except (RawRunnerError, Sley2RunnerError) as error:
        raise AccountingError(AccountingErrorCode.CHAIN_INVALID, f"{arm}:{error}") from error
    _fail(AccountingErrorCode.ARM_UNKNOWN, arm)
    return None


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
    required = [arm["id"] for arm in plan["arms"] if arm.get("required")]
    arms: dict[str, Any] = {}
    for arm in required:
        claims = _arm_claims(run_directory, arm)
        for claim in claims or []:
            if claim.get("arm_id") != arm:
                _fail(AccountingErrorCode.ARM_UNKNOWN, str(claim.get("arm_id")))
        arms[arm] = arm_accounting(claims, manifest, corpus) if claims else NO_CHAIN
    if all(value == NO_CHAIN for value in arms.values()):
        status = "NO_TRIALS"
    elif all(_complete(value) for value in arms.values()):
        status = "COMPLETE"
    else:
        status = "PARTIAL"
    if require_complete and status != "COMPLETE":
        _fail(AccountingErrorCode.INCOMPLETE, status)
    report = {
        "arms": arms,
        "contract": REPORT_CONTRACT,
        "corpus_version": manifest["corpus_version"],
        "every_attempt_in_denominator": True,
        "evidence_status": EVIDENCE_STATUS,
        "required_arms": required,
        "run_id": manifest["run_id"],
        "run_manifest_digest": manifest_digest(manifest),
        "status": status,
        "thresholds": evaluate_thresholds(arms, plan),
    }
    report["report_digest"] = report_digest(report)
    return report


def verify_report(report: Mapping[str, Any]) -> None:
    if report.get("contract") != REPORT_CONTRACT or report.get("evidence_status") != EVIDENCE_STATUS:
        _fail(AccountingErrorCode.REPORT_INVALID, "contract")
    if report.get("report_digest") != report_digest(report):
        _fail(AccountingErrorCode.REPORT_INVALID, "digest")


def write_report(report: Mapping[str, Any], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(canonical_json_bytes(dict(report)) + b"\n")


def smoke(sley2_evidence: Path, output_directory: Path) -> int:
    evidence: dict[str, Any] = {
        "contract": "s20-630-accounting-smoke-v1",
        "work_package": "S20-630",
        "scope": "REPORT_OVER_THE_S20_620_SCRIPTED_SMOKE_RUN_NO_TRIALS",
        "full_s20_630_complete": False,
        "trials_accounted": 0,
        "problems": [],
    }
    try:
        source = json.loads(sley2_evidence.read_text(encoding="utf-8"))
        run_directory = ROOT / source["run_directory"]
        evidence["run_directory"] = source["run_directory"]
        report = derive_report(run_directory)
        verify_report(report)
        write_report(report, output_directory / "report.json")
        sley2 = report["arms"][SLEY2_ARM]
        checks = {
            "status_partial": report["status"] == "PARTIAL",
            "raw_arm_absent": report["arms"][RAW_ARM] == NO_CHAIN,
            "legacy_arm_absent": report["arms"][LEGACY_ARM] == NO_CHAIN,
            "sley2_arm_partial": isinstance(sley2, dict) and sley2["status"] == "PARTIAL",
            "no_accepted_change": isinstance(sley2, dict) and sley2["accepted"] == 0 and sley2["accepted_change_tokens"] is None,
            "every_attempt_counted": isinstance(sley2, dict) and sley2["attempted"] == sley2["claims"] == sley2["rejected"] + sley2["harness_failures"] + sley2["timeouts"],
            "thresholds_undetermined": all(value["result"] == "UNDETERMINED" for value in report["thresholds"].values()),
        }
        evidence["checks"] = checks
        evidence["report_digest"] = report["report_digest"]
        evidence["report_status"] = report["status"]
        for name, passed in checks.items():
            if not passed:
                evidence["problems"].append(name)
    except (AccountingError, RawRunnerError, Sley2RunnerError, OSError, ValueError, KeyError) as error:
        evidence["problems"].append(f"{type(error).__name__}:{error}")
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
        print(json.dumps({"report_digest": report["report_digest"], "result": "PASS", "status": report["status"]}, sort_keys=True))
        return 0
    except (AccountingError, RawRunnerError, Sley2RunnerError) as error:
        print(json.dumps({"problem": str(error), "result": "FAIL"}, sort_keys=True))
        return 1


if __name__ == "__main__":
    sys.exit(main())
