"""Offline tests of the GA acceptance report: derived states, forms, lanes, binding."""

from __future__ import annotations

import copy
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

from bench.accounting import report as accounting

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "build_ga_acceptance_report", ROOT / "scripts/build_ga_acceptance_report.py"
)
assert SPEC is not None and SPEC.loader is not None
ga = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ga)

BAD_PASS_FORMS = (
    "PASS_PENDING_CONFIRMATION_2_P0_OPEN",
    "PASSED_TO_NEXT_ROUND",
    "PASS_2_P1",
    "PASS_WITH_OPEN_P1",
    "PASS_0_P0",
    "PASS_0_P0_0_P1",
    "PASS_0_P0_0_P1_0_P2_0_P3_0_P3",
)


def state_of(report: dict, criterion: str) -> str:
    for entry in report["criteria"]:
        if entry["criterion"] == criterion:
            return entry["state"]
    raise AssertionError(f"no criterion {criterion!r}")


def evidence_of(report: dict, criterion: str) -> str:
    for entry in report["criteria"]:
        if entry["criterion"] == criterion:
            return entry["evidence"]
    raise AssertionError(f"no criterion {criterion!r}")


def security_row(register: dict) -> dict:
    return next(
        row
        for row in register["obligations"]
        if row["section"] == "threat_coverage" and row["field"] == "independent_security_review"
    )


def unflag(register: dict, section: str, field: str) -> None:
    for key in ("unclaimed_carried_findings", "unclassified"):
        register[key] = [
            row for row in register[key] if not (row["section"] == section and row["field"] == field)
        ]


