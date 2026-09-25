"""Regression tests for the invariant-audit declared-limits coupling.

Repair round 8: the old test ("this number appears somewhere in the
concatenated spec tree") let a ceiling raised to any value another
contract already states pass. These tests pin the coupled rule: a value
counts for a constant only in a file that also names the constant.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"scripts/{name}.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


limits = load("check_declared_limits")


class EvaluateTests(unittest.TestCase):
    def test_plain_and_grouped_numerals(self) -> None:
        self.assertEqual(limits.evaluate("65535"), 65535)
        self.assertEqual(limits.evaluate("1_000_000"), 1000000)

    def test_shift_expression(self) -> None:
        self.assertEqual(limits.evaluate("(1 << 53) - 1"), 9007199254740991)

    def test_type_max(self) -> None:
        self.assertEqual(limits.evaluate("u16::MAX"), 65535)
        self.assertEqual(limits.evaluate("u8::MAX"), 255)

    def test_platform_dependent_is_unevaluated(self) -> None:
        self.assertIsNone(limits.evaluate("usize::MAX"))
        self.assertIsNone(limits.evaluate("MAX_OTHER + 1"))


class CouplingTests(unittest.TestCase):
    docs = {
        "a.md": "The `MAX_A` ceiling is 65535 operations.",
        "b.md": "Unrelated contract stating 65535 and 1000000 as its own bounds.",
    }

    def test_name_plus_value_in_one_file_is_documented(self) -> None:
        verdict = limits.check_constant("MAX_A", 65535, "65535", self.docs)
        self.assertEqual(verdict["grade"], "documented")

    def test_shared_value_elsewhere_is_weak_not_documented(self) -> None:
        # The repair-round-8 defect in miniature: 1000000 appears in b.md
        # but MAX_B is named nowhere, so it must not read documented.
        verdict = limits.check_constant("MAX_B", 1000000, "1000000", self.docs)
        self.assertEqual(verdict["grade"], "weak")

    def test_absent_value_is_undocumented(self) -> None:
        verdict = limits.check_constant("MAX_C", 123456789, "123456789", self.docs)
        self.assertEqual(verdict["grade"], "undocumented")

    def test_power_idiom_counts_as_the_value(self) -> None:
        docs = {"bridge.md": "An integer of at most 2^53 - 1 is valid."}
        verdict = limits.check_constant(
            "MAX_N", 9007199254740991, "(1 << 53) - 1", docs
        )
        self.assertEqual(verdict["grade"], "weak")
        named = dict(docs, extra="The `MAX_N` bound is 2^53 - 1.")
        verdict = limits.check_constant(
            "MAX_N", 9007199254740991, "(1 << 53) - 1", named
        )
        self.assertEqual(verdict["grade"], "documented")

    def test_unevaluated_needs_a_named_constant(self) -> None:
        verdict = limits.check_constant("MAX_Q", None, "usize::MAX", self.docs)
        self.assertEqual(verdict["grade"], "undocumented")
        named = dict(self.docs, extra="The `MAX_Q` bound is platform-native.")
        verdict = limits.check_constant("MAX_Q", None, "usize::MAX", named)
        self.assertEqual(verdict["grade"], "documented")


if __name__ == "__main__":
    unittest.main()
