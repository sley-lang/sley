"""Offline tests for S20-630 accounting: exact arithmetic, denominators, thresholds, reports."""

from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from fractions import Fraction
from pathlib import Path

from bench.accounting.report import (
    LEGACY_ARM,
    NO_CHAIN,
    RAW_ARM,
    SLEY2_ARM,
    AccountingError,
    AccountingErrorCode,
    arm_accounting,
    derive_report,
    evaluate_thresholds,
    median,
    ratio,
    report_digest,
    verify_report,
)
from bench.raw.runner import (
    PLAN_PATH,
    _plan_and_corpus,
    append_trial_digest_claim,
    write_run_manifest,
)
from bench.sley2.runner import ARM, CLAIM_CONTRACT, append_trial_claim, smoke_manifest


def digest(byte: int) -> str:
    return f"{byte:02x}" * 32


def zero_metrics() -> dict:
    plan = json.loads(PLAN_PATH.read_text(encoding="utf-8"))
    values = {name: 0 for name in plan["metrics"]}
    values["attempted_tasks"] = 1
    values["strict_accepted_correctness"] = False
    values["accepted_change_tokens"] = None
    return values


def raw_claim(run_id: str, task_id: str, seed: int, status: str, *, tokens: int, context: int, repairs: int) -> dict:
    accepted = status == "accepted"
    timed_out = status == "timeout"
    metrics = zero_metrics()
    metrics.update(
        {
            "strict_accepted_correctness": accepted,
            "accepted_correct_changes": int(accepted),
            "model_input_tokens": tokens,
            "model_output_tokens": tokens // 2,
            "total_observable_tokens": tokens + tokens // 2,
            "context_bytes": context,
            "repair_loops": repairs,
            "wall_time": 1_000,
        }
    )
    return {
        "contract": "sley2.raw-trial-digest-claim.v1",
        "run_id": run_id,
        "trial_id": f"raw-{task_id.lower()}-{seed}",
        "arm_id": RAW_ARM,
        "task_id": task_id,
        "seed": seed,
        "started_at_utc": "2026-09-03T12:00:00Z",
        "ended_at_utc": "2026-09-03T12:00:01Z",
        "status": status,
        "failure_code": None if accepted else ("RAW_TRIAL_TIMEOUT" if timed_out else "ORACLE_REJECTED"),
        "timeout": timed_out,
        "prompt_digest": digest(10),
        "model_output_digest": None if timed_out else digest(11),
        "tool_call_digest": None if timed_out else digest(12),
        "candidate_digest": None if timed_out else digest(13),
        "workspace_before_digest": digest(14),
        "workspace_after_digest": digest(15),
        "oracle_report_digest": None if timed_out else digest(16),
        "evidence_status": "UNVERIFIED_INJECTED_DIGEST_CLAIMS",
        "oracle_verification_status": "UNVERIFIED_ADAPTER_CLAIM",
        "accounting_verification_status": "UNVERIFIED_ADAPTER_CLAIM",
        "metrics": metrics,
    }


def sley2_claim(run_id: str, task_id: str, seed: int, status: str, *, tokens: int, context: int, repairs: int) -> dict:
    accepted = status == "accepted"
    metrics = zero_metrics()
    metrics.update(
        {
            "strict_accepted_correctness": accepted,
            "accepted_correct_changes": int(accepted),
            "model_input_tokens": tokens,
            "model_output_tokens": tokens // 2,
            "total_observable_tokens": tokens + tokens // 2,
            "context_bytes": context,
            "repair_loops": repairs,
            "tool_calls": 3,
            "wall_time": 500,
        }
    )
    harness = status == "harness_failure"
    return {
        "accounting_verification_status": "UNVERIFIED_ADAPTER_CLAIM",
        "arm_id": ARM,
        "contract": CLAIM_CONTRACT,
        "endpoint_sha256": digest(9),
        "ended_at_utc": "2026-09-03T12:00:02Z",
        "evidence_status": "UNVERIFIED_INJECTED_DIGEST_CLAIMS",
        "exchange_digest": digest(4),
        "failure_code": None if accepted else ("SLEY2_TRIAL_INTERNAL_INVARIANT" if harness else "ORACLE_REJECTED"),
        "fixture_digest": digest(3),
        "handshake_id": None if harness else digest(7),
        "metrics": metrics,
        "model_output_digest": None if harness else digest(11),
        "oracle_report_digest": None if harness else digest(12),
        "oracle_verification_status": "UNVERIFIED_ADAPTER_CLAIM",
        "prompt_digest": digest(5),
        "report_digest": None if harness else digest(13),
        "run_id": run_id,
        "seed": seed,
        "started_at_utc": "2026-09-03T12:00:01Z",
        "status": status,
        "task_id": task_id,
        "timeout": False,
        "trace_head_digest": digest(8),
        "trace_record_count": 3,
        "trial_id": f"sley2-{task_id.lower()}-{seed}",
    }