class Fixture(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.live = ga.load_sources()

    def sources(self) -> dict:
        return copy.deepcopy(self.live)


class DerivationTests(Fixture):
    """The previously hard-coded criteria fall under contrary facts."""

    def test_the_report_is_a_pure_function_with_fifty_two_criteria(self) -> None:
        report = ga.build_report(self.sources())
        self.assertEqual(report["criterion_count"], 52)
        self.assertEqual(sum(report["states"].values()), 52)
        self.assertEqual(report, ga.build_report(self.sources()))
        self.assertFalse(report["ga_claimed"])
        self.assertTrue(ga.verify_report_digest(report))

    def test_missing_or_malformed_register_digests_refuse(self) -> None:
        for key in ("register_digest", "obligations_digest"):
            for value in (None, "", "not-a-digest"):
                sources = self.sources()
                sources["register"][key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    ga.build_report(sources)

    def test_missing_candidate_gates_all_packaging_facts(self) -> None:
        sources = self.sources()
        sources["repro"]["attestations"] = []
        report = ga.build_report(sources)
        for row in report["criteria"]:
            if row["group"] == "26.8 packaging":
                self.assertEqual(row["state"], ga.GATED, row["criterion"])

    def test_no_criterion_is_a_constant(self) -> None:
        def mutate(name: str):
            sources = self.sources()
            summary = sources["summary"]
            attestation = sources["repro"]["attestations"][0]
            if name == "Sley 1.2.0 is preserved and checksum-verified":
                summary["legacy_freeze"]["artifact_sha256"] = None
            elif name == "No Sley 1.2.1 implementation was required":
                sources["legacy_adr_present"] = False
            elif name == "The new repository has independent history":
                summary["m0_commit"] = ""
            elif name == "Rust and independent oracle produce byte-identical output":
                summary["conformance"]["cross_implementation_agreement"] = "FAIL"
            elif name == "Every non-canonical fixture is rejected":
                sources["conformance"]["independently_checked"] = 3
            elif name == "Hashes are domain separated":
                summary["identifiers"]["status"] = "S20_110_IN_PROGRESS"
                sources["register"]["complete_packages"] = [
                    section for section in sources["register"]["complete_packages"] if section != "identifiers"
                ]
            elif name in ("invalid candidates cannot commit", "exact stale conflicts are rejected"):
                summary["s20_360_candidate_validation"]["status"] = "IN_PROGRESS"
            elif name == "machine outputs use stable codes and contracts":
                sources["symbols"]["result"] = "FAIL"
            elif name == "crash recovery produces only old or complete new state":
                summary["s20_530_crash_recovery"]["matrix_rows"] = None
            elif name == "opacity is not used as a security argument":
                for entry in sources["anti_goals"]["anti_goals"]:
                    if entry["anti_goal"] == "opacity as security":
                        entry["state"] = "REVIEW_ONLY"
            elif name == f"artifact name {ga.ARTIFACT_NAME}":
                sources["repro"]["attestations"] = []
            elif name == "artifact runs with no source-tree access":
                summary["release_candidate_packaging"]["candidate_demo"] = "FAIL_3_STEPS"
            elif name == "artifact contains no secrets, local paths, caches, or debug files":
                sources["candidate_content"] = None
            elif name == "manifest, SHA-256, size, SBOM, license inventory, and provenance are recorded":
                sources["provenance"] = None
            elif name == "second clean build establishes reproducibility":
                sources["repro"]["result"] = "SINGLE_HOST_REPRODUCIBLE"
            elif name == "the source working tree is clean":
                for row in sources["repro"]["attestations"]:
                    row["working_tree_clean"] = False
            elif name == "publication remains unauthorized unless separately granted":
                summary["publication_authorized"] = True
            else:
                raise AssertionError(name)
            return sources

        previously_constant = (
            "Sley 1.2.0 is preserved and checksum-verified",
            "No Sley 1.2.1 implementation was required",
            "The new repository has independent history",
            "Rust and independent oracle produce byte-identical output",
            "Every non-canonical fixture is rejected",
            "Hashes are domain separated",
            "invalid candidates cannot commit",
            "exact stale conflicts are rejected",
            "machine outputs use stable codes and contracts",
            "crash recovery produces only old or complete new state",
            "opacity is not used as a security argument",
            f"artifact name {ga.ARTIFACT_NAME}",
            "artifact runs with no source-tree access",
            "artifact contains no secrets, local paths, caches, or debug files",
            "manifest, SHA-256, size, SBOM, license inventory, and provenance are recorded",
            "second clean build establishes reproducibility",
            "the source working tree is clean",
            "publication remains unauthorized unless separately granted",
        )
        self.assertEqual(len(previously_constant), 18)
        for name in previously_constant:
            report = ga.build_report(mutate(name))
            self.assertNotEqual(state_of(report, name), ga.EVIDENCED, name)

    def test_contrary_facts_read_in_the_evidence_string(self) -> None:
        sources = self.sources()
        sources["repro"]["result"] = "PACKAGE_NOT_REPRODUCIBLE"
        for row in sources["repro"]["attestations"]:
            row["working_tree_clean"] = None
        sources["summary"]["conformance"]["cross_implementation_agreement"] = "FAIL"
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, "second clean build establishes reproducibility"), ga.GATED)
        self.assertIn("PACKAGE_NOT_REPRODUCIBLE", evidence_of(report, "second clean build establishes reproducibility"))
        self.assertEqual(state_of(report, "the source working tree is clean"), ga.GATED)
        self.assertEqual(
            state_of(report, "Rust and independent oracle produce byte-identical output"), ga.AWAITS_REVIEW
        )

    def test_opacity_is_evidenced_only_when_the_anti_goal_holds(self) -> None:
        sources = self.sources()
        for entry in sources["anti_goals"]["anti_goals"]:
            if entry["anti_goal"] == "opacity as security":
                entry["state"] = "REVIEW_ONLY"
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, "opacity is not used as a security argument"), ga.AWAITS_REVIEW)
        for entry in sources["anti_goals"]["anti_goals"]:
            if entry["anti_goal"] == "opacity as security":
                entry["state"] = "HOLDS"
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, "opacity is not used as a security argument"), ga.EVIDENCED)


