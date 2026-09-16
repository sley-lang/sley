"""Offline tests of the S20-750 decision dossier: coverage, gating, decision state."""

from __future__ import annotations

import importlib.util
import json
import re
import unittest
from unittest.mock import patch
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


ARM_ITEMS = (
    "strict correctness by arm",
    "ACT by arm",
    "context bytes and model tokens by arm",
    "repair loops by arm",
    "invalid committed states",
    "stale candidates incorrectly accepted",
)


class DecisionTests(unittest.TestCase):
    def sources(self, **overrides) -> dict:
        base = {
            "summary": {
                "release_candidate_packaging": {"release_check_gate": "OPEN"},
            },
            # The one section 22 threshold verdict (the GA builder's derivation
            # from the tracked accounting report), injected as the builder does.
            "thresholds": {"pass": True, "note": "every threshold row PASS"},
        }
        for key, value in overrides.items():
            if isinstance(value, dict) and isinstance(base.get(key), dict):
                merged = dict(base[key])
                merged.update(value)
                base[key] = merged
            else:
                base[key] = value
        return base

    def entries(self, **overrides) -> list[dict]:
        base = {
            "findings by severity and disposition": (
                "EVIDENCED",
                {
                    "open_reviews": 0,
                    "deferred_reviews": 0,
                    "unclassified": 0,
                    "unclaimed_carried_findings": 0,
                    "result": "FINDING_REGISTER_CLEAR",
                    "declared_open_findings": {"p0": 0, "p1": 0, "p2": 0},
                },
            ),
            "SBOM and license inventory": (
                "EVIDENCED",
                {"root_license_text_approved": True},
            ),
            "reproducibility result": (
                "EVIDENCED",
                {"result": "MULTI_HOST_REPRODUCIBLE"},
            ),
        }
        for item in ARM_ITEMS:
            base[item] = ("EVIDENCED", {})
        for item, change in overrides.items():
            state, value = base[item]
            if isinstance(change, dict):
                value = {**value, **change}
            else:
                state = change
            base[item] = (state, value)
        return [{"item": item, "state": state, "value": value} for item, (state, value) in base.items()]

    def test_the_current_tree_is_blocked_with_exact_reasons(self) -> None:
        state = dossier.build_dossier()
        self.assertEqual(state["decision_state"], "BLOCKED")
        self.assertTrue(state["decision_reasons"])
        self.assertEqual(state["decision_reasons"], sorted(state["decision_reasons"]))

    def test_every_blocking_condition_yields_blocked(self) -> None:
        gate_closed = {"summary": {"release_candidate_packaging": {"release_check_gate": "FAIL_CLOSED_NOT_IMPLEMENTED"}}}
        for sources, entries, reason in (
            (self.sources(), self.entries(**{"findings by severity and disposition": {"open_reviews": 1}}), "review obligations are open"),
            (self.sources(), self.entries(**{"findings by severity and disposition": {"deferred_reviews": 2}}), "deferred"),
            (self.sources(), self.entries(**{"findings by severity and disposition": {"unclassified": 3}}), "unclassified"),
            (self.sources(), self.entries(**{"findings by severity and disposition": {"result": "FINDING_REGISTER_OPEN"}}), "not FINDING_REGISTER_CLEAR"),
            (self.sources(), self.entries(**{"SBOM and license inventory": {"root_license_text_approved": False}}), "root license"),
            (self.sources(), self.entries(**{item: "GATED" for item in ARM_ITEMS}), "succession trial"),
            (self.sources(**gate_closed), self.entries(), "fail-closed"),
            (self.sources(), self.entries(**{"reproducibility result": {"result": "SINGLE_HOST_REPRODUCIBLE"}}), "one host"),
        ):
            state, reasons = dossier.derive_decision(sources, entries)
            self.assertEqual(state, "BLOCKED", (sources, entries))
            self.assertTrue(any(reason in line for line in reasons), (entries, reasons))

    def test_precedence_reaches_the_other_states(self) -> None:
        state, _ = dossier.derive_decision(
            self.sources(),
            self.entries(**{"findings by severity and disposition": {"declared_open_findings": {"p1": 1}}}),
        )
        self.assertEqual(state, "FAIL")
        state, reasons = dossier.derive_decision(
            self.sources(thresholds={"pass": False, "note": "no tracked S20-630 accounting report"}),
            self.entries(),
        )
        self.assertEqual(state, "ALPHA_COMPLETE")
        self.assertTrue(any("no tracked S20-630 accounting report" in line for line in reasons))
        # An absent verdict is a non-passing one, never a pass.
        sources = self.sources()
        del sources["thresholds"]
        state, _ = dossier.derive_decision(sources, self.entries())
        self.assertEqual(state, "ALPHA_COMPLETE")
        state, _ = dossier.derive_decision(
            self.sources(summary={"approved_conditional_items": ["an approved P2"]}),
            self.entries(),
        )
        self.assertEqual(state, "CONDITIONAL_PASS")
        state, reasons = dossier.derive_decision(self.sources(), self.entries())
        self.assertEqual((state, reasons), ("PASS", []))

    def test_the_arm_entries_drive_the_trial_rule_not_the_sources(self) -> None:
        # Summary hand keys are not read: zero trials in the summary do not
        # block while the arm entries are evidenced ...
        state, reasons = dossier.derive_decision(
            self.sources(summary={"succession": {"trials_executed": 0, "thresholds_pass": True}}),
            self.entries(),
        )
        self.assertEqual(state, "PASS")
        # ... and claimed trials plus a hand thresholds_pass do not unblock
        # while the arm entries are gated.
        state, reasons = dossier.derive_decision(
            self.sources(summary={"succession": {"trials_executed": 9, "thresholds_pass": True}}),
            self.entries(**{item: "GATED" for item in ARM_ITEMS}),
        )
        self.assertEqual(state, "BLOCKED")
        self.assertTrue(any("succession trial" in line for line in reasons))

    def test_a_missing_decision_input_entry_fails_closed(self) -> None:
        entries = [entry for entry in self.entries() if entry["item"] != "findings by severity and disposition"]
        state, reasons = dossier.derive_decision(self.sources(), entries)
        self.assertEqual(state, "BLOCKED")
        self.assertTrue(any("no findings by severity and disposition entry" in line for line in reasons))

    def test_a_gated_findings_entry_fails_closed(self) -> None:
        state, reasons = dossier.derive_decision(
            self.sources(),
            self.entries(**{"findings by severity and disposition": "GATED"}),
        )
        self.assertEqual(state, "BLOCKED")
        self.assertTrue(any("openness is unknown" in line for line in reasons))

