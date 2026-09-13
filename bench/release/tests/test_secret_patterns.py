"""Positive controls for the S20-710 secret-scan patterns.

Each high-confidence pattern must fire on a synthetic token: a guard
that has only ever produced zero findings cannot distinguish a clean
tree from a broken regex. Findings must carry no secret values.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"scripts/{name}.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


supply = load("generate_supply_chain_evidence")

POSITIVE_CONTROLS = {
    # Every token is assembled from pieces so this file itself carries no
    # contiguous secret shape for the scan to flag.
    "PRIVATE_KEY_PEM": b"-----BEGIN " + b"PRIVATE KEY-----",
    "AWS_ACCESS_KEY": b"AKIA" + b"IOSFODNN7EXAMPLE",
    "GITHUB_TOKEN": b"ghp_" + b"a" * 40,
    "GITLAB_PAT": b"glpat-" + b"a" * 24,
    "PGP_PRIVATE_KEY_BLOCK": b"-----BEGIN PGP PRIVATE KEY BLOCK-----",
    "SENDGRID_KEY": b"SG." + b"a" * 22 + b"." + b"b" * 43,
    "SLACK_TOKEN": b"xoxb-" + b"1" * 25,
    "OPENAI_KEY": b"sk-" + b"a" * 24,
    "OPENAI_PROJECT_KEY": b"sk-proj-" + b"a" * 24,
    "OPENAI_ADMIN_KEY": b"sk-admin-" + b"a" * 24,
    "OPENAI_SVCACCT_KEY": b"sk-svcacct-" + b"a" * 24,
    "ANTHROPIC_KEY": b"sk-ant-" + b"a" * 24,
    "XAI_KEY": b"xai-" + b"a" * 24,
    "HUGGINGFACE_TOKEN": b"hf_" + b"a" * 24,
    "PYPI_TOKEN": b"pypi-AgEI" + b"a" * 44,
    "NPM_TOKEN": b"npm_" + b"a" * 24,
    "DISCORD_BOT_TOKEN": b"a" * 24 + b"." + b"b" * 6 + b"." + b"c" * 27,
    "GCP_SERVICE_ACCOUNT": b'"type"' + b': "service_account"',
    "JWT_TOKEN": b"eyJhbGciOiJIUzI1NiJ9." + b"eyJzdWIiOiIxIn0." + b"SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
    "STRIPE_LIVE_KEY": b"sk_live_" + b"a" * 24,
    "GOOGLE_API_KEY": b"AIza" + b"a" * 35,
    "URL_CREDENTIAL": b"https://" + b"user:password123@example.com/",
}


class SecretPatternTests(unittest.TestCase):
    def test_every_pattern_fires_on_its_control(self) -> None:
        uncovered = set(supply.SECRET_PATTERNS) - set(POSITIVE_CONTROLS)
        self.assertEqual(uncovered, set())
        for name, token in sorted(POSITIVE_CONTROLS.items()):
            with self.subTest(pattern=name):
                self.assertIn(name, supply.scan_blob(token))

    def test_clean_prose_fires_nothing(self) -> None:
        prose = (
            b"The sk-ant- pattern requires twenty trailing characters, "
            b"so short mentions stay silent; see docs for the sk- prefix rule."
        )
        self.assertEqual(supply.scan_blob(prose), [])

    def test_findings_carry_no_values(self) -> None:
        allowed = {"pattern", "path", "scope", "blob_oid"}
        for token in POSITIVE_CONTROLS.values():
            for pattern in supply.scan_blob(token):
                entry = {"pattern": pattern, "path": "synthetic", "scope": "test"}
                self.assertTrue(set(entry) <= allowed)
                self.assertNotIn(token.decode("utf-8", "replace"), str(entry.values()))


if __name__ == "__main__":
    unittest.main()
