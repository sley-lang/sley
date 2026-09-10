#!/usr/bin/env python3
"""Negative tests for the S20-430 current-delta-review record gate.

`check_cli_contract.py` already refuses a stale revision carried forward
with PASS, a missing record, and a frozen status with PENDING judgments
(Vulcan R6-P2-3); these cases pin that behavior so the review record can
never silently satisfy the freeze gate. The valid control passes before
and after. Nothing here mutates the repository under test.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def load_module(name: str, relative: str):
    path = ROOT / relative
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


CHECKER = load_module("cli_contract_checker", "scripts/check_cli_contract.py")


def review_problems(review, status: str) -> list:
    section = {"current_delta_review": review} if review is not None else {}
    problems: list = []
    CHECKER.check_current_delta_review(section, CHECKER.SPEC_REVISION, status, problems)
    return [item for item in problems if item.startswith("review:")]


def passing_review(revision: int = 6) -> dict:
    return {
        "contract_revision": revision,
        "ariadne": "PASS",
        "nabu": "PASS",
        "vulcan": "PASS",
    }


class CurrentDeltaReviewCases(unittest.TestCase):
    def test_stale_revision_with_pass_carried_forward_refused(self) -> None:
        problems = review_problems(passing_review(revision=5), CHECKER.REVIEW_PENDING_STATUS)
        self.assertIn("review:current-delta-revision:5", problems)

    def test_missing_record_refused(self) -> None:
        problems = review_problems(None, CHECKER.REVIEW_PENDING_STATUS)
        self.assertIn("review:current-delta-shape", problems)

    def test_frozen_status_with_pending_refused(self) -> None:
        review = passing_review()
        review["vulcan"] = "PENDING"
        problems = review_problems(review, CHECKER.FROZEN_STATUS)
        self.assertIn("review:current-delta-frozen-requires-pass", problems)

    def test_complete_status_with_fail_refused(self) -> None:
        review = passing_review()
        review["nabu"] = "FAIL"
        problems = review_problems(review, CHECKER.COMPLETE_STATUS)
        self.assertIn("review:current-delta-frozen-requires-pass", problems)

    def test_unknown_judgment_word_refused(self) -> None:
        review = passing_review()
        review["ariadne"] = "APPROVED"
        problems = review_problems(review, CHECKER.REVIEW_PENDING_STATUS)
        self.assertIn("review:current-delta-judgment:ariadne", problems)


class ValidPinControl(unittest.TestCase):
    def test_current_passing_review_accepted(self) -> None:
        self.assertEqual(review_problems(passing_review(), CHECKER.REVIEW_PENDING_STATUS), [])

    def test_pending_review_accepted_before_freeze(self) -> None:
        review = passing_review()
        review["vulcan"] = "PENDING"
        self.assertEqual(review_problems(review, CHECKER.REVIEW_PENDING_STATUS), [])


if __name__ == "__main__":
    unittest.main()
