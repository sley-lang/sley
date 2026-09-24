"""Live-input accounting (contract section 11) over SYNTHETIC fixtures.

Every run below is a synthetic, test-only fixture built in a temporary
directory by ``synthetic_live`` (run ids ``synthetic-fixture-*``); none is a
model trial. The PILOT test reads the retained real pilot run read-only
when it exists on this host.
"""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from fractions import Fraction
from pathlib import Path
from unittest import mock

from bench.accounting.live import derive_live_report
from bench.accounting.report import (
    AccountingError,
    evaluate_section_22_rows,
    verify_report,
)
from bench.accounting.tests.synthetic_live import CONTEXT_BUDGET, Slot, build_run
from bench.live.artifacts import ArtifactStore

PILOT_RUN = Path(os.environ.get(
    "SLEY2_PILOT_RUN",
    "/home/gfarch/Work/checkpoints/sley2-campaign-runs/s20-640-pilot-20260924-small"))
STALE_INDEX = 9      # S2B-STALE-001
COLLATERAL_INDEX = 7  # S2B-DEAD-001


def fraction(value) -> Fraction:
    return Fraction(value["numerator"], value["denominator"])


def good_plan(arm: str, index: int, task: str) -> Slot:
    """Sley 2 beats legacy on every plan row; raw is mediocre."""

    if index == 13:
        # Every arm: one timeout, so timeouts sit in every denominator.
        return Slot(status="timeout", code="LIVE_PROVIDER_TIMEOUT", stream="failed", wall_time=600_001)
    if index == 14:
        # Every arm: one attempt retained over the frozen context budget.
        return Slot(status="harness_failure", code="LIVE_PROVIDER_BUDGET_EXCEEDED",
                    input_tokens=CONTEXT_BUDGET + 1)
    if index == 12:
        return Slot(status="harness_failure", code="LIVE_PROVIDER_EXIT_NONZERO", stream="failed")
    if arm == "sley_1_2_0":
        accepted = index < 5
        return Slot(status="accepted" if accepted else "rejected",
                    collateral=0 if accepted or index != COLLATERAL_INDEX else 1,
                    input_tokens=1_000, context_bytes=1_000, repair_loops=4)
    if arm == "sley_2_0":
        accepted = index < 10
        return Slot(status="accepted" if accepted else "rejected", input_tokens=600,
                    context_bytes=500, repair_loops=2,
                    invalid_candidates=0 if accepted else 1,
                    stale=1 if index == STALE_INDEX else 0)
    return Slot(status="accepted" if index < 3 else "rejected")


def bad_plan(arm: str, index: int, task: str) -> Slot:
    """Sley 2 is worse than legacy on every plan row."""

    if arm == "sley_1_2_0":
        return Slot(status="accepted" if index < 10 else "rejected", input_tokens=1_000,
                    context_bytes=1_000, repair_loops=1)
    if arm == "sley_2_0":
        accepted = index < 5
        return Slot(status="accepted" if accepted else "rejected", input_tokens=1_300,
                    context_bytes=700, repair_loops=3, invalid_candidates=0 if accepted else 1,
                    invalid_committed=1 if index == 6 else 0,
                    stale=1 if index == STALE_INDEX else 0,
                    stale_bad=1 if index == STALE_INDEX else 0,
                    collateral=1 if index == COLLATERAL_INDEX else 0, human=1 if index == 0 else 0)
    return Slot(status="rejected")


class LiveAccountingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temporary = tempfile.TemporaryDirectory(prefix="synthetic-fixture-", dir=os.environ.get("TMPDIR"))
        root = Path(cls.temporary.name)
        cls.good = derive_live_report(build_run(root, "good", good_plan))
        cls.bad = derive_live_report(build_run(root, "bad", bad_plan))
        cls.root = root

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temporary.cleanup()

    def test_every_attempt_in_every_denominator(self) -> None:
        for arm in ("raw_files", "sley_1_2_0", "sley_2_0"):
            accounting = self.good["arms"][arm]
            self.assertEqual(accounting["status"], "COMPLETE")
            self.assertEqual(accounting["attempted"], 15)
            self.assertEqual(accounting["timeouts"], 1)
            self.assertEqual(accounting["harness_failures"], 2)
            self.assertEqual(accounting["attempted"], accounting["accepted"] + accounting["rejected"]
                             + accounting["timeouts"] + accounting["harness_failures"])
            self.assertTrue(accounting["every_attempt_in_denominator"])
            self.assertEqual(fraction(accounting["strict_correctness"]),
                             Fraction(accounting["accepted"], 15))
            # Medians run over all 15 attempts (the unjudged zero-usage
            # attempts included); companions drop the two harness failures.
            self.assertEqual(accounting["median_excluded_harness_failures"], 2)
        sley2 = self.good["arms"]["sley_2_0"]
        # The over-budget attempt's observed tokens are counted, not dropped:
        # 10 accepted + 2 rejected at 600/100, the retained 100001/100, and
        # zero for the failed-stream timeout and exit-nonzero attempts.
        self.assertEqual(sley2["total_observable_tokens"], 12 * 700 + CONTEXT_BUDGET + 1 + 100)
        self.assertEqual(fraction(sley2["accepted_change_tokens"]), Fraction(12 * 700 + CONTEXT_BUDGET + 101, 10))
        self.assertEqual(fraction(sley2["model_input_tokens"]["median"]), 600)
        self.assertEqual(sley2["by_seed"]["7"]["attempted"], 15)
        self.assertEqual(sum(slot["attempted"] for slot in sley2["by_class"].values()), 15)
        self.assertTrue(self.good["counts_toward_succession"])
        self.assertEqual(self.good["evidence_status"], "DERIVED_FROM_VERIFIED_CLAIMS")
        verify_report(self.good)

    def test_every_plan_threshold_passes_and_fails(self) -> None:
        plan_rows = [name for name in self.good["thresholds"] if not name.startswith("section_22_")]
        self.assertEqual(len(plan_rows), 10)
        for name in plan_rows:
            self.assertEqual(self.good["thresholds"][name]["result"], "PASS", name)
            self.assertEqual(self.bad["thresholds"][name]["result"], "FAIL", name)

    def test_section_22_rows_from_live_claims(self) -> None:
        good = self.good["thresholds"]
        # The oracle report's two booleans are runner constants, so accepted
        # mutations carry no measurement and the rows stay UNDETERMINED.
        for name in ("section_22_1_no_required_check_bypassed", "section_22_4_mutation_reconstructability"):
            self.assertEqual(good[name]["result"], "UNDETERMINED")
            self.assertEqual(good[name]["reason"]["reason"], "not_measured")
            self.assertEqual(good[name]["reason"]["measurement_bases"], ["RUNNER_CONSTANT_NOT_A_MEASUREMENT"])
        collateral = good["section_22_4_collateral_semantic_comparison"]
        self.assertEqual(collateral["result"], "UNDETERMINED")
        self.assertEqual(collateral["reason"]["reason"], "multi_entity_class_selection_not_frozen")
        self.assertEqual(collateral["reason"]["classes_strictly_lower"], ["dead_path_removal"])
        bad = self.bad["thresholds"]["section_22_4_collateral_semantic_comparison"]
        self.assertEqual((bad["result"], bad["facts"]["failed_leg"]), ("FAIL", "no_worse"))

    def test_collateral_row_fails_when_no_class_is_strictly_lower(self) -> None:
        def equal(arm: str, index: int, task: str) -> Slot:
            return Slot(status="accepted" if index < 5 else "rejected")

        report = derive_live_report(build_run(self.root, "equal", equal))
        row = report["thresholds"]["section_22_4_collateral_semantic_comparison"]
        self.assertEqual((row["result"], row["facts"]["failed_leg"]), ("FAIL", "strictly_lower_in_no_class"))

    def test_measured_section_22_values_decide_the_rows(self) -> None:
        arms = {"sley_2_0": {"status": "COMPLETE"}, "sley_1_2_0": "NO_CLAIM_CHAIN"}

        def claims(values: list) -> list[dict]:
            return [{"status": "accepted", "section_22": {"basis": "MEASURED", "required_check_bypassed": value,
                                                          "mutation_reconstructable": None if value is None else not value}}
                    for value in values]

        rows = evaluate_section_22_rows(arms, claims([False, False]), "X")
        self.assertEqual(rows["section_22_1_no_required_check_bypassed"]["result"], "PASS")
        self.assertEqual(rows["section_22_4_mutation_reconstructability"]["result"], "PASS")
        rows = evaluate_section_22_rows(arms, claims([False, True, None]), "X")
        self.assertEqual(rows["section_22_1_no_required_check_bypassed"]["result"], "FAIL")
        self.assertEqual(rows["section_22_4_mutation_reconstructability"]["result"], "FAIL")
        rows = evaluate_section_22_rows(arms, claims([False, None]), "X")
        self.assertEqual(rows["section_22_1_no_required_check_bypassed"]["result"], "UNDETERMINED")
        rows = evaluate_section_22_rows(arms, [{"status": "rejected", "section_22": {}}], "X")
        self.assertEqual(rows["section_22_1_no_required_check_bypassed"]["reason"]["reason"], "no_accepted_mutation")
        self.assertEqual(rows["section_22_4_collateral_semantic_comparison"]["result"], "UNDETERMINED")
        with self.assertRaises(AccountingError):
            evaluate_section_22_rows(arms, [{"status": "accepted", "section_22": {"required_check_bypassed": 0,
                                                                                 "mutation_reconstructable": True}}], "X")

    def test_partial_arms_leave_every_row_undetermined(self) -> None:
        def partial(arm: str, index: int, task: str) -> Slot | None:
            return None if index == 0 and arm == "sley_2_0" else Slot(status="accepted")

        report = derive_live_report(build_run(self.root, "partial", partial))
        self.assertEqual(report["status"], "PARTIAL")
        self.assertEqual(report["arms"]["sley_2_0"]["status"], "PARTIAL")
        for name, row in report["thresholds"].items():
            self.assertEqual(row["result"], "UNDETERMINED", name)
        with self.assertRaises(AccountingError) as caught:
            derive_live_report(self.root / "partial", require_complete=True)
        self.assertEqual(caught.exception.symbol, "ACCOUNTING_INCOMPLETE")

    def test_unjudged_usage_without_a_completed_stream_fails_closed(self) -> None:
        def forged(arm: str, index: int, task: str) -> Slot | None:
            if index:
                return None
            # A failed provider stream that nonetheless claims usage.
            return Slot(status="harness_failure", code="LIVE_PROVIDER_EXIT_NONZERO", stream="failed",
                        extra={"model_input_tokens": 5, "total_observable_tokens": 5})

        run = build_run(self.root, "forged-usage", forged)
        with self.assertRaises(AccountingError) as caught:
            derive_live_report(run)
        self.assertEqual(caught.exception.symbol, "ACCOUNTING_CHAIN_INVALID")

    def test_starting_state_and_legacy_artifact_binding(self) -> None:
        def one(arm: str, index: int, task: str) -> Slot | None:
            return Slot(status="rejected") if index == 1 else None

        run = build_run(self.root, "start-binding", one)
        self.assertEqual(derive_live_report(run)["arms"]["sley_1_2_0"]["chain_verifier"],
                         "verify_live_legacy_claims")
        from bench.legacy import runner as legacy_runner

        drifted = legacy_runner.FROZEN_CONTRACT.__class__(**{
            **legacy_runner.FROZEN_CONTRACT.__dict__, "artifact_sha256": "0" * 64})
        with mock.patch.object(legacy_runner, "FROZEN_CONTRACT", drifted):
            with self.assertRaises(AccountingError) as caught:
                derive_live_report(run)
        self.assertIn("sley_1_2_0", str(caught.exception))
        with mock.patch("bench.live.live_claims._initial_snapshot", return_value=b"not the frozen start"):
            with self.assertRaises(AccountingError) as caught:
                derive_live_report(run)
        self.assertIn("starting state", str(caught.exception))

    def test_pilot_label_never_counts(self) -> None:
        run = build_run(self.root, "pilot-label", lambda arm, index, task: Slot(status="accepted"), label="PILOT")
        report = derive_live_report(run)
        self.assertEqual(report["status"], "COMPLETE")
        self.assertFalse(report["counts_toward_succession"])
        with self.assertRaises(AccountingError):
            derive_live_report(run, require_campaign=True)


