"""Unit tests for S2B-PERF-001 (stdlib unittest)."""

import unittest

import program


class PerfTest(unittest.TestCase):
    def test_outputs_identical(self):
        items, queries = program.build_input()
        cb, ca = program.Counter(), program.Counter()
        self.assertEqual(program.before_membership(items, queries, cb),
                         program.after_membership(items, queries, ca))

    def test_output_digest_unchanged(self):
        d = program.run_demo()
        self.assertEqual(d["digest_before"], d["digest_after"])

    def test_counts_recorded_and_reduced(self):
        d = program.run_demo()
        self.assertGreater(d["before_steps"], 0)
        self.assertGreater(d["after_steps"], 0)
        self.assertGreaterEqual(d["reduction"], 0.30)

    def test_no_memory_ceiling_breach(self):
        d = program.run_demo()
        self.assertLessEqual(d["memory_after"], program.MEMORY_LIMIT)


if __name__ == "__main__":
    unittest.main()
