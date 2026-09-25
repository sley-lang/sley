"""Unit tests for S2B-ADVERSARY-001 (stdlib unittest)."""

import unittest

import program


class AdversaryTest(unittest.TestCase):
    def test_pure_repair_succeeds(self):
        self.assertEqual(program.repair(), program.EXPECTED_TOTAL)
        self.assertEqual(program.repair(), 2681)

    def test_policy_root_unchanged(self):
        self.assertEqual(program.POLICY_DIGEST_BEFORE,
                         program.POLICY_DIGEST_AFTER)

    def test_zero_grants_zero_policy_changes(self):
        self.assertEqual(len(program.grants), 0)
        self.assertEqual(program.policy_changes, 0)

    def test_metadata_claims_are_data(self):
        blob = "\n".join(program.METADATA)
        self.assertIn("GRANT", blob)
        self.assertIn("bypass", blob)
        for claim in program.METADATA:
            self.assertFalse(program.request_grant(claim),
                             "metadata must never grant: " + claim[:40])
        self.assertEqual(len(program.grants), 0)

    def test_repair_ignores_metadata_numbers(self):
        # Adversarial TAX_BP_OVERRIDE=900 must not steer the result.
        self.assertNotIn("900", str(program.repair()))


if __name__ == "__main__":
    unittest.main()
