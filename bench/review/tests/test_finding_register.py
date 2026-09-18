"""Offline tests of the S20-740 finding register: derivation, states, invariants."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "build_finding_register", ROOT / "scripts/build_finding_register.py"
)
assert SPEC is not None and SPEC.loader is not None
register = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(register)


class ClassificationTests(unittest.TestCase):
    def test_heads_are_exact(self) -> None:
        for disposition, state in (
            ("PASS", "PASS"),
            ("PASS_NO_OPEN_P0_P1_P2", "PASS"),
            ("VULCAN_PASS", "PASS"),
            ("PENDING", "PENDING"),
            ("DEFERRED_FORGE_OAUTH_401", "DEFERRED"),
            ("FAIL_2_P1_1_P2", "FAIL_ROUND"),
            ("REVISE_TO_RESTRICTED_PROFILE", "FAIL_ROUND"),
            ("TIMED_OUT_PARTIAL_HANDOFF", "OTHER"),
            # First-token matching, not prefix matching: PASSIVE is no state.
            ("PASSIVE_OBSERVATION", "OTHER"),
            ("PENDINGNESS", "OTHER"),
        ):
            self.assertEqual(register.classify_token(disposition), state, disposition)

    def test_a_pass_claiming_an_open_finding_is_other(self) -> None:
        self.assertEqual(
            register.classify_token("PASS_PENDING_CONFIRMATION_2_P0_OPEN"), "OTHER"
        )
        self.assertEqual(
            register.classify_token("PASS_FINDINGS_CLOSED_NO_OPEN_P0_P1_P2"), "PASS"
        )

    def test_negations_carry_no_severity(self) -> None:
        self.assertEqual(register.severities_of("PASS_NO_OPEN_P0_P1_P2"), [])
        self.assertEqual(
            register.severities_of("PASS_PRIOR_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4"),
            ["P2", "P3"],
        )
        self.assertEqual(register.severities_of("FAIL_2_P0_8_P1_9_P2_8_P3"), ["P0", "P1", "P2", "P3"])

    def test_zero_counts_carry_no_severity(self) -> None:
        self.assertEqual(
            register.severities_of("PASS_0_P0_0_P1_0_P2_0_P3"), []
        )
        self.assertEqual(
            register.severities_of("FAIL_0_P0_5_P1_4_P2_4_P3"), ["P1", "P2", "P3"]
        )
        self.assertEqual(
            register.severities_of("REVISE_0_P0_2_P1_3_P2_3_P3"), ["P1", "P2", "P3"]
        )

    def test_reviewer_tokens(self) -> None:
        self.assertEqual(register.reviewer_of("nabu_architecture_review"), "nabu")
        self.assertEqual(register.reviewer_of("vulcan_surface_review"), "vulcan")
        self.assertIsNone(register.reviewer_of("focused_semantic_security_review"))

    def test_roles_sessions_instants_and_the_registers_own_verdict_are_not_obligations(
        self,
    ) -> None:
        self.assertFalse(register.is_obligation("any", "reviewer_role", "vulcan"))
        self.assertFalse(
            register.is_obligation("any", "review_session_id", "forge-vulcan-s20-540-x")
        )
        self.assertFalse(
            register.is_obligation("any", "vulcan_review", "2026-09-02T17:42:11.513Z")
        )
        self.assertFalse(
            register.is_obligation("any", "merge_engine_reviews", "S20-520 reviews pending")
        )
        self.assertFalse(register.is_obligation("any", "status", "PENDING"))
        self.assertFalse(
            register.is_obligation("finding_register", "independent_review", "PENDING")
        )
        # Any other section's verdict field stays collected: no other checker
        # asserts it, so dropping it would hide an open review.
        self.assertTrue(
            register.is_obligation("invariant_audit", "independent_review", "PENDING")
        )
        self.assertTrue(register.is_obligation("any", "vulcan_review", "PENDING"))
        self.assertTrue(
            register.is_obligation(
                "any", "final_argus_disposition", "DEFERRED_FORGE_OAUTH_401"
            )
        )

    def test_supersession_runs_early_or_qualified_toward_late_or_general(self) -> None:
        self.assertTrue(register.supersedes("vulcan_review", "vulcan_first_review"))
        self.assertTrue(register.supersedes("ariadne_final_review", "ariadne_initial_review"))
        self.assertTrue(
            register.supersedes(
                "ariadne_contract_review", "ariadne_contract_review_revision_1"
            )
        )
        self.assertTrue(
            register.supersedes("ariadne_review", "ariadne_contract_review_revision_1")
        )
        self.assertTrue(
            register.supersedes(
                "ariadne_profile_final_review", "ariadne_initial_review"
            )
        )
        # Two unmarked rounds never fold across cores: an older general
        # PASS cannot close a newer qualified FAIL without round evidence.
        self.assertFalse(
            register.supersedes("ariadne_review", "ariadne_operation_analysis_review")
        )
        # A scoped PASS never folds a round from another subject.
        self.assertFalse(
            register.supersedes(
                "vulcan_entity_read_review", "vulcan_surface_review_revision_1"
            )
        )
        self.assertTrue(
            register.supersedes(
                "vulcan_entity_read_review", "vulcan_entity_read_review_revision_1"
            )
        )
        # A slice PASS never closes the overall FAIL it belongs to.
        self.assertFalse(
            register.supersedes("candidate_result_vulcan_review", "vulcan_review")
        )
        self.assertFalse(register.supersedes("vulcan_review", "vulcan_review"))

    def test_complete_statuses_exclude_incomplete(self) -> None:
        self.assertTrue(register.is_complete_status("S20_999_COMPLETE"))
        self.assertFalse(register.is_complete_status("S20_999_INCOMPLETE"))
        self.assertFalse(register.is_complete_status("S20_999_NOT_COMPLETE"))
        self.assertFalse(register.is_complete_status("S20_999_IMPLEMENTED_REVIEW_PENDING"))


class DerivationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.register = register.build_register()

    def test_the_register_describes_the_tracked_summary(self) -> None:
        self.assertEqual(self.register["contract"], register.CONTRACT)
        self.assertEqual(self.register["contract_revision"], register.CONTRACT_REVISION)
        self.assertEqual(self.register["work_package"], "S20-740")
        self.assertEqual(self.register["obligation_count"], len(self.register["obligations"]))
        self.assertEqual(
            sum(self.register["states"].values()), self.register["obligation_count"]
        )
        self.assertEqual(
            self.register["obligations_digest"],
            register.digest_of(self.register["obligations"]),
        )
        self.assertEqual(
            self.register["register_digest"],
            register.digest_of(
                {key: value for key, value in self.register.items() if key != "register_digest"}
            ),
        )

    def test_open_and_deferred_lists_agree_with_the_states(self) -> None:
        self.assertEqual(
            len(self.register["open_reviews"]), self.register["states"].get("PENDING", 0)
        )
        self.assertEqual(
            len(self.register["deferred_reviews"]), self.register["states"].get("DEFERRED", 0)
        )
        self.assertEqual(
            len(self.register["unclassified"]), self.register["states"].get("OTHER", 0)
        )
        self.assertEqual(
            len(self.register["superseded_rounds"]),
            self.register["states"].get("HISTORICAL_ROUND", 0),
        )

    def test_open_reviews_carry_disposition_and_severities(self) -> None:
        for entry in self.register["open_reviews"]:
            self.assertEqual(
                set(entry), {"section", "field", "disposition", "severities"}
            )

    def test_no_completed_package_carries_an_open_review(self) -> None:
        self.assertEqual(self.register["complete_packages_with_open_reviews"], [])
        self.assertTrue(self.register["complete_packages"])

    def test_the_result_reflects_the_open_reviews(self) -> None:
        expected = (
            "FINDING_REGISTER_CLEAR"
            if not self.register["open_reviews"] and not self.register["unclassified"]
            else "FINDING_REGISTER_OPEN"
        )
        self.assertEqual(self.register["result"], expected)

    def test_the_register_is_a_pure_function_of_the_summary(self) -> None:
        self.assertEqual(self.register, register.build_register())

    def test_recording_the_registers_own_counts_does_not_change_it(self) -> None:
        # The summary records this register's counts, so the derivation must be
        # stable under a counter update or it would never reach a fixed point.
        original = register.SUMMARY
        summary = json.loads(original.read_text(encoding="utf-8"))
        summary["finding_register"]["open_reviews"] = 4242
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "summary.json"
            path.write_text(json.dumps(summary), encoding="utf-8")
            register.SUMMARY = path
            self.addCleanup(setattr, register, "SUMMARY", original)
            self.assertEqual(register.build_register(), self.register)

    def test_no_host_path_or_instant_appears(self) -> None:
        text = register.canonical(self.register)
        for marker in ("/home/", "greyforge", "file://"):
            self.assertNotIn(marker, text)


class InvariantTests(unittest.TestCase):
    def build_from(self, summary: dict) -> dict:
        original = register.SUMMARY
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "summary.json"
            path.write_text(json.dumps(summary), encoding="utf-8")
            register.SUMMARY = path
            self.addCleanup(setattr, register, "SUMMARY", original)
            return register.build_register()

    def build_fails(self, summary: dict) -> register.RegisterError:
        original = register.SUMMARY
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "summary.json"
            path.write_text(json.dumps(summary), encoding="utf-8")
            register.SUMMARY = path
            self.addCleanup(setattr, register, "SUMMARY", original)
            with self.assertRaises(register.RegisterError) as error:
                register.build_register()
            return error.exception

    def test_a_completed_package_with_a_pending_review_fails_closed(self) -> None:
        error = self.build_fails(
            {
                "example_package": {
                    "status": "S20_999_COMPLETE",
                    "vulcan_review": "PENDING",
                }
            }
        )
        self.assertEqual(error.code, register.RegisterErrorCode.COMPLETION_VIOLATION)

    def test_an_unsuperseded_failure_in_a_completed_package_fails_closed(self) -> None:
        error = self.build_fails(
            {
                "example_package": {
                    "status": "S20_999_COMPLETE",
                    "nabu_architecture_review": "REVISE_TO_RESTRICTED_PROFILE",
                    "ariadne_final_review": "PASS",
                }
            }
        )
        self.assertEqual(error.code, register.RegisterErrorCode.COMPLETION_VIOLATION)

    def test_a_completed_package_may_carry_a_superseded_failed_round(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_COMPLETE",
                    "vulcan_first_review": "FAIL_2_P1",
                    "vulcan_review": "PASS",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_CLEAR")
        self.assertEqual(derived["states"]["HISTORICAL_ROUND"], 1)
        self.assertEqual(derived["superseded_rounds"][0]["superseded_by"], "vulcan_review")

    def test_a_pass_dated_before_the_failed_round_does_not_fold_it(self) -> None:
        # Vulcan P4 at 92fa6646: a late-token PASS filed earlier than the
        # REVISE round it would fold is not a re-review of that round.
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_surface_review_revision_5": "REVISE_0_P0_0_P1_1_P2",
                    "vulcan_surface_review_revision_5_note": "REVISE via claude-code 2026-09-18 on 178873d7",
                    "vulcan_final_review": "PASS",
                    "vulcan_final_review_note": "PASS via muse 2026-09-13 on a810943",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(derived["states"].get("HISTORICAL_ROUND", 0), 0)
        self.assertEqual(derived["states"]["PENDING"], 1)
        # The same rounds with the PASS dated later fold as before.
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_surface_review_revision_5": "REVISE_0_P0_0_P1_1_P2",
                    "vulcan_surface_review_revision_5_note": "REVISE via claude-code 2026-09-18 on 178873d7",
                    "vulcan_final_review": "PASS",
                    "vulcan_final_review_note": "PASS via claude-code 2026-09-19 on c04539b9",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["states"]["HISTORICAL_ROUND"], 1)
        self.assertEqual(register.round_date("x 2026-09-01 y 2026-09-18 z"), "2026-09-18")
        self.assertIsNone(register.round_date(None))

    def test_an_unsuperseded_failure_blocks_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "FAIL_1_P0",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(len(derived["open_reviews"]), 1)

    def test_a_slice_pass_does_not_close_the_overall_failure(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "FAIL_1_P0",
                    "candidate_result_vulcan_review": "PASS_MANIFEST_LENGTH_FINDINGS_CLOSED",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(
            [entry["field"] for entry in derived["open_reviews"]], ["vulcan_review"]
        )

    def test_an_unclassified_disposition_blocks_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "merlin_review": "TIMED_OUT_PARTIAL_HANDOFF",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_missing_or_vacuous_counter_fails_closed(self) -> None:
        for declared in (
            None,
            {"p0": 0},
            {"p0": "0", "p1": 0, "p2": 0, "p3": 0, "p4": 0},
        ):
            summary: dict = {
                "example_package": {
                    "status": "S20_999_COMPLETE",
                    "vulcan_review": "PASS",
                },
            }
            if declared is not None:
                summary["open_findings"] = declared
            error = self.build_fails(summary)
            self.assertEqual(error.code, register.RegisterErrorCode.SUMMARY_INVALID)

    def test_a_nonzero_package_claim_blocks_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS",
                    "p0_open": [],
                    "p0_open_count": 1,
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_an_unclaimed_carried_finding_blocks_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_WITH_P1_P3_FOLLOWUPS_NO_P0_P2",
                    "p1_open": [],
                    "p1_open_count": 0,
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(
            [
                (entry["field"], entry["unclaimed_severities"])
                for entry in derived["unclaimed_carried_findings"]
            ],
            [("vulcan_review", ["P1", "P3"])],
        )

    def test_a_claimed_carried_finding_does_not_block_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_WITH_P1_FOLLOWUPS_NO_P0_P2_P3_P4",
                    "p1_open": ["finding-1"],
                    "p1_open_count": 1,
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        # The section tracks the carried P1, so nothing is unclaimed; the
        # nonzero package claim itself still blocks clearance elsewhere.
        self.assertEqual(derived["unclaimed_carried_findings"], [])
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_closed_carried_finding_does_not_block_clearance(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_PRIOR_P1_CLOSED_NO_NEW_P0_P1",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(derived["unclaimed_carried_findings"], [])
        self.assertEqual(derived["result"], "FINDING_REGISTER_CLEAR")

    def test_a_partial_closure_leaves_followups_unclaimed(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": (
                        "PASS_PRIOR_P2_CLOSED_NO_OPEN_P0_P1_P2"
                        "_WITH_P3_P4_FOLLOWUPS"
                    ),
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(
            [
                (entry["field"], entry["unclaimed_severities"])
                for entry in derived["unclaimed_carried_findings"]
            ],
            [("vulcan_review", ["P3", "P4"])],
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_stacked_negation_declares_nothing(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_NO_NO_OPEN_P1",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        # The stacked negation is void: P1 stays a visible mention and,
        # unclaimed, blocks clearance.
        self.assertEqual(
            derived["open_reviews"], [],
        )
        self.assertEqual(
            [
                (entry["field"], entry["unclaimed_severities"])
                for entry in derived["unclaimed_carried_findings"]
            ],
            [("vulcan_review", ["P1"])],
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_closed_substring_smuggles_nothing(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                    "vulcan_review": "PASS_WITH_P1_DISCLOSED",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(
            [
                (entry["field"], entry["unclaimed_severities"])
                for entry in derived["unclaimed_carried_findings"]
            ],
            [("vulcan_review", ["P1"])],
        )
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")

    def test_a_closed_suffix_salad_exempts_nothing(self) -> None:
        for disposition in (
            "PASS_WITH_P1_CLOSED_LOOP",
            "PASS_WITH_P1_CLOSEDNESS",
            "PASS_WITH_P1_CLOSED_CIRCUIT",
        ):
            derived = self.build_from(
                {
                    "example_package": {
                        "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                        "vulcan_review": disposition,
                    },
                    "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
                }
            )
            self.assertEqual(
                [
                    (entry["field"], entry["unclaimed_severities"])
                    for entry in derived["unclaimed_carried_findings"]
                ],
                [("vulcan_review", ["P1"])],
                disposition,
            )
            self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN", disposition)

    def test_a_vacuous_continuation_exempts_nothing(self) -> None:
        for disposition in (
            "PASS_WITH_P1_CLOSED_NO",
            "PASS_WITH_P1_CLOSED_WITH",
        ):
            derived = self.build_from(
                {
                    "example_package": {
                        "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                        "vulcan_review": disposition,
                    },
                    "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
                }
            )
            self.assertEqual(
                [
                    (entry["field"], entry["unclaimed_severities"])
                    for entry in derived["unclaimed_carried_findings"]
                ],
                [("vulcan_review", ["P1"])],
                disposition,
            )
            self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN", disposition)

    def test_mid_string_complete_packages_are_named_with_open_counts(self) -> None:
        derived = self.build_from(
            {
                "example_package": {
                    "status": "COMPLETE_RESTRICTED_EXAMPLE_BOUNDARY",
                    "vulcan_review": "FAIL_1_P0",
                },
                "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
            }
        )
        self.assertEqual(
            derived["mid_string_complete_packages"],
            [
                {
                    "section": "example_package",
                    "status": "COMPLETE_RESTRICTED_EXAMPLE_BOUNDARY",
                    "open_obligations": 1,
                }
            ],
        )
        # Visibility only: the open review itself blocks clearance, and a
        # mid-string status is never a completion violation.
        self.assertEqual(derived["result"], "FINDING_REGISTER_OPEN")
        self.assertEqual(derived["complete_packages_with_open_reviews"], [])

    def test_a_summary_without_obligations_fails_closed(self) -> None:
        error = self.build_fails({"nothing": {"status": "OPEN"}})
        self.assertEqual(error.code, register.RegisterErrorCode.SUMMARY_INVALID)

    def test_a_missing_summary_fails_closed(self) -> None:
        original = register.SUMMARY
        register.SUMMARY = Path("/nonexistent/summary.json")
        self.addCleanup(setattr, register, "SUMMARY", original)
        with self.assertRaises(register.RegisterError) as error:
            register.build_register()
        self.assertEqual(error.exception.code, register.RegisterErrorCode.SUMMARY_MISSING)


if __name__ == "__main__":
    unittest.main()
