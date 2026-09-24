"""Production-staging regression: scripted solvers stay test-only.

Production scratch holds exactly the permitted file set (documented
tooling, generic shim, generic transport) with no solver, witness,
or event-generator assets. Deterministic tests inject their stand-in
at the provider boundary after that staging is verified, exercising
the real confinement, mediation, capture, oracle, append, and
verification machinery.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


class ProductionStagingTests(unittest.TestCase):
    def test_permitted_set_and_no_solver_assets(self) -> None:
        from bench.live.mediated_attempt import (
            FORBIDDEN_STAGING_PATTERNS,
            PRODUCTION_STAGING_FILES,
            assert_production_staging_clean,
            production_staging_digest,
            stage_mediated_scratch,
        )

        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp) / "scratch"
            scratch.mkdir(mode=0o700)
            stage_mediated_scratch(scratch)
            # Permitted set exact.
            self.assertEqual(tuple(PRODUCTION_STAGING_FILES), (
                ".sley-live/TOOLING.md",
                ".sley-live/sley-tool",
                "mediated_transport.py",
            ))
            # Every legitimate agent-visible input digested.
            digests = production_staging_digest(scratch)
            self.assertEqual(set(digests), set(PRODUCTION_STAGING_FILES))
            for value in digests.values():
                self.assertEqual(len(value), 64)
            # Regression gate passes on production staging.
            assert_production_staging_clean(scratch)
            # No solver/witness/event-generator assets anywhere in
            # the staged production bytes.
            for rel in PRODUCTION_STAGING_FILES:
                text = (scratch / rel).read_text(encoding="utf-8")
                for pattern in FORBIDDEN_STAGING_PATTERNS:
                    self.assertNotIn(pattern, text, f"{pattern} in {rel}")
            # The test-only adapter still carries the scripted
            # sequences (proving the split did not delete test
            # behavior), but it is never in production staging.
            client = (ROOT / "bench" / "live" / "mediated_client.py").read_text(
                encoding="utf-8")
            for marker in ("seq_type", "seq_stale", "CLIENT_MEMBERS",
                           "emit_provider_stream"):
                self.assertIn(marker, client)
            self.assertFalse((scratch / "mediated_client.py").exists())

    def test_test_adapter_injects_after_production(self) -> None:
        from bench.live.mediated_attempt import (
            assert_production_staging_clean,
            stage_mediated_scratch,
            stage_test_adapter,
        )

        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp) / "scratch"
            scratch.mkdir(mode=0o700)
            stage_mediated_scratch(scratch)
            assert_production_staging_clean(scratch)
            dest = stage_test_adapter(scratch)
            self.assertTrue(dest.is_file())
            # Production gate now fails (extra file present) — proving
            # the adapter is test-only injection, not production.
            with self.assertRaises(Exception):
                assert_production_staging_clean(scratch)


if __name__ == "__main__":
    unittest.main()
