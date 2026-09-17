"""Unit tests for S2B-CAP-001 (stdlib unittest)."""

import unittest

import program
from program import Capability, read_with_cap, CapScopeMismatch


class CapTest(unittest.TestCase):
    def test_configured_read_succeeds(self):
        cap = Capability(program.ALLOWED)
        self.assertEqual(read_with_cap(cap, program.ALLOWED), b"rate=725")

    def test_sibling_denied_with_stable_code(self):
        cap = Capability(program.ALLOWED)
        with self.assertRaises(CapScopeMismatch) as cm:
            read_with_cap(cap, program.DENIED)
        self.assertEqual(cm.exception.code, "CAP_SCOPE_MISMATCH")

    def test_exact_object_scope_only(self):
        cap = Capability(program.ALLOWED)
        self.assertEqual(cap.scope, "config/settings.bin")
        self.assertNotIn("*", cap.scope)

    def test_wildcard_scope_refused(self):
        with self.assertRaises(program.ScopeError) as cm:
            Capability("config/*")
        self.assertEqual(cm.exception.code, "ORACLE_SCOPE_WIDENED")


if __name__ == "__main__":
    unittest.main()
