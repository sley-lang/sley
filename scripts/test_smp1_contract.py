#!/usr/bin/env python3
"""Discriminating static tests for the S20-400 checker and S20-420 generator.

Each mutation case serves one input from memory while every other file
remains the real pinned input, runs the real checker/generator entrypoint,
captures its real JSON/return code, and requires a specific refusal. The
pinned checker/generator currently misses these drifts (VUL-P2S-02,
VUL-P2S-03, A-ST-03), so the mutation cases fail before source repair and
pass after it. The valid controls pass before and after. Nothing here
mutates the repository under test.
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


CHECKER = load_module("smp1_contract_checker", "scripts/check_smp1_contract.py")
GENERATOR = load_module("bridge_table_generator", "scripts/generate_smp1_json_bridge_table.py")

SPEC_TEXT = (ROOT / "docs/spec/SMP1.md").read_text(encoding="utf-8")
SUMMARY_TEXT = (ROOT / "machineresearch/sley-2.0/machine-summary.json").read_text(
    encoding="utf-8"
)

APPENDIX_A = "## Appendix A. Body records of the dispatched methods (S20-410)"
APPENDIX_B = "## Appendix B. Cancellation, streaming, and budget records (S20-440)"
CURRENT_COMPOSITION = "Current composition (revision 12):"
V1_REPORT_ROW = "| 604 | `report` | report identity | report record | S20-290 (report store is S20-560) |"


def run_checker_with_spec(
    mutated_spec: str, summary_text: str | None = None
) -> tuple[int, dict]:
    """Run the real checker with SPEC served from memory; all else real."""
    original_read = CHECKER.read

    def read(path):
        if Path(path) == CHECKER.SPEC:
            return mutated_spec
        if summary_text is not None and Path(path) == CHECKER.SUMMARY:
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


class ValidPinControl(unittest.TestCase):
    def test_valid_pin_accepted(self):
        code, payload = run_checker_with_spec(SPEC_TEXT)
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


class AppendixScopeCases(unittest.TestCase):
    """VUL-P2S-02: body rows outside appendices A/C/D are currently invisible."""

    def test_reserved_row_in_appendix_b_refused(self):
        self.assertIn(APPENDIX_B, SPEC_TEXT)
        probe = "| 305 `diagnostics` | reserved | reserved |"
        mutated = SPEC_TEXT.replace(APPENDIX_B, APPENDIX_B + "\n" + probe, 1)
        self.assertNotEqual(mutated, SPEC_TEXT)
        code, payload = run_checker_with_spec(mutated)
        assert_refused(self, code, payload, "appendix")

    def test_duplicate_live_row_before_appendix_a_refused(self):
        self.assertIn(APPENDIX_A, SPEC_TEXT)
        probe = "| 100 `session.open` | duplicate probe request | duplicate probe response |"
        mutated = SPEC_TEXT.replace(APPENDIX_A, probe + "\n" + APPENDIX_A, 1)
        self.assertNotEqual(mutated, SPEC_TEXT)
        code, payload = run_checker_with_spec(mutated)
        assert_refused(self, code, payload, "appendix")


class ReversePinCases(unittest.TestCase):
    """VUL-P2S-03/N-STATIC-02: historical prose must not satisfy current pins."""

    BRIDGE_PIN = "`docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 8"
    CLI_PIN = "`docs/spec/SLEY_CLI_V1.md` revision 6"

    def test_historical_shadow_does_not_satisfy_current_pin(self):
        self.assertIn(CURRENT_COMPOSITION, SPEC_TEXT)
        self.assertIn(self.BRIDGE_PIN, SPEC_TEXT)
        self.assertIn(self.CLI_PIN, SPEC_TEXT)
        shadow = (
            "Historical note: an earlier draft pinned the bridge "
            "`docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 8 and the CLI "
            "`docs/spec/SLEY_CLI_V1.md` revision 6 in passing.\n"
        )
        head, sep, tail = SPEC_TEXT.partition(CURRENT_COMPOSITION)
        self.assertEqual(sep, CURRENT_COMPOSITION)
        paragraph_end = tail.index("\n\n")
        paragraph = tail[:paragraph_end]
        stale = paragraph.replace(
            self.BRIDGE_PIN,
            "`docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 7",
        ).replace(
            self.CLI_PIN,
            "`docs/spec/SLEY_CLI_V1.md` revision 5",
        )
        self.assertNotEqual(stale, paragraph)
        mutated = head + shadow + CURRENT_COMPOSITION + stale + tail[paragraph_end:]
        code, payload = run_checker_with_spec(mutated)
        assert_refused(self, code, payload, "reverse-pin")

    def test_duplicate_current_composition_refused(self):
        self.assertIn(CURRENT_COMPOSITION, SPEC_TEXT)
        head, sep, tail = SPEC_TEXT.partition(CURRENT_COMPOSITION)
        self.assertEqual(sep, CURRENT_COMPOSITION)
        paragraph_end = tail.index("\n\n")
        paragraph = tail[:paragraph_end]
        rest = tail[paragraph_end:]
        mutated = head + CURRENT_COMPOSITION + paragraph + "\n" + CURRENT_COMPOSITION + paragraph + rest
        self.assertNotEqual(mutated, SPEC_TEXT)
        code, payload = run_checker_with_spec(mutated)
        self.assertEqual(payload.get("result"), "FAIL", "checker must refuse the drift")
        self.assertNotEqual(code, 0, "checker must exit nonzero on the drift")
        problems = payload.get("problems", [])
        self.assertTrue(
            any("composition" in problem or "reverse-pin" in problem for problem in problems),
            f"refusal must come from the composition/pin guard, got: {problems}",
        )


class GeneratorTableCases(unittest.TestCase):
    """A-ST-03: exact v1 tag inventory and cross-table name uniqueness."""

    def test_valid_tables_parse(self):
        v1_methods, v2_additions = GENERATOR.parse_tables(SPEC_TEXT)
        self.assertEqual(len(v1_methods), 41)
        self.assertEqual(
            [(method["tag"], method["name"]) for method in v2_additions],
            [(306, "entity.version"), (307, "entity.signature")],
        )

    def test_unknown_tag_605_refused(self):
        self.assertIn(V1_REPORT_ROW, SPEC_TEXT)
        mutated = SPEC_TEXT.replace(V1_REPORT_ROW, V1_REPORT_ROW.replace("| 604 |", "| 605 |", 1), 1)
        self.assertNotEqual(mutated, SPEC_TEXT)
        with self.assertRaises(SystemExit):
            GENERATOR.parse_tables(mutated)

    def test_v1_renamed_to_entity_version_refused(self):
        self.assertIn(V1_REPORT_ROW, SPEC_TEXT)
        renamed = V1_REPORT_ROW.replace("`report`", "`entity.version`", 1)
        mutated = SPEC_TEXT.replace(V1_REPORT_ROW, renamed, 1)
        self.assertNotEqual(mutated, SPEC_TEXT)
        with self.assertRaises(SystemExit):
            GENERATOR.parse_tables(mutated)


class CompositionAnchorCases(unittest.TestCase):
    """A-ST-R2-02/N-STATIC-R2-03/VUL-P2S-R2-02: the current record and the
    Status it binds to are line-anchored, not substrings."""

    STATUS_LINE = "Status: S20-400 contract draft, revision 12"

    def test_historical_prefixed_composition_refused(self):
        self.assertEqual(SPEC_TEXT.count(CURRENT_COMPOSITION), 1)
        mutated = SPEC_TEXT.replace(
            CURRENT_COMPOSITION, "Historical note: " + CURRENT_COMPOSITION, 1
        )
        self.assertNotEqual(mutated, SPEC_TEXT)
        code, payload = run_checker_with_spec(mutated)
        self.assertEqual(payload.get("result"), "FAIL", "checker must refuse the drift")
        self.assertNotEqual(code, 0, "checker must exit nonzero on the drift")
        problems = payload.get("problems", [])
        self.assertTrue(
            any("composition" in problem or "reverse-pin" in problem for problem in problems),
            f"refusal must come from the composition/pin guard, got: {problems}",
        )

    def test_historical_prefixed_status_refused(self):
        self.assertEqual(SPEC_TEXT.count(self.STATUS_LINE), 1)
        mutated = SPEC_TEXT.replace(
            self.STATUS_LINE, "Historical note: " + self.STATUS_LINE, 1
        )
        self.assertNotEqual(mutated, SPEC_TEXT)
        code, payload = run_checker_with_spec(mutated)
        self.assertEqual(payload.get("result"), "FAIL", "checker must refuse the drift")
        self.assertNotEqual(code, 0, "checker must exit nonzero on the drift")
        problems = payload.get("problems", [])
        self.assertTrue(
            any("revision" in problem or "status" in problem for problem in problems),
            f"refusal must come from the revision/status guard, got: {problems}",
        )

    def test_historical_references_outside_record_accepted(self):
        shadow = (
            "Historical note: an earlier draft pinned the bridge "
            "`docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 8 and the CLI "
            "`docs/spec/SLEY_CLI_V1.md` revision 6 in passing.\n"
        )
        self.assertIn(CURRENT_COMPOSITION, SPEC_TEXT)
        mutated = shadow + SPEC_TEXT
        self.assertNotEqual(mutated, SPEC_TEXT)
        code, payload = run_checker_with_spec(mutated)
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


class CurrentDeltaReviewCases(unittest.TestCase):
    """N-STATIC-01: the current revision review is bound, not historical."""

    def test_missing_current_review_refused(self):
        summary = json.loads(SUMMARY_TEXT)
        self.assertIn("current_delta_review", summary["protocol"])
        del summary["protocol"]["current_delta_review"]
        code, payload = run_checker_with_spec(SPEC_TEXT, json.dumps(summary))
        assert_refused(self, code, payload, "review")

    def test_mismatched_current_revision_refused(self):
        summary = json.loads(SUMMARY_TEXT)
        review = summary["protocol"]["current_delta_review"]
        self.assertEqual(review["contract_revision"], 12)
        review["contract_revision"] = 11
        code, payload = run_checker_with_spec(SPEC_TEXT, json.dumps(summary))
        assert_refused(self, code, payload, "review")

    def test_bound_all_pass_review_accepted(self):
        summary = json.loads(SUMMARY_TEXT)
        review = summary["protocol"]["current_delta_review"]
        self.assertEqual(review["contract_revision"], 12)
        for lane in ("ariadne", "nabu", "vulcan"):
            review[lane] = "PASS"
        code, payload = run_checker_with_spec(SPEC_TEXT, json.dumps(summary))
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


if __name__ == "__main__":
    unittest.main()
