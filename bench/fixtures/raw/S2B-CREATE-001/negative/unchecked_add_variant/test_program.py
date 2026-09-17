"""Unit tests for S2B-CREATE-001 (stdlib unittest). Mirrors corpus cases."""

import unittest

import program
from program import LineItem, total, subtotal, ArithmeticOverflowError


class CreateTest(unittest.TestCase):
    def test_empty(self):
        self.assertEqual(total([], 0), {"ok": True, "cents": 0})

    def test_one_line(self):
        # subtotal 2*1250=2500; tax (2500*725)//10000=181 -> 2681
        self.assertEqual(total([LineItem(2, 1250)], 725),
                         {"ok": True, "cents": 2681})

    def test_overflow(self):
        r = total([LineItem(program.I64_MAX, 2)], 0)
        self.assertEqual(r, {"ok": False, "code": "ARITHMETIC_OVERFLOW"})

    def test_overflow_raises_with_code(self):
        with self.assertRaises(ArithmeticOverflowError) as cm:
            subtotal([LineItem(program.I64_MAX, 2)])
        self.assertEqual(cm.exception.code, "ARITHMETIC_OVERFLOW")

    def test_overflow_add(self):
        with self.assertRaises(ArithmeticOverflowError):
            program.checked_add(program.I64_MAX, 1)


if __name__ == "__main__":
    unittest.main()
