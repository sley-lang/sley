"""Unit tests for S2B-TEST-001 (stdlib unittest)."""

import hashlib
import unittest

import program
from program import checked_div


class DivTest(unittest.TestCase):
    def test_success(self):
        self.assertEqual(checked_div(20, 4), 5)

    def test_divide_by_zero(self):
        with self.assertRaises(ZeroDivisionError) as cm:
            checked_div(7, 0)
        self.assertIn("DIVIDE_BY_ZERO", str(cm.exception))

    def test_signed_overflow(self):
        with self.assertRaises(ArithmeticError) as cm:
            checked_div(program.I64_MIN, -1)
        self.assertEqual(cm.exception.code, "ARITHMETIC_OVERFLOW")

    def test_implementation_unchanged(self):
        self.assertEqual(
            hashlib.sha256(program.IMPL_RECORD.encode()).hexdigest(),
            program.IMPL_DIGEST)
        # Pinned root fact: any implementation change flips this.
        self.assertEqual(
            program.IMPL_DIGEST,
            "34aea94910c906c3d4d4a4049556451407862e31389194c535d5246ab25a7899")

    def test_binding_covers_required_cases(self):
        self.assertEqual(sorted(program.TEST_BINDING["cases"]),
                         sorted(program.REQUIRED_CASES))
        self.assertEqual(len(program.TEST_BINDING["cases"]), 3)


if __name__ == "__main__":
    unittest.main()
