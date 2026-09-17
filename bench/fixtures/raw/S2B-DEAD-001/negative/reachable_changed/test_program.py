"""Unit tests for S2B-DEAD-001 (stdlib unittest)."""

import re
import unittest

import program


class DeadTest(unittest.TestCase):
    def test_unreachable_block_removed(self):
        src = program.module_source()
        ns = dir(program)
        for name in program.REMOVED:
            with self.subTest(name=name):
                self.assertNotIn(name, ns)
                self.assertIsNone(
                    re.search(r"^\s*(def|class)\s+" + name + r"\b", src,
                              re.MULTILINE),
                    name + " must have no definition")

    def test_public_entities_intact(self):
        for name in program.PUBLIC:
            self.assertTrue(callable(getattr(program, name)), name)

    def test_observation_digest_unchanged(self):
        self.assertEqual(program.observation_digest(),
                         program.EXPECTED_OBS_DIGEST)

    def test_effect_closure_not_expanded(self):
        self.assertEqual(program.EFFECTS, frozenset({"Pure"}))

    def test_reachable_values(self):
        self.assertEqual(program.calculate(5), 13)
        self.assertEqual(program.format_result(5), "result:13")


if __name__ == "__main__":
    unittest.main()
