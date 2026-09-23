#!/usr/bin/env python3
"""Revert cases for the S20-620 checker's composed-pin anchor (revision 7).

A stale SMP1 or S20-300 pin in the trial-runner contract is refused
against those documents' own Status lines. Inputs are served from memory;
nothing here mutates the repository.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "trial_runner_checker", ROOT / "scripts/check_sley2_trial_runner.py")
CHECKER = importlib.util.module_from_spec(spec)
sys.modules["trial_runner_checker"] = CHECKER
spec.loader.exec_module(CHECKER)

SPEC = (ROOT / "docs/spec/SLEY2_TRIAL_RUNNER_V1.md").read_text(encoding="utf-8")
SMP1 = (ROOT / "docs/spec/SMP1.md").read_text(encoding="utf-8")
SNAPSHOT = (ROOT / "docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md").read_text(encoding="utf-8")


class ComposedPins(unittest.TestCase):
    def test_current_pins_pass(self):
        self.assertEqual(CHECKER.composed_pin_problems(SPEC, SMP1, SNAPSHOT), [])

    def test_a_stale_smp1_pin_is_refused(self):
        stale = SPEC.replace("`docs/spec/SMP1.md`\n  revision 15", "`docs/spec/SMP1.md`\n  revision 13", 1)
        self.assertNotEqual(stale, SPEC)
        self.assertTrue(any(p.startswith("smp1-pin") for p in
                            CHECKER.composed_pin_problems(stale, SMP1, SNAPSHOT)))

    def test_a_stale_snapshot_pin_is_refused(self):
        stale = SPEC.replace("section 5, revision 6)", "section 5, revision 4)", 1)
        self.assertNotEqual(stale, SPEC)
        self.assertTrue(any(p.startswith("s20-300-pin") for p in
                            CHECKER.composed_pin_problems(stale, SMP1, SNAPSHOT)))

    def test_an_authority_moving_leaves_the_contract_red(self):
        moved = SMP1.replace("contract draft, revision 15", "contract draft, revision 16", 1)
        self.assertTrue(any(p.startswith("smp1-pin") for p in
                            CHECKER.composed_pin_problems(SPEC, moved, SNAPSHOT)))


if __name__ == "__main__":
    unittest.main()
