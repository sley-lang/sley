"""Unit tests for S2B-MODULE-001 (stdlib unittest)."""

import inspect
import unittest

import program


class ModuleTest(unittest.TestCase):
    def test_reference_count(self):
        self.assertEqual(len(program.REFS), 6)

    def test_all_resolve_to_integrity(self):
        for r in program.REFS:
            with self.subTest(ref=r.__name__):
                src = inspect.getsource(r)
                self.assertIn("integrity.checksum", src)

    def test_single_implementation(self):
        src = inspect.getsource(program)
        self.assertEqual(src.count("def checksum"), 1)

    def test_outputs_agree(self):
        for data in program.INPUTS:
            vals = {r(data) for r in program.REFS}
            self.assertEqual(len(vals), 1)

    def test_observation_digest_unchanged(self):
        self.assertEqual(program.observation_digest(),
                         program.EXPECTED_OBS_DIGEST)


if __name__ == "__main__":
    unittest.main()
