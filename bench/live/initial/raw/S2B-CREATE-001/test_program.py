"""Acceptance tests supplied with the blank raw CREATE task."""

import unittest

import program
from program import ArithmeticOverflowError, LineItem, subtotal, total


class CreateTest(unittest.TestCase):
    def test_empty(self):
        self.assertEqual(total([], 0), {"ok": True, "cents": 0})

    def test_one_line(self):
        self.assertEqual(
            total([LineItem(2, 1250)], 725),
            {"ok": True, "cents": 2681},
        )

    def test_overflow(self):
        self.assertEqual(
            total([LineItem(program.I64_MAX, 2)], 0),
            {"ok": False, "code": "ARITHMETIC_OVERFLOW"},
        )

    def test_overflow_raises_with_code(self):
        with self.assertRaises(ArithmeticOverflowError) as caught:
            subtotal([LineItem(program.I64_MAX, 2)])
        self.assertEqual(caught.exception.code, "ARITHMETIC_OVERFLOW")

    def test_overflow_add(self):
        with self.assertRaises(ArithmeticOverflowError):
            program.checked_add(program.I64_MAX, 1)


if __name__ == "__main__":
    unittest.main()