@unittest.skipUnless((PILOT_RUN / "attempts.jsonl").is_file(), "retained PILOT run not on this host")
class RetainedPilotTests(unittest.TestCase):
    """The nine real PILOT records of 2026-09-24, read-only."""

    def test_pilot_records_are_harness_failures_that_never_count(self) -> None:
        before = (PILOT_RUN / "attempts.jsonl").read_bytes()
        report = derive_live_report(PILOT_RUN)
        self.assertEqual((PILOT_RUN / "attempts.jsonl").read_bytes(), before)
        self.assertFalse(report["counts_toward_succession"])
        self.assertEqual(report["input"]["label"], "PILOT")
        self.assertEqual(report["status"], "PARTIAL")
        for arm in ("raw_files", "sley_1_2_0", "sley_2_0"):
            accounting = report["arms"][arm]
            self.assertEqual((accounting["attempted"], accounting["harness_failures"], accounting["accepted"]), (3, 3, 0))
            self.assertEqual(accounting["total_observable_tokens"], 0)
        self.assertEqual(report["claim_statuses"]["accounting_verification_status"],
                         ["VERIFIED_NO_PROVIDER_USAGE_REPORTED"])
        for name, row in report["thresholds"].items():
            self.assertEqual(row["result"], "UNDETERMINED", name)
        with self.assertRaises(AccountingError):
            derive_live_report(PILOT_RUN, require_campaign=True)
        events = ArtifactStore(PILOT_RUN / "artifacts")
        for line in before.splitlines():
            record = json.loads(line)
            self.assertIn(b"usage limit", events.read(record["artifacts"]["provider_events_sha256"]))


if __name__ == "__main__":
    unittest.main()
