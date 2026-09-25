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

    def test_numbered_rounds_need_positive_chronology(self) -> None:
        # Merge of work/succession-sley20-arm (FINDING_REGISTER_V1 revision
        # 11): a numbered-round fold fails closed without two scopes, the
        # PASS's a strict descendant of the round's.
        earlier = "69907ddf904a7d3df48558425273cc58179a7077"
        later = "8966da2e0a52a6f6e56949e9a5902464c86613c0"
        rounds = ("vulcan_surface_review_revision_10", "vulcan_surface_review_revision_9")
        self.assertTrue(builder.numbered_round_chronology(*rounds, later, earlier))
        self.assertFalse(builder.numbered_round_chronology(*rounds, earlier, later))
        self.assertFalse(builder.numbered_round_chronology(*rounds, later, later))
        self.assertFalse(builder.numbered_round_chronology(*rounds, None, earlier))
        self.assertFalse(builder.numbered_round_chronology(*rounds, later, None))
        self.assertFalse(builder.numbered_round_chronology(*rounds, "f" * 40, earlier))
        # Every other fold keeps its existing guards.
        self.assertTrue(builder.numbered_round_chronology(
            "vulcan_surface_review", "vulcan_surface_review_revision_9", None, None))

    def test_an_unscoped_numbered_pass_leaves_the_round_pending(self) -> None:
        def summary(scoped: bool) -> dict:
            note = "via x 2026-09-23 on {}: transcript t"
            return {"sec": {
                "status": "S20_999_IMPLEMENTED_REVIEW_PENDING",
                "vulcan_surface_review_revision_9": "REVISE_0_P0_0_P1_1_P2",
                "vulcan_surface_review_revision_9_note": note.format("69907ddf904a7d3df48558425273cc58179a7077") if scoped else "via x 2026-09-23",
                "vulcan_surface_review_revision_10": "PASS",
                "vulcan_surface_review_revision_10_note": note.format("8966da2e0a52a6f6e56949e9a5902464c86613c0") if scoped else "via x 2026-09-23",
            }}
        def state(scoped: bool) -> str:
            rows = builder.collect(summary(scoped))
            return next(row["state"] for row in rows if row["field"] == "vulcan_surface_review_revision_9")
        self.assertEqual(state(True), "HISTORICAL_ROUND")
        self.assertEqual(state(False), "PENDING")

    def test_multi_item_findings_lines_bind_each_items_severity(self) -> None:
        lines = ["FINDINGS: [P3] [spec-code] a.md:1 - first | [P4] [record] b.py:2 - second item; [P2] [gate] c.rs:3 - third"]
        self.assertEqual(builder._finding_line_match(lines, "[spec-code] a.md:1 - first"), "P3")
        self.assertEqual(builder._finding_line_match(lines, "[record] b.py:2 - second item"), "P4")
        self.assertEqual(builder._finding_line_match(lines, "[gate] c.rs:3 - third"), "P2")
        self.assertIsNone(builder._finding_line_match(lines, "[gate] c.rs:3 - another"))

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
