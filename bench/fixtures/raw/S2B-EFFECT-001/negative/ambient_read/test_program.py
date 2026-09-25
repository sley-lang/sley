"""Unit tests for S2B-EFFECT-001 (stdlib unittest)."""

import unittest

import program
from program import FileRead, load_config, get_invoice_total, sum_lines


class EffectTest(unittest.TestCase):
    def test_function_declares_fileread(self):
        self.assertIn("FileRead", load_config.EFFECTS)

    def test_caller_closure_includes_fileread(self):
        self.assertIn("FileRead", get_invoice_total.EFFECTS)

    def test_pure_sibling_remains_pure(self):
        self.assertEqual(sum_lines.EFFECTS, frozenset())
        self.assertEqual(sum_lines([(2, 1250)]), 2500)

    def test_replayed_output_unchanged(self):
        self.assertEqual(program.replay_digest(),
                         program.EXPECTED_REPLAY_DIGEST)

    def test_effect_actually_used_not_ambient(self):
        eff = FileRead()
        out = get_invoice_total(eff)
        self.assertEqual(out["total_cents"], 2681)
        self.assertGreaterEqual(
            len(eff.log), 2,
            "declared effect bypassed: log=%r ambient_reads=%d"
            % (eff.log, program.ambient_read_count()))
        self.assertEqual(program.ambient_read_count(), 0)


if __name__ == "__main__":
    unittest.main()