class SourceSeparationTests(unittest.TestCase):
    def live_sources(self) -> dict:
        return {
            "summary": dossier.load(dossier.SUMMARY),
            "repro": dossier.load(dossier.REPRO, "sley2.reproducibility-report.v1"),
            "cyclonedx": dossier.load(dossier.CYCLONEDX),
            "spdx": dossier.load(dossier.SPDX),
            "inventory": dossier.load(dossier.INVENTORY, "s20-710-pre-release-inventory-v1"),
            "provenance": dossier.load(dossier.PROVENANCE, "sley2.release-provenance.v1"),
            "conformance": dossier.load(dossier.CONFORMANCE, "sley2.independent-conformance-report.v1"),
            "register": dossier.load(dossier.REGISTER, "sley2.finding-register.v1"),
            "inventory_of_tests": dossier.load(dossier.TEST_INVENTORY, "sley2.test-inventory.v1"),
            "threat_coverage": dossier.load(dossier.THREAT_COVERAGE, "sley2.threat-coverage-report.v1"),
            "ga_acceptance": dossier.load(dossier.GA_ACCEPTANCE, "sley2.ga-acceptance-report.v1"),
        }

    def entry(self, entries: list[dict], item: str) -> dict:
        return next(entry for entry in entries if entry["item"] == item)

    def test_the_sbom_entry_reads_the_license_inventory(self) -> None:
        sources = self.live_sources()
        sources["inventory"] = {
            "packages": [
                {"license_disposition": "BLOCKED_MISSING_TEXT"},
                {"license_disposition": "BLOCKED_MISSING_TEXT"},
                {"license_disposition": "APPROVED"},
            ]
        }
        sbom = self.entry(dossier.build_entries(sources), "SBOM and license inventory")
        self.assertEqual(sbom["value"]["license_disposition_blocked"], 2)
        self.assertIn("evidence/security/T52/pre-release-inventory.json", sbom["evidence"])

    def test_the_property_entry_reads_the_test_inventory(self) -> None:
        sources = self.live_sources()
        sources["inventory_of_tests"] = {
            **sources["inventory_of_tests"],
            "rust_unit_tests": 424242,
            "property_tests": 7,
        }
        entries = dossier.build_entries(sources)
        counts = self.entry(entries, "property-test counts")
        self.assertEqual(counts["value"]["rust_unit_tests"], 424242)
        self.assertEqual(counts["value"]["property_tests"], 7)
        sbom = self.entry(entries, "SBOM and license inventory")
        self.assertEqual(sbom["value"]["license_disposition_blocked"], 0)
        self.assertTrue(sbom["value"]["root_license_text_approved"])

    def test_approved_inventory_reports_zero_blocked_licenses(self) -> None:
        sources = self.live_sources()
        sources["inventory"] = {"packages": [{"license_disposition": "APPROVED"}],
                                "root_license_text_approved": True}
        sbom = self.entry(dossier.build_entries(sources), "SBOM and license inventory")
        self.assertEqual(sbom["state"], "EVIDENCED")
        self.assertEqual(sbom["value"]["license_disposition_blocked"], 0)
        self.assertTrue(sbom["value"]["root_license_text_approved"])

    def test_the_property_count_is_a_counted_zero(self) -> None:
        counts = self.entry(dossier.build_dossier()["entries"], "property-test counts")
        self.assertEqual(counts["state"], "EVIDENCED")
        self.assertEqual(counts["value"]["property_tests"], 0)
        detail = counts["value"]["property_test_detail"]
        self.assertEqual(detail["rust_use_sites"], 0)
        self.assertEqual(detail["python_use_sites"], 0)
        self.assertEqual(detail["manifest_harness_deps"], [])
        self.assertGreaterEqual(detail["scanned_manifests"], 2)
        inventory = json.loads((ROOT / "evidence/validation/test-inventory.json").read_text(encoding="utf-8"))
        self.assertEqual(inventory["property_tests"], 0)

    def test_a_missing_source_fails_closed(self) -> None:
        original = dossier.REGISTER
        dossier.REGISTER = Path("/nonexistent/register.json")
        self.addCleanup(setattr, dossier, "REGISTER", original)
        with self.assertRaises(dossier.DossierError) as error:
            dossier.build_dossier()
        self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_MISSING)


class GateAndAcceptanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.decision = DecisionTests()
        self.decision.setUp() if hasattr(self.decision, "setUp") else None

    def sources(self, **overrides):
        return DecisionTests.sources(self.decision, **overrides)

    def entries(self, **overrides):
        return DecisionTests.entries(self.decision, **overrides)

    def test_pending_ga_criteria_block_without_failing(self) -> None:
        sources = self.sources()
        sources["ga_acceptance"] = {"states": {"EVIDENCED": 50, "AWAITS_REVIEW": 2, "GATED": 0}}
        state, reasons = dossier.derive_decision(sources, self.entries())
        self.assertEqual(state, "BLOCKED")
        self.assertTrue(any("GA acceptance" in line for line in reasons))

    def test_evidenced_ga_criteria_do_not_block(self) -> None:
        sources = self.sources()
        sources["ga_acceptance"] = {"states": {"EVIDENCED": 52, "AWAITS_REVIEW": 0, "GATED": 0}}
        state, _ = dossier.derive_decision(sources, self.entries())
        self.assertEqual(state, "PASS")

    def test_a_failed_gate_fails_while_an_unimplemented_gate_blocks(self) -> None:
        sources = self.sources()
        sources["gates"] = {"release-check": "FAILED", "v2": "OPEN"}
        state, reasons = dossier.derive_decision(sources, self.entries())
        self.assertEqual(state, "FAIL")
        self.assertTrue(any("release-check" in line for line in reasons))
        sources["gates"] = {"release-check": "NOT_IMPLEMENTED", "v2": "OPEN"}
        state, reasons = dossier.derive_decision(sources, self.entries())
        self.assertEqual(state, "BLOCKED")
        self.assertTrue(any("fail-closed" in line for line in reasons))

    def test_unapproved_p2_fails_and_approved_p2_conditions(self) -> None:
        row = {
            "section": "sley",
            "field": "review",
            "severities": ["P2"],
            "state": "FAIL",
            "declares_no_open_p0_p1_p2": False,
        }
        findings = {"declared_open_findings": {"p0": 0, "p1": 0, "p2": 1}}
        sources = self.sources(register={"obligations": [row]})
        state, reasons = dossier.derive_decision(
            sources, self.entries(**{"findings by severity and disposition": findings})
        )
        self.assertEqual(state, "FAIL")
        self.assertTrue(any("sley:review" in line for line in reasons))
        sources = self.sources(
            register={"obligations": [row]},
            summary={"approved_conditional_items": ["sley:review"]},
        )
        state, reasons = dossier.derive_decision(
            sources, self.entries(**{"findings by severity and disposition": findings})
        )
        self.assertEqual(state, "CONDITIONAL_PASS")

    def test_unverifiable_p2_approval_fails_closed(self) -> None:
        findings = {"declared_open_findings": {"p0": 0, "p1": 0, "p2": 1}}
        state, reasons = dossier.derive_decision(
            self.sources(), self.entries(**{"findings by severity and disposition": findings})
        )
        self.assertEqual(state, "FAIL")
        self.assertTrue(any("unverifiable" in line for line in reasons))

    def test_malformed_tracked_sources_raise_source_invalid(self) -> None:
        with self.assertRaises(dossier.DossierError) as error:
            dossier.required({}, "project", "machine-summary")
        self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_INVALID)
        sources = self.sources(thresholds={"pass": "yes"})
        with self.assertRaises(dossier.DossierError) as error:
            dossier.derive_decision(sources, self.entries())
        self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_INVALID)
        sources = self.sources()
        sources["thresholds"] = "PASS"
        with self.assertRaises(dossier.DossierError) as error:
            dossier.derive_decision(sources, self.entries())
        self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_INVALID)

    def test_the_pass_guard_fires_only_behind_closed_gates(self) -> None:
        with self.assertRaises(dossier.DossierError) as error:
            dossier.enforce_pass_guard("PASS", True)
        self.assertEqual(error.exception.code, dossier.DossierErrorCode.DECISION_INVALID)
        dossier.enforce_pass_guard("PASS", False)
        dossier.enforce_pass_guard("BLOCKED", True)

    def test_an_all_null_object_reads_gated_and_absent_evidence_raises(self) -> None:
        item = dossier.entry("probe", value={"a": None, "b": None}, note="probe")
        self.assertEqual(item["state"], "GATED")
        self.assertIsNone(item["value"])
        item = dossier.entry("probe", value={"a": None, "b": 1}, note="probe")
        self.assertEqual(item["state"], "EVIDENCED")
        with self.assertRaises(dossier.DossierError) as error:
            dossier.entry("probe", value=1, evidence=[Path("/nonexistent/x.json")], note="probe")
        self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_MISSING)