class RegisterPredicateTests(Fixture):
    """Completion, verdict forms, and lane clearance are the register's."""

    def test_package_completion_is_the_registers_predicate(self) -> None:
        sources = self.sources()
        report = ga.build_report(sources)
        # A mid-string COMPLETE names a restricted boundary, not completion.
        sources["summary"]["s20_500_native_refs_branches"]["status"] = "S20_500_COMPLETE_RESTRICTED"
        sources["register"]["complete_packages"] = [
            name for name in sources["register"]["complete_packages"] if name != "s20_500_native_refs_branches"
        ]
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, "refs update atomically"), ga.AWAITS_REVIEW)
        self.assertNotIn("s20_500_native_refs_branches", sources["register"]["complete_packages"])
        sources["summary"]["s20_500_native_refs_branches"]["status"] = "S20_500_COMPLETE"
        # A terminal status the register has not listed does not count either.
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, "refs update atomically"), ga.AWAITS_REVIEW)
        sources["register"]["complete_packages"].append("s20_500_native_refs_branches")
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, "refs update atomically"), ga.EVIDENCED)
        self.assertEqual(state_of(report, "branches preserve ancestry"), ga.EVIDENCED)

    def test_only_a_bare_or_all_zero_pass_is_a_complete_pass(self) -> None:
        self.assertTrue(ga.complete_pass_form("PASS"))
        self.assertTrue(ga.complete_pass_form("PASS_0_P0_0_P1_0_P2_0_P3"))
        self.assertTrue(ga.complete_pass_form("PASS_0_P0_0_P1_0_P2_0_P3_0_P4"))
        self.assertFalse(ga.complete_pass_form("PASS_0_P0_0_P1_0_P2_3_P3_5_P4"))
        self.assertFalse(ga.complete_pass_form("PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4"))
        self.assertFalse(ga.complete_pass_form("VULCAN_PASS"))
        self.assertFalse(ga.complete_pass_form(None))
        for form in BAD_PASS_FORMS:
            self.assertFalse(ga.complete_pass_form(form), form)

    def test_bad_pass_forms_never_evidence_the_security_criteria(self) -> None:
        criteria = (
            "all P0/P1 threats have passing tests",
            "No ambient authority exists",
            "capability tokens cannot be forged or replayed across scope",
        )
        builder = ga.register_builder()
        for form in BAD_PASS_FORMS:
            sources = self.sources()
            register = sources["register"]
            row = security_row(register)
            unflag(register, "threat_coverage", "independent_security_review")
            sources["summary"]["threat_coverage"]["independent_security_review"] = form
            row["disposition"] = form
            row["state"] = builder.classify_token(form)
            row["severities"] = builder.severities_of(form)
            report = ga.build_report(sources)
            for criterion in criteria:
                self.assertNotEqual(state_of(report, criterion), ga.EVIDENCED, (form, criterion))
        # Control: the bare token over a row the register neither flags nor
        # rejects evidences the criterion that depends on the verdict alone.
        sources = self.sources()
        register = sources["register"]
        row = security_row(register)
        unflag(register, "threat_coverage", "independent_security_review")
        sources["summary"]["threat_coverage"]["independent_security_review"] = "PASS"
        row["disposition"] = "PASS"
        row["state"] = "PASS"
        row["severities"] = []
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, "all P0/P1 threats have passing tests"), ga.EVIDENCED)

    def test_security_verdict_naming_follow_ups_is_not_a_complete_pass(self) -> None:
        sources = self.sources()
        row = security_row(sources["register"])
        row.update(disposition="PASS_0_P0_0_P1_0_P2_1_P3", state="PASS", severities=["P3"])
        sources["summary"]["threat_coverage"]["independent_security_review"] = row["disposition"]
        sources["register"]["unclaimed_carried_findings"].append(dict(row, unclaimed_severities=["P3"]))
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, "all P0/P1 threats have passing tests"), ga.AWAITS_REVIEW)
        self.assertIn("unclaimed findings", evidence_of(report, "all P0/P1 threats have passing tests"))

    def test_the_independent_review_needs_a_complete_pass_over_a_clear_register(self) -> None:
        criterion = "Vulcan or current independent reviewer issues a complete PASS"
        for form in BAD_PASS_FORMS:
            sources = self.sources()
            sources["summary"]["finding_register"]["independent_review"] = form
            sources["register"]["result"] = "FINDING_REGISTER_CLEAR"
            self.assertNotEqual(state_of(ga.build_report(sources), criterion), ga.EVIDENCED, form)
        sources = self.sources()
        sources["summary"]["finding_register"]["independent_review"] = "PASS"
        sources["register"]["result"] = "FINDING_REGISTER_OPEN"
        self.assertEqual(state_of(ga.build_report(sources), criterion), ga.AWAITS_REVIEW)
        sources["register"]["result"] = "FINDING_REGISTER_CLEAR"
        self.assertEqual(state_of(ga.build_report(sources), criterion), ga.EVIDENCED)

    def test_lane_clearance_honours_unclaimed_and_unclassified_rows(self) -> None:
        criterion = "Nabu approves architectural and cross-product boundaries"
        sources = self.sources()
        register = sources["register"]
        register["obligations"] = [{
            "section": "fixture", "field": "nabu_architecture_review",
            "reviewer": "nabu", "state": "PASS", "disposition": "PASS",
        }]
        register["unclaimed_carried_findings"] = []
        register["unclassified"] = []
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, criterion), ga.EVIDENCED)
        nabu_pass = next(
            row for row in register["obligations"] if row["reviewer"] == "nabu" and row["state"] == "PASS"
        )
        register["unclaimed_carried_findings"] = [
            {
                "section": nabu_pass["section"],
                "field": nabu_pass["field"],
                "disposition": "PASS_WITH_2_P1_FOLLOWUPS",
                "unclaimed_severities": ["P1"],
            }
        ]
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, criterion), ga.AWAITS_REVIEW)
        self.assertIn("1 unclaimed or unclassified", evidence_of(report, criterion))
        register["unclaimed_carried_findings"] = []
        register["unclassified"] = [
            {"section": nabu_pass["section"], "field": nabu_pass["field"], "disposition": "MAYBE"}
        ]
        self.assertEqual(state_of(ga.build_report(sources), criterion), ga.AWAITS_REVIEW)
        register["unclassified"] = []
        nabu_pass["state"] = "PENDING"
        self.assertEqual(state_of(ga.build_report(sources), criterion), ga.AWAITS_REVIEW)

    def test_lanes_read_open_while_the_register_lists_unclaimed_rows(self) -> None:
        sources = self.sources()
        for role in ("ariadne", "nabu"):
            row = {"section": "fixture", "field": role + "_review", "reviewer": role,
                   "state": "PASS", "disposition": "PASS_0_P0_0_P1_0_P2_1_P3"}
            sources["register"]["obligations"].append(row)
            sources["register"]["unclaimed_carried_findings"].append(dict(row, unclaimed_severities=["P3"]))
        report = ga.build_report(sources)
        for criterion in (
            "Ariadne approves SSMC1 and semantic correctness",
            "Nabu approves architectural and cross-product boundaries",
        ):
            self.assertEqual(state_of(report, criterion), ga.AWAITS_REVIEW)
            self.assertIn("lane open", evidence_of(report, criterion))


