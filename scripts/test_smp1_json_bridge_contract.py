#!/usr/bin/env python3
"""Revert cases for the S20-420 checker's pin anchors (revision 12).

The SMP1 pin is read from the one composition sentence, so a stale pin
there fails even while a history line carries the current revision; the
ADR-0034 status line must name the current contract revision and carry
its revision record. Inputs are served from memory; nothing here mutates
the repository.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "bridge_contract_checker", ROOT / "scripts/check_smp1_json_bridge_contract.py")
CHECKER = importlib.util.module_from_spec(spec)
sys.modules["bridge_contract_checker"] = CHECKER
spec.loader.exec_module(CHECKER)

SPEC = (ROOT / "docs/spec/SMP1_JSON_BRIDGE_V1.md").read_text(encoding="utf-8")
ADR = (ROOT / "docs/adr/ADR-0034-json-bridge-boundary.md").read_text(encoding="utf-8")
PIN = f"`docs/spec/SMP1.md` (revision {CHECKER.SMP1_REVISION}):"


class PinAnchors(unittest.TestCase):
    def test_current_texts_pass(self):
        self.assertEqual(CHECKER.pin_problems(SPEC, ADR), [])

    def test_stale_composition_pin_with_a_current_history_line_is_refused(self):
        self.assertIn(PIN, SPEC)
        stale = SPEC.replace(PIN, "`docs/spec/SMP1.md` (revision 12):", 1)
        stale += f"\nHistory: re-pins {PIN}\n"
        self.assertIn("composition-sentence:smp1-revision-12", CHECKER.pin_problems(stale, ADR))

    def test_stale_adr_status_is_refused(self):
        current = f"the S20-420 contract is a draft at revision {CHECKER.SPEC_REVISION}"
        self.assertIn(current, ADR)
        stale = ADR.replace(current, "the S20-420 contract is a draft at revision 3", 1)
        self.assertIn("adr-current-revision", CHECKER.pin_problems(SPEC, stale))

    def test_missing_adr_revision_record_is_refused(self):
        record = f"Revision {CHECKER.SPEC_REVISION} record ("
        self.assertIn(record, ADR)
        self.assertIn("adr-revision-record",
                      CHECKER.pin_problems(SPEC, ADR.replace(record, "Revision N record (")))


if __name__ == "__main__":
    unittest.main()