def synthetic_arm(*, status: str, accepted: int, attempted: int, tokens: int, context_median: int, tokens_median: int, repairs_median: int, invalid_committed: int = 0, stale_accepted: int = 0, interventions: int = 0, by_class: dict | None = None) -> dict:
    return {
        "accepted": accepted,
        "accepted_change_tokens": ratio(tokens, accepted),
        "attempted": attempted,
        "by_class": by_class or {"repair": {"accepted": accepted, "attempted": attempted, "strict_correctness": ratio(accepted, attempted)}},
        "context_bytes": {"median": ratio(context_median, 1), "sum": context_median * attempted},
        "human_interventions": interventions,
        "invalid_committed_states": invalid_committed,
        "model_input_tokens_median": ratio(tokens_median, 1),
        "repair_loops": {"median": ratio(repairs_median, 1), "sum": repairs_median * attempted},
        "stale_candidates_incorrectly_accepted": stale_accepted,
        "status": status,
        "strict_correctness": ratio(accepted, attempted),
        "total_observable_tokens": tokens,
    }


class AccountingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.run = Path(self.temp.name) / "run"
        self.manifest = smoke_manifest("accounting-offline-001", "2026-09-03T12:00:00Z", "e" * 40, digest(3))
        self.manifest_digest = write_run_manifest(self.run, self.manifest)
        _, corpus = _plan_and_corpus()
        self.tasks = sorted(task["id"] for task in corpus["tasks"])

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_ratios_and_medians_are_exact(self) -> None:
        self.assertEqual(ratio(6, 4), {"denominator": 2, "numerator": 3})
        self.assertEqual(ratio(0, 5), {"denominator": 1, "numerator": 0})
        self.assertIsNone(ratio(3, 0))
        self.assertEqual(median([5, 1, 3]), {"denominator": 1, "numerator": 3})
        self.assertEqual(median([4, 1, 3, 2]), {"denominator": 2, "numerator": 5})
        self.assertEqual(median([2, 2]), {"denominator": 1, "numerator": 2})
        self.assertIsNone(median([]))
        with self.assertRaises(AccountingError) as error:
            median([1, True])
        self.assertEqual(error.exception.code, AccountingErrorCode.METRIC_INVALID)
        with self.assertRaises(AccountingError) as flt:
            report_digest({"contract": "x", "value": 1.5})
        self.assertEqual(flt.exception.code, AccountingErrorCode.FLOAT_FORBIDDEN)

    def test_every_attempt_stays_in_the_denominator(self) -> None:
        _, corpus = _plan_and_corpus()
        claims = [
            raw_claim("r", self.tasks[0], 1, "accepted", tokens=100, context=10, repairs=1),
            raw_claim("r", self.tasks[1], 1, "rejected", tokens=200, context=20, repairs=3),
            raw_claim("r", self.tasks[2], 1, "timeout", tokens=0, context=0, repairs=0),
            raw_claim("r", self.tasks[3], 1, "harness_failure", tokens=0, context=0, repairs=0),
        ]
        for index, claim in enumerate(claims):
            claim["record_digest"] = digest(20 + index)
        arm = arm_accounting(claims, self.manifest, corpus)
        self.assertEqual((arm["attempted"], arm["accepted"], arm["rejected"], arm["timeouts"], arm["harness_failures"]), (4, 1, 1, 1, 1))
        self.assertEqual(arm["strict_correctness"], {"denominator": 4, "numerator": 1})
        self.assertEqual(arm["total_observable_tokens"], 450)
        self.assertEqual(arm["accepted_change_tokens"], {"denominator": 1, "numerator": 450})
        self.assertEqual(arm["context_bytes"], {"median": {"denominator": 1, "numerator": 5}, "sum": 30})
        self.assertEqual(arm["repair_loops"]["median"], {"denominator": 2, "numerator": 1})
        self.assertEqual(arm["status"], "PARTIAL")
        self.assertEqual(arm["chain_head_digest"], digest(23))
        classes = {task["id"]: task["class"] for task in corpus["tasks"]}
        self.assertEqual(arm["by_class"][classes[self.tasks[0]]]["accepted"], 1)
        none_accepted = arm_accounting(claims[1:], self.manifest, corpus)
        self.assertIsNone(none_accepted["accepted_change_tokens"])
        self.assertEqual(none_accepted["accepted_change_tokens_reason"], "no_accepted_change")
        broken = dict(claims[0], status="won")
        with self.assertRaises(AccountingError):
            arm_accounting([broken], self.manifest, corpus)

    def test_thresholds_pass_fail_and_undetermined(self) -> None:
        plan, _ = _plan_and_corpus()
        legacy = synthetic_arm(status="COMPLETE", accepted=10, attempted=20, tokens=20_000, context_median=1_000, tokens_median=800, repairs_median=4)
        better = synthetic_arm(status="COMPLETE", accepted=16, attempted=20, tokens=16_000, context_median=500, tokens_median=700, repairs_median=2)
        results = evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: better}, plan)
        self.assertTrue(all(value["result"] == "PASS" for value in results.values()), results)
        failure = results["failure_rate_relative_reduction_percent_or_equal_correctness_act_reduction_percent"]
        self.assertEqual(failure["facts"]["failure_rate_reduction"], {"denominator": 5, "numerator": 3})
        self.assertEqual(failure["facts"]["act_reduction"], {"denominator": 2, "numerator": 1})
        worse = synthetic_arm(status="COMPLETE", accepted=8, attempted=20, tokens=20_000, context_median=900, tokens_median=790, repairs_median=4, invalid_committed=1, stale_accepted=1, interventions=1)
        results = evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: worse}, plan)
        self.assertTrue(all(value["result"] == "FAIL" for value in results.values()), results)
        # Equal correctness with a 30 percent ACT reduction passes the second clause.
        cheaper = synthetic_arm(status="COMPLETE", accepted=10, attempted=20, tokens=14_000, context_median=1_000, tokens_median=800, repairs_median=4)
        results = evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: cheaper}, plan)
        self.assertEqual(results["failure_rate_relative_reduction_percent_or_equal_correctness_act_reduction_percent"]["result"], "PASS")
        self.assertEqual(results["median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent"]["result"], "FAIL")
        partial = dict(better, status="PARTIAL")
        results = evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: partial}, plan)
        self.assertTrue(all(value["result"] == "UNDETERMINED" for value in results.values()))
        self.assertEqual(results["invalid_committed_states"]["reason"], {LEGACY_ARM: "COMPLETE", SLEY2_ARM: "PARTIAL"})
        results = evaluate_thresholds({LEGACY_ARM: NO_CHAIN, SLEY2_ARM: better}, plan)
        self.assertEqual(results["human_interventions_sley2"]["reason"][LEGACY_ARM], NO_CHAIN)

    def test_report_over_real_chains_is_exact_digested_and_fails_closed(self) -> None:
        empty = derive_report(self.run)
        self.assertEqual(empty["status"], "NO_TRIALS")
        self.assertEqual(empty["arms"], {RAW_ARM: NO_CHAIN, LEGACY_ARM: NO_CHAIN, SLEY2_ARM: NO_CHAIN})
        run_id = self.manifest["run_id"]
        for index, task_id in enumerate(self.tasks):
            status = "accepted" if index % 3 == 0 else ("timeout" if index % 3 == 1 else "rejected")
            append_trial_digest_claim(self.run, raw_claim(run_id, task_id, 1, status, tokens=100 + index, context=1_000 + index, repairs=index % 4))
        for index, task_id in enumerate(self.tasks[:-1]):
            status = "accepted" if index % 2 == 0 else "harness_failure"
            append_trial_claim(self.run, sley2_claim(run_id, task_id, 1, status, tokens=50 + index, context=200 + index, repairs=index % 2))
        report = derive_report(self.run)
        verify_report(report)
        self.assertEqual(report["status"], "PARTIAL")
        raw = report["arms"][RAW_ARM]
        sley2 = report["arms"][SLEY2_ARM]
        self.assertEqual(raw["status"], "COMPLETE")
        self.assertEqual(sley2["status"], "PARTIAL")
        self.assertEqual(report["arms"][LEGACY_ARM], NO_CHAIN)
        self.assertEqual(raw["attempted"], len(self.tasks))
        self.assertEqual(raw["accepted"] + raw["rejected"] + raw["timeouts"], len(self.tasks))
        self.assertEqual(sley2["attempted"], len(self.tasks) - 1)
        self.assertEqual(sley2["harness_failures"], (len(self.tasks) - 1) // 2)
        total = sum(150 + index + index // 2 for index in range(len(self.tasks)))
        self.assertEqual(raw["total_observable_tokens"], total)
        self.assertEqual(Fraction(raw["accepted_change_tokens"]["numerator"], raw["accepted_change_tokens"]["denominator"]), Fraction(total, raw["accepted"]))
        self.assertTrue(all(value["result"] == "UNDETERMINED" for value in report["thresholds"].values()))
        self.assertEqual(report["evidence_status"], "DERIVED_FROM_UNVERIFIED_CLAIMS")
        self.assertEqual(report["report_digest"], report_digest(report))
        again = derive_report(self.run)
        self.assertEqual(again, report)
        with self.assertRaises(AccountingError) as incomplete:
            derive_report(self.run, require_complete=True)
        self.assertEqual(incomplete.exception.code, AccountingErrorCode.INCOMPLETE)
        tampered = dict(report, status="COMPLETE")
        with self.assertRaises(AccountingError) as bad:
            verify_report(tampered)
        self.assertEqual(bad.exception.code, AccountingErrorCode.REPORT_INVALID)
        chain = self.run / "sley2" / "claims.jsonl"
        chain.write_bytes(chain.read_bytes().replace(b'"accepted"', b'"rejected"', 1))
        with self.assertRaises(AccountingError) as broken:
            derive_report(self.run)
        self.assertEqual(broken.exception.code, AccountingErrorCode.CHAIN_INVALID)
        with self.assertRaises(AccountingError) as missing:
            derive_report(self.run / "nope")
        self.assertEqual(missing.exception.code, AccountingErrorCode.RUN_INVALID)
        self.assertEqual(hashlib.sha256(b"").hexdigest()[:4], "e3b0")


if __name__ == "__main__":
    unittest.main()
