"""The claim-retirement tool: automatic and explicit rules, replay, folding."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import build_finding_register as register  # noqa: E402
import retire_review_claims as retire  # noqa: E402


class RetireReviewClaimsTests(unittest.TestCase):
    """A fixture repository with two rounds of one lane's transcripts."""

    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        (self.root / "x").write_text("a\n")
        self.commit("first")
        self.first = self.git("rev-parse", "HEAD").strip()
        (self.root / "x").write_text("b\n")
        self.commit("second")
        self.second = self.git("rev-parse", "HEAD").strip()
        self.section = self.root / "evidence/review/verdicts/release_candidate_packaging"
        self.section.mkdir(parents=True)
        (self.section / f"vulcan_surface_review-{self.first[:7]}.md").write_text(
            "FINDINGS: [P3] [record] scripts/a.py:1 - stale note\nVERDICT: PASS_0_P0_0_P1_0_P2_1_P3\n"
        )
        (self.section / f"vulcan_surface_review-{self.second[:7]}.md").write_text(
            "- **[P3] stale note in scripts/a.py — CLOSED.** repaired\n"
            "VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED\n"
        )
        for module in (register, retire):
            self.addCleanup(setattr, module, "ROOT", module.ROOT)
            module.ROOT = self.root
        retire._ancestry.clear()
        retire._resolved.clear()
        register._scope_order.clear()

    def git(self, *args: str) -> str:
        return subprocess.check_output(
            ["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", *args],
            cwd=self.root, text=True,
        )

    def commit(self, message: str) -> None:
        self.git("add", ".")
        self.git("commit", "-qm", message)

    def summary(self, claim: str) -> dict:
        return {
            "release_candidate_packaging": {
                "vulcan_surface_review": "PASS_0_P0_0_P1_0_P2_0_P3_PRIOR_P3_CLOSED",
                "vulcan_surface_review_note": f"on {self.second}: transcript evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-{self.second[:7]}.md",
                "p3_open": [claim],
                "p3_open_count": 1,
            }
        }

    def test_the_automatic_rule_retires_a_strictly_earlier_claim_only(self) -> None:
        summary = self.summary(f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - stale note")
        self.assertEqual(retire.retire(summary, []), 1)
        closed = summary["release_candidate_packaging"]["p3_closed_claims"][0]
        self.assertIn(f"vulcan_surface_review-{self.second[:7]}.md#L1", closed["verified_by"])
        self.assertEqual(summary["release_candidate_packaging"]["p3_open"], [])
        # The claim's own round, and an untagged claim, never retire automatically.
        for claim in (
            f"vulcan_surface_review@{self.second[:7]}: [record] scripts/a.py:1 - stale note",
            "vulcan_surface_review: [record] scripts/a.py:1 - stale note",
        ):
            summary = self.summary(claim)
            self.assertEqual(retire.retire(summary, []), 0, claim)
        # A closure line that does not speak about the claim retires nothing.
        summary = self.summary(f"vulcan_surface_review@{self.first[:7]}: [wording] docs/other.md:9 - unrelated")
        self.assertEqual(retire.retire(summary, []), 0)

    def test_transcript_closers_come_from_filed_verdict_lines(self) -> None:
        closers = retire.transcript_closers("release_candidate_packaging")
        self.assertEqual(len(closers), 1)
        _, lane, severities, scope, path = closers[0]
        self.assertEqual((lane, severities, scope), ("vulcan", {"P3"}, self.second))
        self.assertTrue(path.endswith(f"vulcan_surface_review-{self.second[:7]}.md"))

    def test_the_explicit_rule_needs_speaking_closure_lines_and_a_field_prefix(self) -> None:
        claim = "vulcan_surface_review: [record] scripts/a.py:1 - stale note"
        path = f"evidence/review/verdicts/release_candidate_packaging/vulcan_surface_review-{self.second[:7]}.md"
        entry = {"section": "release_candidate_packaging", "severities": [3], "prefixes": ["vulcan_surface_review: [record]"],
                 "verified_by": path, "reason": "closed at the second round"}
        summary = self.summary(claim)
        self.assertEqual(retire.retire(summary, [entry]), 1)
        self.assertIn("#L1 — closed at the second round", summary["release_candidate_packaging"]["p3_closed_claims"][0]["verified_by"])
        for bad in (
            dict(entry, lines=[2]),
            dict(entry, prefixes=["vulcan_surface_review"]),
            dict(entry, prefixes=[""]),
        ):
            with self.assertRaises(SystemExit):
                retire.retire(self.summary(claim), [bad])

    def test_replay_refuses_a_stale_or_duplicated_closure(self) -> None:
        claim = f"vulcan_surface_review@{self.first[:7]}: [record] scripts/a.py:1 - stale note"
        summary = self.summary(claim)
        retire.retire(summary, [])
        self.assertEqual(retire.replay_problems(summary), [])
        stale = json.loads(json.dumps(summary))
        stale["release_candidate_packaging"]["p3_closed_claims"][0]["verified_by"] = stale["release_candidate_packaging"]["p3_closed_claims"][0]["verified_by"].replace("#L1", "#L2")
        self.assertEqual(len(retire.replay_problems(stale)), 1)
        duplicated = json.loads(json.dumps(summary))
        duplicated["release_candidate_packaging"]["p3_open"] = [claim]
        self.assertTrue(any("open and closed" in problem for problem in retire.replay_problems(duplicated)))

    def test_transcript_resolution_fails_closed_on_ambiguity(self) -> None:
        (self.section / f"vulcan_surface_review_final-{self.second[:7]}.md").write_text("VERDICT: PASS\n")
        with self.assertRaises(SystemExit):
            retire.transcript_for("release_candidate_packaging", "vulcan", self.second, None)
        self.assertEqual(retire.role("current_delta_review.nabu"), "nabu")

    def test_restatements_fold_into_the_earliest_claim(self) -> None:
        earliest = "vulcan_surface_review: [record] scripts/a.py:1 - `note` stale"
        carried = f"vulcan_surface_review@{self.second[:7]}: [record] scripts/a.py:3 - carried OPEN: `note` still stale"
        fresh = f"vulcan_surface_review@{self.second[:7]}: [record] scripts/a.py:3 - a new `note` finding"
        other = f"vulcan_surface_review@{self.second[:7]}: [record] scripts/b.py:3 - carried OPEN: another `note`"
        summary = {"s": {"p4_open": [earliest, carried, fresh, other], "p4_open_count": 4}}
        self.assertEqual(retire.fold_restatements(summary), 1)
        self.assertEqual(summary["s"]["p4_open"], [earliest, fresh, other])
        self.assertEqual(summary["s"]["p4_restated_claims"], [{"claim": carried, "restates": earliest}])
        self.assertEqual(retire.fold_restatements(summary), 0)


if __name__ == "__main__":
    unittest.main()
