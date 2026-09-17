"""Unit tests for S2B-MERGE-001 (stdlib unittest)."""

import unittest

import program


class MergeTest(unittest.TestCase):
    def test_zero_conflicts(self):
        combined, conflict = program.merge(program.BASE, program.LEFT,
                                           program.RIGHT)
        self.assertEqual(repr(conflict), "Conflict(entities=[], count=0)")
        self.assertIsNotNone(combined)

    def test_both_changes_preserved(self):
        combined, conflict = program.merge(program.BASE, program.LEFT,
                                           program.RIGHT)
        self.assertIsNotNone(combined,
                             "merge returned conflict=%r" % (conflict,))
        self.assertEqual(combined["invoice_rounding"], "floor_v2")
        self.assertEqual(combined["checksum"], "fnv1a64")

    def test_combined_root_deterministic(self):
        c1, _ = program.merge(program.BASE, program.LEFT, program.RIGHT)
        c2, _ = program.merge(program.BASE, program.LEFT, program.RIGHT)
        self.assertEqual(program.root_digest(c1), program.root_digest(c2))

    def test_commutative_semantic_result(self):
        cab, _ = program.merge(program.BASE, program.LEFT, program.RIGHT)
        cba, _ = program.merge(program.BASE, program.RIGHT, program.LEFT)
        self.assertEqual(cab, cba)
        self.assertEqual(program.root_digest(cab), program.root_digest(cba))

    def test_disjointness(self):
        kl = {k for k in program.LEFT if program.BASE.get(k) != program.LEFT[k]}
        kr = {k for k in program.RIGHT
              if program.BASE.get(k) != program.RIGHT[k]}
        self.assertEqual(kl & kr, set())


if __name__ == "__main__":
    unittest.main()