class ReleaseAndThresholdTests(Fixture):

    def test_current_candidate_selection_is_shared_with_the_dossier(self) -> None:
        from scripts import build_reproducibility_report as repro_builder
        from bench.review.tests.test_decision_dossier import SourceSeparationTests, dossier
        current = copy.deepcopy(self.live["repro"]["attestations"][0])
        current["host_label"] = "primary"
        secondary = {**current, "host_label": "secondary"}
        archive = {**current, "host_label": "archive", "commit": "d" * 40}
        for label, attestations, expected in (
            ("archived host", [archive, current, secondary], current["commit"]),
            ("tie", [archive, current], None),
            ("dirty", [{**current, "working_tree_clean": False}], None),
            ("absent", [], None),
        ):
            with self.subTest(label=label):
                report = dict(self.live["repro"], attestations=attestations)
                selected = repro_builder.select_attestation(report)
                self.assertEqual(selected["commit"] if selected else None, expected)
                sources = self.sources()
                sources["repro"] = report
                sources["summary"]["release_decision"] = {
                    "state": "RELEASE_APPROVED", "final_commit": current["commit"]
                }
                criterion = "artifact is built from the final candidate commit"
                self.assertEqual(state_of(ga.build_report(sources), criterion), ga.EVIDENCED if expected else ga.GATED)
                dossier_sources = SourceSeparationTests().live_sources()
                dossier_sources["repro"] = report
                entries = dossier.build_entries(dossier_sources)
                actual = next(entry for entry in entries if entry["item"] == "final commit")
                self.assertEqual(actual["value"], expected)

    def test_artifact_safety_needs_bound_artifact_checks(self) -> None:
        sources = self.sources()
        attestation = sources["repro"]["attestations"][0]
        content = {
            "contract": "sley2.candidate-content-checks.v1",
            **{key: attestation[key] for key in ("commit", "artifact_sha256", "manifest_digest", "artifact_size_bytes")},
            "result": "PASS",
            "checks": {"manifest": True, "forbidden_content": True},
        }
        criterion = "artifact contains no secrets, local paths, caches, or debug files"
        for change in (None, {"result": "FAIL"}, {"commit": "0" * 40}, {"checks": {"manifest": False, "forbidden_content": True}}, {"checks": {}}, {"report_digest": "0" * 64}):
            sources["candidate_content"] = None if change is None else dict(content, **change)
            if change is not None and "report_digest" not in change:
                sources["candidate_content"]["report_digest"] = ga.digest_of(sources["candidate_content"])
            self.assertNotEqual(state_of(ga.build_report(sources), criterion), ga.EVIDENCED, change)
        sources["candidate_content"] = dict(content, report_digest=ga.digest_of(content))
        self.assertEqual(state_of(ga.build_report(sources), criterion), ga.EVIDENCED)
        sources["repro"]["attestations"] = []
        self.assertNotEqual(state_of(ga.build_report(sources), criterion), ga.EVIDENCED)

    def test_the_release_approved_path_does_not_crash(self) -> None:
        criterion = "artifact is built from the final candidate commit"
        sources = self.sources()
        commit = sources["repro"]["attestations"][0]["commit"]
        sources["summary"]["release_decision"] = {"state": "RELEASE_APPROVED", "final_commit": commit}
        report = ga.build_report(sources)
        self.assertEqual(state_of(report, criterion), ga.EVIDENCED)
        sources["summary"]["release_decision"] = {"state": "RELEASE_APPROVED", "final_commit": "0" * 40}
        self.assertEqual(state_of(ga.build_report(sources), criterion), ga.GATED)
        sources["summary"]["release_decision"] = {"state": "RELEASE_APPROVED", "final_commit": commit}
        sources["repro"]["attestations"] = []
        self.assertEqual(state_of(ga.build_report(sources), criterion), ga.GATED)

    def test_the_threshold_verdict_reads_the_tracked_accounting_report(self) -> None:
        original = ga.ACCOUNTING_REPORT
        self.addCleanup(setattr, ga, "ACCOUNTING_REPORT", original)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "succession-accounting-report.json"
            ga.ACCOUNTING_REPORT = path
            verdict = ga.succession_thresholds()
            self.assertFalse(verdict["pass"])
            self.assertFalse(verdict["present"])
            report = {
                "contract": ga.ACCOUNTING_CONTRACT,
                "status": "COMPLETE",
                "evidence_status": accounting.EVIDENCE_VERIFIED,
                "claim_statuses": {name: ["VERIFIED"] for name in accounting.STATUS_FIELDS},
                "thresholds": {
                    name: {"result": "PASS"}
                    for name in set(json.loads((ROOT / "bench/benchmark-plan.json").read_text())["thresholds"])
                    | accounting.NOT_EVALUATED_NAMES
                },
            }
            report["report_digest"] = accounting.report_digest(report)
            accounting.verify_report(report)
            path.write_text(json.dumps(report), encoding="utf-8")
            verdict = ga.succession_thresholds()
            self.assertTrue(verdict["pass"])
            self.assertEqual(set(verdict["rows"]), set(report["thresholds"]))
            for change in (
                {"status": "PARTIAL"},
                {"evidence_status": "VERIFIED"},
                {"evidence_status": accounting.EVIDENCE_UNVERIFIED},
                {"report_digest": "0" * 64},
                {"contract": "other"},
                {"thresholds": {"a": {"result": "PASS"}, "b": {"result": "UNDETERMINED"}}},
                {"thresholds": {}},
            ):
                changed = {**report, **change}
                if "report_digest" not in change:
                    changed["report_digest"] = accounting.report_digest(changed)
                path.write_text(json.dumps(changed), encoding="utf-8")
                self.assertFalse(ga.succession_thresholds()["pass"], change)
            # The GA criterion reads the same verdict.
            sources = self.sources()
            sources["thresholds"] = {"pass": True, "note": "every row PASS", "source": "probe"}
            self.assertEqual(state_of(ga.build_report(sources), "every section 22 threshold passes"), ga.EVIDENCED)
            sources["thresholds"] = {"pass": False, "note": "absent", "source": "probe"}
            self.assertEqual(state_of(ga.build_report(sources), "every section 22 threshold passes"), ga.GATED)

    def test_summary_hand_keys_do_not_evidence_the_thresholds(self) -> None:
        sources = self.sources()
        sources["summary"]["succession"]["thresholds"] = {"strict_correctness": "PASS"}
        sources["summary"]["succession"]["trials_executed"] = 1
        sources["summary"]["succession"]["thresholds_pass"] = True
        self.assertEqual(state_of(ga.build_report(sources), "every section 22 threshold passes"), ga.GATED)


class BindingTests(Fixture):
    def test_the_register_digest_is_recorded(self) -> None:
        report = ga.build_report(self.sources())
        self.assertEqual(report["register_digest"], self.live["register"]["register_digest"])
        self.assertEqual(report["obligations_digest"], self.live["register"]["obligations_digest"])
        self.assertTrue(ga.verify_report_digest(report))
        tampered = dict(report)
        tampered["states"] = {"EVIDENCED": 52}
        self.assertFalse(ga.verify_report_digest(tampered))

    def test_check_detects_drift(self) -> None:
        original = ga.REPORT
        self.addCleanup(setattr, ga, "REPORT", original)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ga-acceptance-report.json"
            ga.REPORT = path
            self.assertEqual(ga.main(["--check"]), 1)
            text = ga.canonical(ga.build_report())
            path.write_text(text, encoding="utf-8")
            self.assertEqual(ga.main(["--check"]), 0)
            path.write_text(text.replace('"ga_claimed": false', '"ga_claimed": true'), encoding="utf-8")
            self.assertEqual(ga.main(["--check"]), 1)
            path.write_text(text[:-1], encoding="utf-8")
            self.assertEqual(ga.main(["--check"]), 1)


if __name__ == "__main__":
    unittest.main()
