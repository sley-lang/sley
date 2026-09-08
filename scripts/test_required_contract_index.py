#!/usr/bin/env python3
"""Discriminating static tests for the S20-770 required-contract index checker.

Each mutation case serves one summary input from memory while every other
file remains the real pinned input, runs the real checker entrypoint,
captures its real JSON/return code, and requires a specific refusal. The
pinned checker binds current_delta_review.contract_revision to the anchored
document revision but never validates section contract_revision
(A-ST-R2-03, N-STATIC-R2-02, VUL-P2S-R2-03), so the section-only mutations
fail before source repair and pass after it. The valid control passes
before and after. Nothing here mutates the repository under test.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
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


CHECKER = load_module(
    "required_contract_index_checker", "scripts/check_required_contract_index.py"
)

SUMMARY_TEXT = (ROOT / "machineresearch/sley-2.0/machine-summary.json").read_text(
    encoding="utf-8"
)


def run_checker_with_summary(summary_text: str) -> tuple[int, dict]:
    """Run the real checker with SUMMARY served from memory; all else real."""
    original_read = CHECKER.read

    def read(path):
        if Path(path) == CHECKER.SUMMARY:
            return summary_text
        return original_read(path)

    CHECKER.read = read
    try:
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            code = CHECKER.main()
        return code, json.loads(buffer.getvalue())
    finally:
        CHECKER.read = original_read


def assert_refused(testcase: unittest.TestCase, code: int, payload: dict, family: str) -> None:
    testcase.assertEqual(payload.get("result"), "FAIL", "checker must refuse the drift")
    testcase.assertNotEqual(code, 0, "checker must exit nonzero on the drift")
    problems = payload.get("problems", [])
    testcase.assertTrue(
        any(family in problem for problem in problems),
        f"refusal must come from the {family!r} guard, got: {problems}",
    )


class ValidBaselineControl(unittest.TestCase):
    def test_valid_baseline_accepted(self):
        summary = json.loads(SUMMARY_TEXT)
        section = summary["required_contract_index"]
        self.assertEqual(section["contract_revision"], 2)
        self.assertEqual(
            section["current_delta_review"]["contract_revision"], 2
        )
        code, payload = run_checker_with_summary(SUMMARY_TEXT)
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


class SectionRevisionCases(unittest.TestCase):
    """A-ST-R2-03/N-STATIC-R2-02/VUL-P2S-R2-03: document, section and
    current review revisions agree as true integers."""

    def test_stale_section_revision_refused(self):
        summary = json.loads(SUMMARY_TEXT)
        section = summary["required_contract_index"]
        self.assertEqual(section["contract_revision"], 2)
        self.assertEqual(
            section["current_delta_review"]["contract_revision"], 2
        )
        section["contract_revision"] = 1
        code, payload = run_checker_with_summary(json.dumps(summary))
        assert_refused(self, code, payload, "revision")

    def test_bool_section_revision_refused(self):
        summary = json.loads(SUMMARY_TEXT)
        section = summary["required_contract_index"]
        self.assertEqual(section["contract_revision"], 2)
        section["contract_revision"] = True
        code, payload = run_checker_with_summary(json.dumps(summary))
        assert_refused(self, code, payload, "revision")

    def test_missing_section_revision_refused(self):
        summary = json.loads(SUMMARY_TEXT)
        section = summary["required_contract_index"]
        self.assertIn("contract_revision", section)
        del section["contract_revision"]
        code, payload = run_checker_with_summary(json.dumps(summary))
        assert_refused(self, code, payload, "revision")


if __name__ == "__main__":
    unittest.main()