class RegisterBoundEntryTests(unittest.TestCase):
    """Items 15 and 32, rule 1's register facts, and the GA report binding."""

    def live_sources(self) -> dict:
        return SourceSeparationTests.live_sources(SourceSeparationTests())

    def entry(self, entries: list[dict], item: str) -> dict:
        return next(entry for entry in entries if entry["item"] == item)

    def security_row(self, register: dict) -> dict:
        return next(
            row
            for row in register["obligations"]
            if row["section"] == "threat_coverage" and row["field"] == "independent_security_review"
        )

    def test_item_15_evidences_only_a_complete_pass_the_register_classifies(self) -> None:
        sources = self.live_sources()
        register = sources["register"]
        row = self.security_row(register)
        register["unclaimed_carried_findings"] = [
            entry
            for entry in register["unclaimed_carried_findings"]
            if not (entry["section"] == "threat_coverage" and entry["field"] == "independent_security_review")
        ]
        for form in ("PASS_PENDING_CONFIRMATION_2_P0_OPEN", "PASSED_TO_NEXT_ROUND", "PASS_2_P1", "PASS_WITH_OPEN_P1"):
            sources["summary"]["threat_coverage"]["independent_security_review"] = form
            row["disposition"] = form
            row["state"] = "PASS"
            item = self.entry(dossier.build_entries(sources), "security review result")
            self.assertEqual(item["state"], "GATED", form)
            self.assertIsNone(item["value"], form)
        sources["summary"]["threat_coverage"]["independent_security_review"] = "PASS"
        sources["summary"]["threat_coverage"]["independent_security_review_note"] = (
            "transcript evidence/review/verdicts/threat_coverage/independent_security_review-43f2f5b.md"
        )
        row["disposition"] = "PASS"
        row["state"] = "PASS"
        row["severities"] = []
        item = self.entry(dossier.build_entries(sources), "security review result")
        self.assertEqual(item["state"], "EVIDENCED")
        self.assertEqual(item["value"], "PASS")
        self.assertIn("evidence/review/finding-register.json", item["evidence"])
        self.assertIn(
            "evidence/review/verdicts/threat_coverage/independent_security_review-43f2f5b.md", item["evidence"]
        )

    def test_item_15_is_gated_by_unclaimed_follow_ups(self) -> None:
        sources = self.live_sources()
        row = self.security_row(sources["register"])
        row.update(disposition="PASS_0_P0_0_P1_0_P2_1_P3", state="PASS", severities=["P3"])
        sources["register"]["unclaimed_carried_findings"].append(dict(row, unclaimed_severities=["P3"]))
        sources["summary"]["threat_coverage"]["independent_security_review"] = row["disposition"]
        sources["summary"]["threat_coverage"]["independent_security_review_note"] = (
            "transcript evidence/review/verdicts/threat_coverage/independent_security_review-43f2f5b.md"
        )
        item = self.entry(dossier.build_entries(sources), "security review result")
        self.assertEqual(item["state"], "GATED")
        self.assertIn("unclaimed findings", item["note"])
        self.assertIn(
            "evidence/review/verdicts/threat_coverage/independent_security_review-43f2f5b.md", item["evidence"]
        )

    def test_a_missing_named_transcript_fails_the_build(self) -> None:
        sources = self.live_sources()
        sources["summary"]["threat_coverage"]["independent_security_review_note"] = (
            "PASS; transcript evidence/review/verdicts/threat_coverage/independent_security_review-0000000.md"
        )
        with self.assertRaises(dossier.DossierError) as error:
            dossier.build_entries(sources)
        self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_MISSING)

    def test_item_32_requires_a_complete_pass_over_a_clear_register(self) -> None:
        sources = self.live_sources()
        sources["summary"]["finding_register"]["independent_review"] = "PASS"
        sources["register"]["result"] = "FINDING_REGISTER_OPEN"
        item = self.entry(dossier.build_entries(sources), "independent review result")
        self.assertEqual(item["state"], "GATED")
        self.assertEqual(item["evidence"], [])
        sources["register"]["result"] = "FINDING_REGISTER_CLEAR"
        item = self.entry(dossier.build_entries(sources), "independent review result")
        self.assertEqual(item["state"], "EVIDENCED")
        self.assertEqual(item["value"], "PASS")
        self.assertIn("evidence/review/finding-register.json", item["evidence"])
        sources["summary"]["finding_register"]["independent_review"] = "PASS_WITH_OPEN_P1"
        item = self.entry(dossier.build_entries(sources), "independent review result")
        self.assertEqual(item["state"], "GATED")

    def test_the_findings_entry_carries_the_register_result_and_other_rows(self) -> None:
        item = self.entry(dossier.build_dossier()["entries"], "findings by severity and disposition")
        register = json.loads((ROOT / "evidence/review/finding-register.json").read_text(encoding="utf-8"))
        self.assertEqual(item["value"]["result"], register["result"])
        self.assertEqual(item["value"]["unclassified"], len(register["unclassified"]))
        self.assertEqual(
            item["value"]["unclaimed_carried_findings"], len(register["unclaimed_carried_findings"])
        )

    def test_historical_rounds_are_not_open_p2_rows(self) -> None:
        rows = [
            {"section": "s", "field": "old", "severities": ["P2"], "state": "HISTORICAL_ROUND"},
            {"section": "s", "field": "live", "severities": ["P2"], "state": "PENDING"},
            {"section": "s", "field": "done", "severities": ["P2"], "state": "PASS"},
        ]
        self.assertEqual(dossier.open_p2_rows({"obligations": rows}, set()), ["s:live"])

    def test_missing_binding_on_both_documents_is_not_equal_binding(self) -> None:
        original_load = dossier.load
        for value in (None, "", "not-a-digest"):
            def modified_load(path, *args):
                document = original_load(path, *args)
                if path in (dossier.REGISTER, dossier.GA_ACCEPTANCE):
                    if value is None:
                        document.pop("register_digest", None)
                    else:
                        document["register_digest"] = value
                    if path == dossier.GA_ACCEPTANCE:
                        document.pop("report_digest")
                        document["report_digest"] = dossier.digest_of(document)
                return document
            with self.subTest(value=value), patch.object(dossier, "load", side_effect=modified_load):
                with self.assertRaises(dossier.DossierError) as error:
                    dossier.build_dossier()
                self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_INVALID)

    def test_a_tampered_or_rebound_ga_report_fails_closed(self) -> None:
        import tempfile

        original = dossier.GA_ACCEPTANCE
        self.addCleanup(setattr, dossier, "GA_ACCEPTANCE", original)
        report = json.loads(original.read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ga.json"
            tampered = dict(report)
            tampered["states"] = {"EVIDENCED": 52}
            path.write_text(dossier.canonical(tampered), encoding="utf-8")
            dossier.GA_ACCEPTANCE = path
            with self.assertRaises(dossier.DossierError) as error:
                dossier.build_dossier()
            self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_INVALID)
            self.assertIn("report_digest", error.exception.detail)
            rebound = {key: value for key, value in report.items() if key != "report_digest"}
            rebound["register_digest"] = "0" * 64
            rebound["report_digest"] = dossier.digest_of(rebound)
            path.write_text(dossier.canonical(rebound), encoding="utf-8")
            with self.assertRaises(dossier.DossierError) as error:
                dossier.build_dossier()
            self.assertEqual(error.exception.code, dossier.DossierErrorCode.SOURCE_INVALID)
            self.assertIn("register_digest", error.exception.detail)


if __name__ == "__main__":
    unittest.main()
