#!/usr/bin/env python3
"""Terminal acceptance regressions for current revision review.

Each case serves one summary input from memory while every other file
remains the real pinned input, runs the real checker entrypoint, and
requires PASS with a coherent terminal fixture: the named section status
and its related stage boolean set, current_delta_review bound to the
current revision with all three judgments PASS, and all original
historical verdict fields preserved byte-for-value. The pinned checkers
additionally require historical verdicts at terminal states
(N-STATIC-R2-01), so these cases fail before source repair and pass after
it. The ordinary baseline controls pass before and after. These
fabricated test-only fixtures are checker controls only, never real
review evidence or stage approval. Nothing here mutates the repository
under test.
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


SMP1_CHECKER = load_module("smp1_contract_checker_terminal", "scripts/check_smp1_contract.py")
BRIDGE_CHECKER = load_module(
    "smp1_json_bridge_contract_checker_terminal",
    "scripts/check_smp1_json_bridge_contract.py",
)
CLI_CHECKER = load_module("cli_contract_checker_terminal", "scripts/check_cli_contract.py")
INDEX_CHECKER = load_module(
    "required_contract_index_checker_terminal",
    "scripts/check_required_contract_index.py",
)

SUMMARY_TEXT = (ROOT / "machineresearch/sley-2.0/machine-summary.json").read_text(
    encoding="utf-8"
)

HISTORICAL_VERDICTS = (
    "ariadne_contract_review",
    "nabu_architecture_review",
    "vulcan_surface_review",
)


def run_checker_with_summary(module, summary_text: str) -> tuple[int, dict]:
    """Run the real checker with SUMMARY served from memory; all else real."""
    original_read = module.read

    def read(path):
        if Path(path) == module.SUMMARY:
            return summary_text
        return original_read(path)

    module.read = read
    try:
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            code = module.main()
        return code, json.loads(buffer.getvalue())
    finally:
        module.read = original_read


def pass_current_review(section: dict, revision: int) -> None:
    review = section["current_delta_review"]
    assert review["contract_revision"] == revision
    for lane in ("ariadne", "nabu", "vulcan"):
        review[lane] = "PASS"


class BridgeValidBaseline(unittest.TestCase):
    def test_valid_baseline_accepted(self):
        section = json.loads(SUMMARY_TEXT)["json_bridge"]
        self.assertEqual(section["contract_revision"], 9)
        code, payload = run_checker_with_summary(BRIDGE_CHECKER, SUMMARY_TEXT)
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


class CliValidBaseline(unittest.TestCase):
    def test_valid_baseline_accepted(self):
        section = json.loads(SUMMARY_TEXT)["cli"]
        self.assertEqual(section["contract_revision"], 6)
        code, payload = run_checker_with_summary(CLI_CHECKER, SUMMARY_TEXT)
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


class Smp1TerminalAcceptance(unittest.TestCase):
    """N-STATIC-R2-01: current all-PASS admits freeze with history kept."""

    def test_frozen_all_pass_accepted(self):
        summary = json.loads(SUMMARY_TEXT)
        section = summary["protocol"]
        preserved = {key: section[key] for key in HISTORICAL_VERDICTS}
        section["status"] = SMP1_CHECKER.FROZEN_STATUS
        section["contract_complete"] = True
        pass_current_review(section, 12)
        self.assertEqual(section["status"], "S20_400_CONTRACT_FROZEN")
        self.assertTrue(section["contract_complete"])
        for key, value in preserved.items():
            self.assertEqual(section[key], value)
        code, payload = run_checker_with_summary(
            SMP1_CHECKER, json.dumps(summary)
        )
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


class IndexTerminalAcceptance(unittest.TestCase):
    """N-STATIC-R2-01: current all-PASS admits acceptance with history kept."""

    def test_accepted_all_pass_accepted(self):
        summary = json.loads(SUMMARY_TEXT)
        section = summary["required_contract_index"]
        preserved = {key: section[key] for key in HISTORICAL_VERDICTS}
        section["status"] = INDEX_CHECKER.ACCEPTED_STATUS
        pass_current_review(section, 3)
        self.assertEqual(section["status"], "S20_770_INDEX_ACCEPTED")
        self.assertEqual(section["contract_revision"], 3)
        self.assertFalse(section["implementation_complete"])
        for key, value in preserved.items():
            self.assertEqual(section[key], value)
        code, payload = run_checker_with_summary(
            INDEX_CHECKER, json.dumps(summary)
        )
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


class BridgeTerminalAcceptance(unittest.TestCase):
    """N-STATIC-R2-01: current all-PASS admits completion with history kept."""

    def test_complete_all_pass_accepted(self):
        summary = json.loads(SUMMARY_TEXT)
        section = summary["json_bridge"]
        preserved = {key: section[key] for key in HISTORICAL_VERDICTS}
        section["status"] = BRIDGE_CHECKER.COMPLETE_STATUS
        section["implementation_complete"] = True
        pass_current_review(section, 9)
        self.assertEqual(section["status"], "S20_420_COMPLETE")
        self.assertTrue(section["implementation_complete"])
        for key, value in preserved.items():
            self.assertEqual(section[key], value)
        code, payload = run_checker_with_summary(
            BRIDGE_CHECKER, json.dumps(summary)
        )
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


class CliTerminalAcceptance(unittest.TestCase):
    """N-STATIC-R2-01: current all-PASS admits completion with history kept."""

    def test_complete_all_pass_accepted(self):
        summary = json.loads(SUMMARY_TEXT)
        section = summary["cli"]
        preserved = {key: section[key] for key in HISTORICAL_VERDICTS}
        section["status"] = CLI_CHECKER.COMPLETE_STATUS
        section["implementation_complete"] = True
        pass_current_review(section, 6)
        self.assertEqual(section["status"], "S20_430_COMPLETE")
        self.assertTrue(section["implementation_complete"])
        for key, value in preserved.items():
            self.assertEqual(section[key], value)
        code, payload = run_checker_with_summary(
            CLI_CHECKER, json.dumps(summary)
        )
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


if __name__ == "__main__":
    unittest.main()
