#!/usr/bin/env python3
"""Regression controls for the S20-250 checker's lock-closure walk.

The transitive direction check once passed vacuously when the root package
was absent from Cargo.lock (Ariadne P4 note N2 at 0bcc9c6): an empty closure
reaches nothing forbidden. The walk now refuses an absent root, and the
checker turns that refusal into a named problem. Nothing here mutates the
repository under test.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "check_complete_entity_impact_profile",
    ROOT / "scripts" / "check_complete_entity_impact_profile.py",
)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)


class LockReachableTests(unittest.TestCase):
    def test_absent_root_is_refused_not_empty(self) -> None:
        with mock.patch.object(checker, "lock_packages", return_value={"a": ["b"], "b": []}):
            with self.assertRaises(KeyError) as caught:
                checker.lock_reachable("sley-query")
        self.assertEqual(caught.exception.args[0], "lock-root-absent:sley-query")

    def test_present_root_walks_transitively(self) -> None:
        packages = {"sley-query": ["x"], "x": ["y"], "y": []}
        with mock.patch.object(checker, "lock_packages", return_value=packages):
            self.assertEqual(checker.lock_reachable("sley-query"), {"x", "y"})

    def test_real_lock_names_the_root(self) -> None:
        self.assertIn("sley-query", checker.lock_packages())
        self.assertFalse(
            checker.lock_reachable("sley-query") & {"sley-store", "sley-mutate", "sley-policy"}
        )


if __name__ == "__main__":
    unittest.main()
