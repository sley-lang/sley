"""Offline tests of the S20-750 decision dossier: coverage, gating, decision state."""

from __future__ import annotations

import importlib.util
import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SPEC_PATH = ROOT / "docs/spec/DECISION_DOSSIER_V1.md"
SPEC = importlib.util.spec_from_file_location(
    "build_decision_dossier", ROOT / "scripts/build_decision_dossier.py"
)
assert SPEC is not None and SPEC.loader is not None
dossier = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dossier)


def contract_items() -> list[str]:
    text = SPEC_PATH.read_text(encoding="utf-8")
    start = text.index("## 1.1 The required items")
    end = text.index("## 2. Sources", start)
    return [match.group(1).strip() for match in re.finditer(r"^\d+\. (.+)$", text[start:end], re.M)]


class CoverageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.dossier = dossier.build_dossier()

    def test_the_dossier_covers_the_contract_items_in_order(self) -> None:
        self.assertEqual([entry["item"] for entry in self.dossier["entries"]], contract_items())
        self.assertEqual(len(self.dossier["entries"]), 34)

    def test_counts_agree_with_the_entry_states(self) -> None:
        evidenced = [e for e in self.dossier["entries"] if e["state"] == "EVIDENCED"]
        gated = [e for e in self.dossier["entries"] if e["state"] == "GATED"]
        self.assertEqual(self.dossier["evidenced"], len(evidenced))
        self.assertEqual(self.dossier["gated"], len(gated))
        self.assertEqual(len(evidenced) + len(gated), 34)

    def test_a_gated_item_never_carries_a_value(self) -> None:
        for entry in self.dossier["entries"]:
            if entry["state"] == "GATED":
                self.assertIsNone(entry["value"], entry["item"])
                self.assertTrue(entry["note"], entry["item"])

    def test_every_evidenced_item_names_tracked_evidence(self) -> None:
        for entry in self.dossier["entries"]:
            if entry["state"] == "EVIDENCED":
                self.assertTrue(entry["evidence"], entry["item"])
                for path in entry["evidence"]:
                    self.assertTrue((ROOT / path).exists(), path)
                    self.assertFalse(path.startswith("/"), path)

    def test_the_dossier_claims_no_publication_and_no_delegated_authority(self) -> None:
        self.assertEqual(
            self.dossier["decision_authority"], dossier.DECISION_AUTHORITY
        )
        self.assertFalse(any(self.dossier["publication"].values()))

    def test_no_host_path_or_instant_appears(self) -> None:
        text = dossier.canonical(self.dossier)
        for marker in ("/home/", "greyforge", "file://"):
            self.assertNotIn(marker, text)

    def test_the_dossier_is_a_pure_function_of_its_sources(self) -> None:
        self.assertEqual(self.dossier, dossier.build_dossier())
        self.assertEqual(
            self.dossier["entries_digest"], dossier.digest_of(self.dossier["entries"])
        )


class DecisionTests(unittest.TestCase):
    def sources(self, **overrides) -> dict:
        base = {
            "summary": {
                "s20_710_pre_release_audit": {"root_license_text_approved": True},
                "succession": {"trials_executed": 3, "thresholds_pass": True},
                "release_candidate_packaging": {"release_check_gate": "OPEN"},
            },
            "register": {
                "open_reviews": [],
                "deferred_reviews": [],
                "declared_open_findings": {"p0": 0, "p1": 0, "p2": 0},
            },
            "repro": {"result": "MULTI_HOST_REPRODUCIBLE"},
        }
        for key, value in overrides.items():
            if isinstance(value, dict) and isinstance(base.get(key), dict):
                merged = dict(base[key])
                merged.update(value)
                base[key] = merged
            else:
                base[key] = value
        return base

    def test_the_current_tree_is_blocked_with_exact_reasons(self) -> None:
        state = dossier.build_dossier()
        self.assertEqual(state["decision_state"], "BLOCKED")
        self.assertTrue(state["decision_reasons"])
        self.assertEqual(state["decision_reasons"], sorted(state["decision_reasons"]))

    def test_every_blocking_condition_yields_blocked(self) -> None:
        for override, reason in (
            ({"register": {"open_reviews": [{"section": "x", "field": "y"}]}}, "review obligations are open"),
            ({"register": {"deferred_reviews": [{"section": "x"}]}}, "deferred"),
            ({"summary": {"s20_710_pre_release_audit": {"root_license_text_approved": False}}}, "root license"),
            ({"summary": {"succession": {"trials_executed": 0, "thresholds_pass": True}}}, "succession trial"),
            (
                {"summary": {"release_candidate_packaging": {"release_check_gate": "FAIL_CLOSED_NOT_IMPLEMENTED"}}},
                "fail-closed",
            ),
            ({"repro": {"result": "SINGLE_HOST_REPRODUCIBLE"}}, "one host"),
        ):
            state, reasons = dossier.derive_decision(self.sources(**override), [])
            self.assertEqual(state, "BLOCKED", override)
            self.assertTrue(any(reason in line for line in reasons), (override, reasons))

    def test_precedence_reaches_the_other_states(self) -> None:
        state, _ = dossier.derive_decision(
            self.sources(register={"open_reviews": [], "deferred_reviews": [], "declared_open_findings": {"p1": 1}}),
            [],
        )
        self.assertEqual(state, "FAIL")
        state, _ = dossier.derive_decision(
            self.sources(summary={"succession": {"trials_executed": 3, "thresholds_pass": False}}), []
        )
        self.assertEqual(state, "ALPHA_COMPLETE")
        state, _ = dossier.derive_decision(
            self.sources(summary={"approved_conditional_items": ["an approved P2"]}), []
        )
        self.assertEqual(state, "CONDITIONAL_PASS")
        state, reasons = dossier.derive_decision(self.sources(), [])
        self.assertEqual((state, reasons), ("PASS", []))

    def test_a_missing_source_fails_closed(self) -> None:
        original = dossier.REGISTER
        dossier.REGISTER = Path("/nonexistent/register.json")
        self.addCleanup(setattr, dossier, "REGISTER", original)
        with self.assertRaises(dossier.DossierError) as error:
            dossier.build_dossier()
        self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_MISSING)


if __name__ == "__main__":
    unittest.main()
