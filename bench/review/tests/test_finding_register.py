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
    def test_states_are_exact(self) -> None:
        for disposition, state in (
            ("PASS", "PASS"),
            ("PASS_NO_OPEN_P0_P1_P2", "PASS"),
            ("VULCAN_PASS", "PASS"),
            ("PENDING", "PENDING"),
            ("DEFERRED_FORGE_OAUTH_401", "DEFERRED"),
            ("FAIL_2_P1_1_P2", "HISTORICAL_ROUND"),
            ("REVISE_TO_RESTRICTED_PROFILE", "HISTORICAL_ROUND"),
            ("TIMED_OUT_PARTIAL_HANDOFF", "OTHER"),
        ):
            self.assertEqual(register.classify(disposition), state, disposition)

    def test_roles_sessions_and_instants_are_not_obligations(self) -> None:
        self.assertFalse(register.is_obligation("reviewer_role", "vulcan"))
        self.assertFalse(register.is_obligation("review_session_id", "forge-vulcan-s20-540-x"))
        self.assertFalse(register.is_obligation("vulcan_review", "2026-09-02T17:42:11.513Z"))
        self.assertFalse(register.is_obligation("merge_engine_reviews", "S20-520 reviews pending"))
        self.assertFalse(register.is_obligation("status", "PENDING"))
        self.assertTrue(register.is_obligation("vulcan_review", "PENDING"))
        self.assertTrue(register.is_obligation("final_argus_disposition", "DEFERRED_FORGE_OAUTH_401"))


class DerivationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.register = register.build_register()

    def test_the_register_describes_the_tracked_summary(self) -> None:
        self.assertEqual(self.register["contract"], register.CONTRACT)
        self.assertEqual(self.register["work_package"], "S20-740")
        self.assertEqual(self.register["obligation_count"], len(self.register["obligations"]))
        self.assertEqual(
            sum(self.register["states"].values()), self.register["obligation_count"]
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

    def test_no_completed_package_carries_an_open_review(self) -> None:
        self.assertEqual(self.register["complete_packages_with_open_reviews"], [])
        self.assertTrue(self.register["complete_packages"])

    def test_the_result_reflects_the_open_reviews(self) -> None:
        expected = (
            "FINDING_REGISTER_CLEAR"
            if not self.register["open_reviews"]
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
    def test_a_completed_package_with_a_pending_review_fails_closed(self) -> None:
        original = register.SUMMARY
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "summary.json"
            path.write_text(
                json.dumps(
                    {
                        "example_package": {
                            "status": "S20_999_COMPLETE",
                            "vulcan_review": "PENDING",
                        }
                    }
                ),
                encoding="utf-8",
            )
            register.SUMMARY = path
            self.addCleanup(setattr, register, "SUMMARY", original)
            with self.assertRaises(register.RegisterError) as error:
                register.build_register()
            self.assertEqual(
                error.exception.code, register.RegisterErrorCode.COMPLETION_VIOLATION
            )

    def test_a_completed_package_may_carry_a_historical_failed_round(self) -> None:
        original = register.SUMMARY
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "summary.json"
            path.write_text(
                json.dumps(
                    {
                        "example_package": {
                            "status": "S20_999_COMPLETE",
                            "vulcan_first_review": "FAIL_2_P1",
                            "vulcan_review": "PASS",
                        },
                        "open_findings": {"p0": 0, "p1": 0, "p2": 0, "p3": 0, "p4": 0},
                    }
                ),
                encoding="utf-8",
            )
            register.SUMMARY = path
            self.addCleanup(setattr, register, "SUMMARY", original)
            derived = register.build_register()
            self.assertEqual(derived["result"], "FINDING_REGISTER_CLEAR")
            self.assertEqual(derived["states"]["HISTORICAL_ROUND"], 1)

    def test_a_summary_without_obligations_fails_closed(self) -> None:
        original = register.SUMMARY
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "summary.json"
            path.write_text(json.dumps({"nothing": {"status": "OPEN"}}), encoding="utf-8")
            register.SUMMARY = path
            self.addCleanup(setattr, register, "SUMMARY", original)
            with self.assertRaises(register.RegisterError) as error:
                register.build_register()
            self.assertEqual(error.exception.code, register.RegisterErrorCode.SUMMARY_INVALID)

    def test_a_missing_summary_fails_closed(self) -> None:
        original = register.SUMMARY
        register.SUMMARY = Path("/nonexistent/summary.json")
        self.addCleanup(setattr, register, "SUMMARY", original)
        with self.assertRaises(register.RegisterError) as error:
            register.build_register()
        self.assertEqual(error.exception.code, register.RegisterErrorCode.SUMMARY_MISSING)


if __name__ == "__main__":
    unittest.main()
