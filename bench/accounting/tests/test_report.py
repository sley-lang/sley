"""Offline tests for S20-630 accounting: exact arithmetic, denominators, thresholds, reports."""

from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from fractions import Fraction
from pathlib import Path
from unittest import mock

from bench.accounting.report import (
    ARM_VERIFIERS,
    EVIDENCE_MIXED,
    EVIDENCE_UNVERIFIED,
    LEGACY_ARM,
    NO_CHAIN,
    RAW_ARM,
    SLEY2_ARM,
    AccountingError,
    AccountingErrorCode,
    arm_accounting,
    derive_evidence_status,
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
from bench.sley2.runner import ARM, CLAIM_CONTRACT, append_trial_claim, arm_affordances_digest, smoke_manifest


def digest(byte: int) -> str:
    return f"{byte:02x}" * 32


def zero_metrics() -> dict:
    plan = json.loads(PLAN_PATH.read_text(encoding="utf-8"))
    values = {name: 0 for name in plan["metrics"]}
    values["attempted_tasks"] = 1
    values["strict_accepted_correctness"] = False
    values["accepted_change_tokens"] = None
    return values


def raw_claim(run_id: str, task_id: str, seed: int, status: str, *, tokens: int, context: int, repairs: int, arm_id: str = RAW_ARM, evidence: str = "UNVERIFIED_INJECTED_DIGEST_CLAIMS") -> dict:
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
        "trial_id": f"legacy-{task_id.lower()}-{seed}" if arm_id == LEGACY_ARM else f"raw-{task_id.lower()}-{seed}",
        "arm_id": arm_id,
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
        "evidence_status": evidence,
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
        "arm_affordances_digest": arm_affordances_digest(),
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


def complete_arm(builder, run_id: str, tasks: list[str], manifest: dict, corpus: dict, *, accepted: int, tokens, context, repairs, invalid_total: int = 0, invalid_bad: int = 0, stale_total: int = 0, stale_bad: int = 0) -> dict:
    """A COMPLETE arm built by the real arm_accounting over synthetic claims."""

    def pick(value, index: int) -> int:
        return value[index] if isinstance(value, list) else value

    claims = []
    for index, task_id in enumerate(tasks):
        claim = builder(run_id, task_id, 1, "accepted" if index < accepted else "rejected", tokens=pick(tokens, index), context=pick(context, index), repairs=pick(repairs, index))
        claim["record_digest"] = digest(40 + (index % 200))
        claim["metrics"]["invalid_candidates"] = 1 if index < invalid_total else 0
        claim["metrics"]["invalid_committed_states"] = 1 if index < invalid_bad else 0
        claim["metrics"]["stale_candidates"] = 1 if index < stale_total else 0
        claim["metrics"]["stale_candidates_incorrectly_accepted"] = 1 if index < stale_bad else 0
        claims.append(claim)
    return arm_accounting(claims, manifest, corpus)


class AccountingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.run = Path(self.temp.name) / "run"
        self.manifest = smoke_manifest("accounting-offline-001", "2026-09-03T12:00:00Z", "e" * 40, digest(3))
        self.manifest_digest = write_run_manifest(self.run, self.manifest)
        _, corpus = _plan_and_corpus()
        self.corpus = corpus
        self.tasks = sorted(task["id"] for task in corpus["tasks"])
        self.plan, _ = _plan_and_corpus()

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
        claims = [
            raw_claim("r", self.tasks[0], 1, "accepted", tokens=100, context=10, repairs=1),
            raw_claim("r", self.tasks[1], 1, "rejected", tokens=200, context=20, repairs=3),
            raw_claim("r", self.tasks[2], 1, "timeout", tokens=0, context=0, repairs=0),
            raw_claim("r", self.tasks[3], 1, "harness_failure", tokens=0, context=0, repairs=0),
        ]
        for index, claim in enumerate(claims):
            claim["record_digest"] = digest(20 + index)
        arm = arm_accounting(claims, self.manifest, self.corpus)
        self.assertEqual((arm["attempted"], arm["accepted"], arm["rejected"], arm["timeouts"], arm["harness_failures"]), (4, 1, 1, 1, 1))
        self.assertEqual(arm["strict_correctness"], {"denominator": 4, "numerator": 1})
        self.assertEqual(arm["total_observable_tokens"], 450)
        self.assertEqual(arm["accepted_change_tokens"], {"denominator": 1, "numerator": 450})
        self.assertEqual(arm["context_bytes"]["median"], {"denominator": 1, "numerator": 5})
        self.assertEqual(arm["context_bytes"]["sum"], 30)
        # The companion median excludes the harness failure; the threshold
        # basis stays every attempt and names itself.
        self.assertEqual(arm["context_bytes"]["median_non_harness_failure"], {"denominator": 1, "numerator": 10})
        self.assertEqual(arm["median_excluded_harness_failures"], 1)
        self.assertEqual(arm["repair_loops"]["median"], {"denominator": 2, "numerator": 1})
        self.assertTrue(arm["every_attempt_in_denominator"])
        self.assertEqual(arm["status"], "PARTIAL")
        self.assertEqual(arm["chain_head_digest"], digest(23))
        self.assertEqual(arm["claim_statuses"]["evidence_status"], ["UNVERIFIED_INJECTED_DIGEST_CLAIMS"])
        self.assertEqual(sum(slot["attempted"] for slot in arm["by_seed"].values()), 4)
        classes = {task["id"]: task["class"] for task in self.corpus["tasks"]}
        self.assertEqual(arm["by_class"][classes[self.tasks[0]]]["accepted"], 1)
        self.assertIn("collateral_semantic_changes", arm["by_class"][classes[self.tasks[0]]])
        for name in ("entities_inspected", "relationships_inspected", "files_inspected", "peak_memory", "canonical_storage_bytes", "pack_bytes", "execution_latency"):
            self.assertIn(name, arm)
        none_accepted = arm_accounting(claims[1:], self.manifest, self.corpus)
        self.assertIsNone(none_accepted["accepted_change_tokens"])
        self.assertEqual(none_accepted["accepted_change_tokens_reason"], "no_accepted_change")
        broken = dict(claims[0], status="won")
        with self.assertRaises(AccountingError):
            arm_accounting([broken], self.manifest, self.corpus)
        # A duplicated trial slot fails the local invariant, not the metric set.
        with self.assertRaises(AccountingError) as duplicate:
            arm_accounting([claims[0], claims[0]], self.manifest, self.corpus)
        self.assertEqual(duplicate.exception.code, AccountingErrorCode.INTERNAL_INVARIANT)
        # A pre-derived per-claim ACT can never mix with the derived one.
        prederived = json.loads(json.dumps(claims[0]))
        prederived["metrics"]["accepted_change_tokens"] = 7
        with self.assertRaises(AccountingError) as act:
            arm_accounting([prederived], self.manifest, self.corpus)
        self.assertEqual(act.exception.code, AccountingErrorCode.METRIC_INVALID)

    def test_thresholds_pass_fail_and_undetermined_on_real_arm_outputs(self) -> None:
        run_id = "arm-seam-001"
        legacy = complete_arm(raw_claim, run_id, self.tasks, self.manifest, self.corpus, accepted=9, tokens=1_000, context=1_000, repairs=4)
        better = complete_arm(sley2_claim, run_id, self.tasks, self.manifest, self.corpus, accepted=12, tokens=700, context=500, repairs=2, invalid_total=2, stale_total=3)
        self.assertEqual((legacy["status"], better["status"]), ("COMPLETE", "COMPLETE"))
        results = evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: better}, self.plan, self.manifest, EVIDENCE_UNVERIFIED)
        evaluated = {name: value for name, value in results.items() if value["result"] != "NOT_EVALUATED"}
        self.assertTrue(all(value["result"] == "PASS" for value in evaluated.values()), results)
        failure = results["failure_rate_relative_reduction_percent_or_equal_correctness_act_reduction_percent"]
        self.assertEqual(failure["facts"]["failure_rate_reduction"], {"denominator": 2, "numerator": 1})
        self.assertEqual(failure["facts"]["failure_rate"][LEGACY_ARM], {"denominator": 5, "numerator": 2})
        self.assertEqual(results["invalid_candidates_leave_state_unchanged_percent"]["facts"]["good_ratio"], {"denominator": 1, "numerator": 1})
        self.assertEqual(results["median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent"]["facts"]["median_basis"], "ALL_ATTEMPTS")
        self.assertEqual(results["median_repair_loop_reduction_percent_or_accepted_changes_increase_percent"]["facts"]["action_budget"], 100)
        self.assertEqual(results["critical_class_correctness_not_lower_than_legacy"]["facts"]["critical_class_selection"], "ALL_CLASSES")
        for name in ("section_22_1_no_required_check_bypassed", "section_22_4_collateral_semantic_comparison", "section_22_4_mutation_reconstructability"):
            self.assertEqual(results[name]["result"], "NOT_EVALUATED", name)

        worse = complete_arm(sley2_claim, run_id, self.tasks, self.manifest, self.corpus, accepted=6, tokens=1_000, context=900, repairs=4, invalid_total=2, invalid_bad=1, stale_total=1, stale_bad=1)
        results = evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: worse}, self.plan, self.manifest, EVIDENCE_UNVERIFIED)
        self.assertEqual(results["failure_rate_relative_reduction_percent_or_equal_correctness_act_reduction_percent"]["result"], "FAIL")
        self.assertEqual(results["median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent"]["result"], "FAIL")
        self.assertEqual(results["invalid_candidates_leave_state_unchanged_percent"]["result"], "FAIL")
        self.assertEqual(results["invalid_committed_states"]["result"], "FAIL")
        # Neither context metric regressed, so the cap row passes beside the
        # failed threshold: it measures the regressions, never the verdict.
        cap = results["other_context_metric_max_regression_percent"]
        self.assertEqual(cap["result"], "PASS")
        self.assertEqual(cap["facts"]["context_bytes_reduction"], {"denominator": 10, "numerator": 1})
        # Equal correctness with a lower token SUM at equal medians passes the
        # ACT clause while the context row still fails on measured medians.
        cheap_tokens = [100] * 7 + [800] * 8
        cheaper = complete_arm(sley2_claim, run_id, self.tasks, self.manifest, self.corpus, accepted=9, tokens=cheap_tokens, context=1_000, repairs=4)
        results = evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: cheaper}, self.plan, self.manifest, EVIDENCE_UNVERIFIED)
        self.assertEqual(results["failure_rate_relative_reduction_percent_or_equal_correctness_act_reduction_percent"]["result"], "PASS")
        self.assertEqual(results["median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent"]["result"], "FAIL")
        self.assertEqual(results["other_context_metric_max_regression_percent"]["result"], "PASS")
        partial = dict(better, status="PARTIAL")
        results = evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: partial}, self.plan, self.manifest, EVIDENCE_UNVERIFIED)
        self.assertTrue(all(value["result"] in ("UNDETERMINED", "NOT_EVALUATED") for value in results.values()))
        self.assertEqual(results["invalid_committed_states"]["reason"], {LEGACY_ARM: "COMPLETE", SLEY2_ARM: "PARTIAL"})
        results = evaluate_thresholds({LEGACY_ARM: NO_CHAIN, SLEY2_ARM: better}, self.plan, self.manifest, EVIDENCE_UNVERIFIED)
        self.assertEqual(results["human_interventions_sley2"]["reason"][LEGACY_ARM], NO_CHAIN)

    def test_undefined_regressions_and_empty_denominators_are_named_nulls(self) -> None:
        run_id = "nulls-001"
        legacy = complete_arm(raw_claim, run_id, self.tasks, self.manifest, self.corpus, accepted=9, tokens=1_000, context=0, repairs=4)
        sley2 = complete_arm(sley2_claim, run_id, self.tasks, self.manifest, self.corpus, accepted=9, tokens=600, context=500, repairs=4)
        results = evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: sley2}, self.plan, self.manifest, EVIDENCE_UNVERIFIED)
        main = results["median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent"]
        # The tokens leg meets 30 percent but the context regression is
        # undefined, so the leg and the row stay UNDETERMINED, never PASS.
        self.assertEqual(main["result"], "UNDETERMINED")
        cap = results["other_context_metric_max_regression_percent"]
        self.assertEqual(cap["result"], "UNDETERMINED")
        self.assertIn("context_bytes", cap["reason"]["reason"])
        # Zero invalid candidates is a zero denominator, never a vacuous PASS.
        percent = results["invalid_candidates_leave_state_unchanged_percent"]
        self.assertEqual(percent["result"], "UNDETERMINED")
        self.assertEqual(percent["reason"]["reason"], "no_invalid_candidates")

    def test_threshold_coverage_and_criticality_flag_fail_closed(self) -> None:
        run_id = "coverage-001"
        legacy = complete_arm(raw_claim, run_id, self.tasks, self.manifest, self.corpus, accepted=9, tokens=1_000, context=1_000, repairs=4)
        better = complete_arm(sley2_claim, run_id, self.tasks, self.manifest, self.corpus, accepted=12, tokens=700, context=500, repairs=2)
        extra = json.loads(json.dumps(self.plan))
        extra["thresholds"]["invented_threshold"] = 0
        with self.assertRaises(AccountingError) as coverage:
            evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: better}, extra, self.manifest, EVIDENCE_UNVERIFIED)
        self.assertEqual(coverage.exception.code, AccountingErrorCode.METRIC_INVALID)
        relaxed = json.loads(json.dumps(self.plan))
        relaxed["thresholds"]["critical_class_correctness_not_lower_than_legacy"] = False
        with self.assertRaises(AccountingError) as critical:
            evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: better}, relaxed, self.manifest, EVIDENCE_UNVERIFIED)
        self.assertEqual(critical.exception.code, AccountingErrorCode.METRIC_INVALID)
        per_arm = dict(self.manifest, action_budgets={"sley_2_0": 50})
        with self.assertRaises(AccountingError) as budget:
            evaluate_thresholds({LEGACY_ARM: legacy, SLEY2_ARM: better}, self.plan, per_arm, EVIDENCE_UNVERIFIED)
        self.assertEqual(budget.exception.code, AccountingErrorCode.RUN_INVALID)

    def test_legacy_registry_gate_and_evidence_derivation(self) -> None:
        from bench.accounting.report import _arm_claims

        self.assertEqual(_arm_claims(self.run, LEGACY_ARM, "FROZEN_LEGACY_ARTIFACT"), (None, None))
        legacy_chain = self.run / "legacy"
        legacy_chain.mkdir(parents=True)
        (legacy_chain / "claims.jsonl").write_text("{}\n", encoding="utf-8")
        with self.assertRaises(AccountingError) as no_verifier:
            _arm_claims(self.run, LEGACY_ARM, "FROZEN_LEGACY_ARTIFACT")
        self.assertEqual(no_verifier.exception.code, AccountingErrorCode.CHAIN_INVALID)
        self.assertEqual(derive_evidence_status({"evidence_status": [], "oracle_verification_status": [], "accounting_verification_status": []}), EVIDENCE_UNVERIFIED)
        mixed = {"evidence_status": ["UNVERIFIED_INJECTED_DIGEST_CLAIMS", "VERIFIED_MANUALLY"], "oracle_verification_status": ["UNVERIFIED_ADAPTER_CLAIM"], "accounting_verification_status": ["UNVERIFIED_ADAPTER_CLAIM"]}
        self.assertEqual(derive_evidence_status(mixed), EVIDENCE_MIXED)

    def test_complete_report_through_derive_over_two_full_chains(self) -> None:
        run_id = self.manifest["run_id"]
        legacy_claims = []
        for index, task_id in enumerate(self.tasks):
            claim = raw_claim(run_id, task_id, 1, "accepted" if index < 9 else "rejected", tokens=1_000, context=1_000, repairs=4, arm_id=LEGACY_ARM)
            claim["record_digest"] = digest(60 + index)
            legacy_claims.append(claim)
        def legacy_loader(_run: Path) -> list[dict]:
            return legacy_claims

        with mock.patch.dict(ARM_VERIFIERS, {LEGACY_ARM: legacy_loader}):
            for index, task_id in enumerate(self.tasks):
                append_trial_digest_claim(self.run, raw_claim(run_id, task_id, 1, "accepted" if index % 3 == 0 else "rejected", tokens=100 + index, context=1_000 + index, repairs=index % 4))
            for index, task_id in enumerate(self.tasks):
                append_trial_claim(self.run, sley2_claim(run_id, task_id, 1, "accepted" if index < 12 else "rejected", tokens=700, context=500, repairs=2))
            report = derive_report(self.run)
            again = derive_report(self.run)
        verify_report(report)
        self.assertEqual(again, report)
        self.assertEqual(report["status"], "COMPLETE")
        self.assertEqual(report["arms"][LEGACY_ARM]["chain_verifier"], "legacy_loader")
        self.assertEqual(report["arm_fixture_status"][LEGACY_ARM], "FROZEN_LEGACY_ARTIFACT")
        self.assertEqual(report["evidence_status"], EVIDENCE_UNVERIFIED)
        self.assertEqual(report["benchmark_plan_digest"], self.manifest["benchmark_plan_digest"])
        self.assertEqual(report["corpus_digest"], self.manifest["corpus_digest"])
        self.assertEqual(report["thresholds"]["median_context_bytes_reduction_percent_or_model_input_tokens_reduction_percent"]["result"], "PASS")
        self.assertTrue(all(row.get("evidence_status") == EVIDENCE_UNVERIFIED for row in report["thresholds"].values()))
        forged = dict(report, evidence_status=EVIDENCE_MIXED)
        with self.assertRaises(AccountingError) as bad:
            verify_report(forged)
        self.assertEqual(bad.exception.code, AccountingErrorCode.REPORT_INVALID)

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
        self.assertEqual(raw["chain_verifier"], "verify_digest_claim_directory")
        self.assertEqual(sley2["chain_verifier"], "verify_trial_claims")
        self.assertEqual(report["arm_fixture_status"], {RAW_ARM: "PENDING", LEGACY_ARM: "FROZEN_LEGACY_ARTIFACT", SLEY2_ARM: "PENDING"})
        self.assertEqual(raw["attempted"], len(self.tasks))
        self.assertEqual(raw["accepted"] + raw["rejected"] + raw["timeouts"], len(self.tasks))
        self.assertEqual(sley2["attempted"], len(self.tasks) - 1)
        self.assertEqual(sley2["harness_failures"], (len(self.tasks) - 1) // 2)
        total = sum(150 + index + index // 2 for index in range(len(self.tasks)))
        self.assertEqual(raw["total_observable_tokens"], total)
        self.assertEqual(Fraction(raw["accepted_change_tokens"]["numerator"], raw["accepted_change_tokens"]["denominator"]), Fraction(total, raw["accepted"]))
        self.assertEqual(len(report["thresholds"]), len(self.plan["thresholds"]) + 3)
        self.assertTrue(all(value["result"] in ("UNDETERMINED", "NOT_EVALUATED") for value in report["thresholds"].values()))
        self.assertEqual(report["evidence_status"], "DERIVED_FROM_UNVERIFIED_CLAIMS")
        self.assertEqual(report["report_digest"], report_digest(report))
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
