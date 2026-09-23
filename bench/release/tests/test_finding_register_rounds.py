"""Unit tests for finding-register round semantics (S20-740 builder).

Covers field_core/supersedes/classify_token directly: a scoped PASS must
never fold a round from another subject, while the established patterns
(unmarked general PASS folding lane rounds, revision rounds folding into
the unmarked PASS) keep working.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def load_builder():
    spec = importlib.util.spec_from_file_location(
        "finding_register", ROOT / "scripts/build_finding_register.py"
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


builder = load_builder()


class SupersedeTests(unittest.TestCase):
    def test_scoped_pass_never_folds_foreign_round(self) -> None:
        self.assertFalse(
            builder.supersedes(
                "vulcan_entity_read_review", "vulcan_surface_review_revision_1"
            )
        )
        self.assertFalse(
            builder.supersedes(
                "vulcan_entity_read_review", "vulcan_surface_review"
            )
        )

    def test_scoped_pass_folds_same_subject_round(self) -> None:
        self.assertTrue(
            builder.supersedes(
                "nabu_entity_read_review", "nabu_entity_read_review_revision_1"
            )
        )

    def test_qualified_pass_folds_unmarked_early_round(self) -> None:
        self.assertTrue(
            builder.supersedes(
                "ariadne_profile_final_review", "ariadne_initial_review"
            )
        )

    def test_unmarked_pass_folds_lane_rounds(self) -> None:
        self.assertTrue(
            builder.supersedes("nabu_final_review", "nabu_architecture_review")
        )
        self.assertTrue(
            builder.supersedes(
                "ariadne_review", "ariadne_contract_review_revision_1"
            )
        )

    def test_revision_fail_folds_into_unmarked_pass(self) -> None:
        self.assertTrue(
            builder.supersedes(
                "ariadne_contract_review", "ariadne_contract_review_revision_1"
            )
        )

    def test_a_later_revision_pass_folds_an_earlier_revision_round(self) -> None:
        self.assertTrue(builder.supersedes(
            "vulcan_surface_review_revision_10", "vulcan_surface_review_revision_9"))
        self.assertTrue(builder.supersedes(
            "nabu_architecture_review_revision_8", "nabu_architecture_review_revision_7"))

    def test_an_earlier_revision_pass_never_folds_a_later_round(self) -> None:
        self.assertFalse(builder.supersedes(
            "vulcan_surface_review_revision_9", "vulcan_surface_review_revision_10"))

    def test_revision_rounds_of_another_subject_never_fold(self) -> None:
        self.assertFalse(builder.supersedes(
            "vulcan_entity_read_review_revision_10", "vulcan_surface_review_revision_9"))

    def test_same_field_never_supersedes(self) -> None:
        self.assertFalse(
            builder.supersedes("ariadne_contract_review", "ariadne_contract_review")
        )

    def test_revise_classifies_fail_round(self) -> None:
        self.assertEqual(builder.classify_token("REVISE_0_P0_1_P1_2_P2_3_P3"), "FAIL_ROUND")

    def test_pass_with_followups_classifies_pass(self) -> None:
        self.assertEqual(
            builder.classify_token("PASS_WITH_P3_P4_FOLLOWUPS_NO_P0_P1_P2"), "PASS"
        )


if __name__ == "__main__":
    unittest.main()
