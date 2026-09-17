"""Unit tests for S2B-REPAIR-001 (stdlib unittest). Mirrors corpus triples."""

import unittest

import program
from program import clamp


class RepairTest(unittest.TestCase):
    def test_below_low(self):
        self.assertEqual(clamp(-1, 0, 10), 0)

    def test_within(self):
        self.assertEqual(clamp(7, 0, 10), 7)

    def test_above_high(self):
        self.assertEqual(clamp(11, 0, 10), 10)

    def test_signature_preserved(self):
        self.assertEqual(program.signature_names(), ("x", "low", "high"))
        self.assertFalse(program.signature_has_defaults())


if __name__ == "__main__":
    unittest.main()
