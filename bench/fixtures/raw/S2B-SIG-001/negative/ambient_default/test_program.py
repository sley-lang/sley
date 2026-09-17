"""Unit tests for S2B-SIG-001 (stdlib unittest)."""

import inspect
import unittest

import program


class SigTest(unittest.TestCase):
    def test_validation_valid(self):
        self.assertEqual(program.validate_migration(), "VALID")

    def test_expected_callers(self):
        self.assertEqual(len(program.CALLERS), program.EXPECTED_CALLERS)
        self.assertEqual(program.EXPECTED_CALLERS, 3)

    def test_no_ambient_default(self):
        params = list(inspect.signature(program.net_total).parameters.values())
        self.assertEqual([p.name for p in params],
                         ["lines", "tax_basis_points"])
        self.assertIs(params[1].default, inspect.Parameter.empty)

    def test_callers_pass_explicit_values(self):
        for name in program.CALLERS:
            with self.subTest(caller=name):
                self.assertTrue(program.caller_passes_literal(name),
                                name + " must pass an explicit literal")

    def test_sample_executions_agree(self):
        lines = [(2, 1250)]
        self.assertEqual(program.caller_checkout(lines),
                         program.net_total(lines, 725))
        self.assertEqual(program.caller_refund(lines),
                         program.net_total(lines, 0))
        self.assertEqual(program.caller_quote(lines),
                         program.net_total(lines, 1000))


if __name__ == "__main__":
    unittest.main()
