#!/usr/bin/env python3
"""Revert cases for the S20-620 checker's composed-pin anchor (revision 7).

A stale SMP1 or S20-300 pin in the trial-runner contract is refused
against those documents' own Status lines. Inputs are served from memory;
nothing here mutates the repository.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import subprocess
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
        stale = SPEC.replace("`docs/spec/SMP1.md`\n  revision 16", "`docs/spec/SMP1.md`\n  revision 13", 1)
        self.assertNotEqual(stale, SPEC)
        self.assertTrue(any(p.startswith("smp1-pin") for p in
                            CHECKER.composed_pin_problems(stale, SMP1, SNAPSHOT)))

    def test_a_stale_snapshot_pin_is_refused(self):
        stale = SPEC.replace("section 5, revision 6)", "section 5, revision 4)", 1)
        self.assertNotEqual(stale, SPEC)
        self.assertTrue(any(p.startswith("s20-300-pin") for p in
                            CHECKER.composed_pin_problems(stale, SMP1, SNAPSHOT)))

    def test_an_authority_moving_leaves_the_contract_red(self):
        moved = SMP1.replace("contract draft, revision 16", "contract draft, revision 17", 1)
        self.assertTrue(any(p.startswith("smp1-pin") for p in
                            CHECKER.composed_pin_problems(SPEC, moved, SNAPSHOT)))



OLD_SCOPE = "2b0f1c9f4940c020565137891a49a3769cd02061"  # contract revision 6
R7_SCOPE = "26d050e629b669ef4acbd54826ef4008c05a061d"   # contract revision 7
LANES = ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review")
SUMMARY_TEXT = (ROOT / "machineresearch/sley-2.0/machine-summary.json").read_text(encoding="utf-8")


def note(scope: str) -> str:
    return f"Lane verdict dated 2026-09-23 on {scope}; transcript x"


def run_with_summary(summary: dict) -> tuple[int, dict]:
    original = CHECKER.read

    def read(path):
        if Path(path) == CHECKER.SUMMARY:
            return json.dumps(summary)
        return original(path)

    CHECKER.read = read
    try:
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = CHECKER.main()
        return code, json.loads(out.getvalue())
    finally:
        CHECKER.read = original


def complete_flip(fields: dict) -> dict:
    summary = json.loads(SUMMARY_TEXT)
    section = summary["sley2_trial_runner"]
    section["status"] = CHECKER.COMPLETE_STATUS
    section["implementation_complete"] = True
    section.update(fields)
    return summary


class CompletionScopeBinding(unittest.TestCase):
    """Round-8 P2 at 26d050e: a `_revision_N` PASS must have reviewed revision N."""

    def test_the_reviewers_probe_a_revision_6_verdict_under_the_revision_7_name(self):
        # The 2b0f1c9 delta verdict sat in vulcan_surface_review_revision_7
        # and satisfied the name-only gate; bound by scope it is refused.
        probe = {"vulcan_surface_review_revision_7": "PASS_0_P0_0_P1_0_P2_2_P3_3_P4",
                 "vulcan_surface_review_revision_7_note": note(OLD_SCOPE)}
        self.assertEqual(
            CHECKER.scope_bound_problems(probe, "vulcan_surface_review", 7,
                                         "docs/spec/SLEY2_TRIAL_RUNNER_V1.md",
                                         CHECKER.STATUS_PATTERN, ROOT),
            ["completion-scope-mismatch:vulcan_surface_review:reviewed-revision-6"])
        probe["vulcan_surface_review_revision_7_note"] = note(R7_SCOPE)
        self.assertEqual(
            CHECKER.scope_bound_problems(probe, "vulcan_surface_review", 7,
                                         "docs/spec/SLEY2_TRIAL_RUNNER_V1.md",
                                         CHECKER.STATUS_PATTERN, ROOT), [])

    def test_a_complete_flip_on_verdicts_scoped_to_an_older_text_fails(self):
        revision = json.loads(SUMMARY_TEXT)["sley2_trial_runner"]["contract_revision"]
        fields = {}
        for lane in LANES:
            fields[f"{lane}_revision_{revision}"] = "PASS_0_P0_0_P1_0_P2_0_P3"
            fields[f"{lane}_revision_{revision}_note"] = note(R7_SCOPE)
        code, payload = run_with_summary(complete_flip(fields))
        self.assertEqual(code, 1)
        for lane in LANES:
            self.assertIn(f"completion-scope-mismatch:{lane}:reviewed-revision-7", payload["problems"])

    def test_unscoped_and_unknown_commit_notes_fail_closed(self):
        revision = json.loads(SUMMARY_TEXT)["sley2_trial_runner"]["contract_revision"]
        fields = {}
        for lane in LANES:
            fields[f"{lane}_revision_{revision}"] = "PASS_0_P0_0_P1_0_P2_0_P3"
        fields[f"ariadne_contract_review_revision_{revision}_note"] = "no scope here"
        fields[f"nabu_architecture_review_revision_{revision}_note"] = note("f" * 40)
        fields[f"vulcan_surface_review_revision_{revision}_note"] = note(R7_SCOPE)
        code, payload = run_with_summary(complete_flip(fields))
        self.assertEqual(code, 1)
        self.assertIn("completion-unscoped-review:ariadne_contract_review", payload["problems"])
        self.assertTrue(any(p.startswith("completion-unknown-commit:nabu_architecture_review")
                            for p in payload["problems"]), payload["problems"])


class HistoryCutScope(unittest.TestCase):
    """A public commit is read live; an archived one only through its frozen Status line."""

    def bound(self, scope: str, revision: int) -> list[str]:
        probe = {f"vulcan_surface_review_revision_{revision}": "PASS_0_P0_0_P1_0_P2_0_P3",
                 f"vulcan_surface_review_revision_{revision}_note": note(scope)}
        return CHECKER.scope_bound_problems(probe, "vulcan_surface_review", revision,
                                            "docs/spec/SLEY2_TRIAL_RUNNER_V1.md",
                                            CHECKER.STATUS_PATTERN, ROOT)

    def test_an_archived_commit_without_a_frozen_status_fails_closed(self):
        # The archived tip is in the ledger but no review note names it.
        tip = CHECKER.completion_scope.history_ledger.archived_tip()
        self.assertTrue(CHECKER.completion_scope.history_ledger.is_archived(tip))
        self.assertEqual(self.bound(tip, 8), [f"completion-unknown-commit:vulcan_surface_review:{tip[:12]}"])

    def test_a_public_commit_is_read_live(self):
        shallow = subprocess.run(["git", "rev-parse", "--is-shallow-repository"], cwd=ROOT,
                                 capture_output=True, text=True, check=True).stdout.strip()
        if shallow == "true":
            self.skipTest("a shallow clone's boundary commit is not the public root")
        root = subprocess.run(["git", "rev-list", "--max-parents=0", "HEAD"], cwd=ROOT,
                              capture_output=True, text=True, check=True).stdout.split()[0]
        self.assertEqual(self.bound(root, 8), [])
        self.assertEqual(self.bound(root, 9),
                         ["completion-scope-mismatch:vulcan_surface_review:reviewed-revision-8"])


if __name__ == "__main__":
    unittest.main()
