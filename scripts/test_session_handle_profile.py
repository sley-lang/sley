#!/usr/bin/env python3
"""Discriminating static tests for the S20-330 session checker.

Each mutation case serves one input from memory while every other file
remains the real pinned input, runs the real checker entrypoint, captures
its real JSON/return code, and requires a specific refusal. The pinned
checker currently misses these drifts (A-ST-01, A-ST-02, N-STATIC-01), so
the mutation cases fail before source repair and pass after it. The valid
control passes before and after. Production Rust stays byte-identical;
mutations exist only in test memory.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def load_module(name: str, relative: str):
    path = ROOT / relative
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


CHECKER = load_module(
    "session_handle_profile_checker", "scripts/check_session_handle_profile.py"
)

REGISTRY_TEXT = (ROOT / "crates/sley-protocol/src/lib.rs").read_text(encoding="utf-8")
SERVER_TEXT = (ROOT / "crates/sley-protocol/src/server.rs").read_text(encoding="utf-8")
SUMMARY_TEXT = (ROOT / "machineresearch/sley-2.0/machine-summary.json").read_text(
    encoding="utf-8"
)


def run_checker_with_overrides(
    registry_text: str | None = None,
    server_text: str | None = None,
    summary_text: str | None = None,
) -> tuple[int, dict]:
    """Run the real checker with selected reads served from memory."""
    original_read = CHECKER.read

    def read(path):
        resolved = Path(path)
        if registry_text is not None and resolved == CHECKER.REGISTRY_MODULE:
            return registry_text
        if server_text is not None and resolved == CHECKER.SERVER_MODULE:
            return server_text
        if summary_text is not None and resolved == CHECKER.SUMMARY:
            return summary_text
        return original_read(path)

    CHECKER.read = read
    try:
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            code = CHECKER.main()
        return code, json.loads(buffer.getvalue())
    finally:
        CHECKER.read = original_read


class ValidPinControl(unittest.TestCase):
    def test_valid_pin_accepted(self):
        code, payload = run_checker_with_overrides()
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


class VariantPairCases(unittest.TestCase):
    """A-ST-01: the exact variant-to-tag map, not just the value set."""

    def test_swapped_entity_tag_arms_refused(self):
        probe = "Self::EntityVersion => ENTITY_VERSION_TAG,"
        self.assertIn(probe, REGISTRY_TEXT)
        self.assertIn("pub const ENTITY_VERSION_TAG: u32 = 306;", REGISTRY_TEXT)
        self.assertIn("pub const ENTITY_SIGNATURE_TAG: u32 = 307;", REGISTRY_TEXT)
        placeholder = "ENTITY_VERSION_TAG_SWAP_PROBE"
        staged = REGISTRY_TEXT.replace(
            "Self::EntityVersion => ENTITY_VERSION_TAG,",
            f"Self::EntityVersion => {placeholder},",
            1,
        )
        staged = staged.replace(
            "Self::EntitySignature => ENTITY_SIGNATURE_TAG,",
            "Self::EntitySignature => ENTITY_VERSION_TAG,",
            1,
        )
        mutated = staged.replace(
            f"Self::EntityVersion => {placeholder},",
            "Self::EntityVersion => ENTITY_SIGNATURE_TAG,",
            1,
        )
        self.assertNotEqual(mutated, REGISTRY_TEXT)
        self.assertIn("Self::EntityVersion => ENTITY_SIGNATURE_TAG,", mutated)
        self.assertIn("Self::EntitySignature => ENTITY_VERSION_TAG,", mutated)
        code, payload = run_checker_with_overrides(registry_text=mutated)
        self.assertEqual(payload.get("result"), "FAIL", "checker must refuse the drift")
        self.assertNotEqual(code, 0, "checker must exit nonzero on the drift")
        problems = payload.get("problems", [])
        self.assertTrue(
            any("EntityVersion" in problem or "EntitySignature" in problem for problem in problems),
            f"refusal must name the swapped variant, got: {problems}",
        )


class DelegationOperatorCases(unittest.TestCase):
    """A-ST-02: the delegated union behavior, not mere token presence."""

    def test_conjunction_instead_of_disjunction_refused(self):
        anchor = "const fn head_bound_versioned(method: Method) -> bool {"
        self.assertIn(anchor, SERVER_TEXT)
        index = SERVER_TEXT.index(anchor)
        window = SERVER_TEXT[index : index + 400]
        self.assertIn("Self::head_bound(method)", window)
        self.assertIn("|| matches!(", window)
        self.assertIn("Method::EntityVersion | Method::EntitySignature", window)
        mutated = (
            SERVER_TEXT[:index]
            + window.replace("|| matches!(", "&& matches!(", 1)
            + SERVER_TEXT[index + 400 :]
        )
        self.assertNotEqual(mutated, SERVER_TEXT)
        self.assertIn("Self::head_bound(method)", mutated)
        self.assertIn("Method::EntityVersion | Method::EntitySignature", mutated)
        code, payload = run_checker_with_overrides(server_text=mutated)
        self.assertEqual(payload.get("result"), "FAIL", "checker must refuse the drift")
        self.assertNotEqual(code, 0, "checker must exit nonzero on the drift")
        problems = payload.get("problems", [])
        self.assertTrue(
            any("versioned" in problem for problem in problems),
            f"refusal must come from the versioned-delegation guard, got: {problems}",
        )


class FrozenReviewCases(unittest.TestCase):
    """N-STATIC-01: freeze needs a current-revision review, not history."""

    def test_frozen_without_current_review_refused(self):
        summary = json.loads(SUMMARY_TEXT)
        section = summary["session_handle_profile"]
        self.assertEqual(section["contract_revision"], CHECKER.CONTRACT_REVISION)
        # The live record may already carry the current-revision PASSes; the
        # negative control is a freeze with the current review still open.
        for lane in ("ariadne", "nabu", "vulcan"):
            section["current_delta_review"][lane] = "PENDING"
        section["status"] = CHECKER.FROZEN_STATUS
        mutated_summary = json.dumps(summary)
        code, payload = run_checker_with_overrides(summary_text=mutated_summary)
        self.assertEqual(payload.get("result"), "FAIL", "checker must refuse the drift")
        self.assertNotEqual(code, 0, "checker must exit nonzero on the drift")
        problems = payload.get("problems", [])
        self.assertTrue(
            any("review" in problem for problem in problems),
            f"refusal must come from the current-revision review guard, got: {problems}",
        )

    def test_mismatched_current_revision_refused(self):
        summary = json.loads(SUMMARY_TEXT)
        review = summary["session_handle_profile"]["current_delta_review"]
        self.assertEqual(review["contract_revision"], CHECKER.CONTRACT_REVISION)
        review["contract_revision"] = CHECKER.CONTRACT_REVISION - 1
        code, payload = run_checker_with_overrides(
            summary_text=json.dumps(summary)
        )
        self.assertEqual(payload.get("result"), "FAIL", "checker must refuse the drift")
        self.assertNotEqual(code, 0, "checker must exit nonzero on the drift")
        problems = payload.get("problems", [])
        self.assertTrue(
            any("review" in problem for problem in problems),
            f"refusal must come from the current-revision review guard, got: {problems}",
        )

    def test_bound_all_pass_review_accepted(self):
        summary = json.loads(SUMMARY_TEXT)
        review = summary["session_handle_profile"]["current_delta_review"]
        self.assertEqual(review["contract_revision"], CHECKER.CONTRACT_REVISION)
        for lane in ("ariadne", "nabu", "vulcan"):
            review[lane] = "PASS"
        code, payload = run_checker_with_overrides(
            summary_text=json.dumps(summary)
        )
        self.assertEqual(code, 0)
        self.assertEqual(payload.get("result"), "PASS")


if __name__ == "__main__":
    unittest.main()
