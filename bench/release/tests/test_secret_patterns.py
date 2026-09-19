"""Positive controls for the S20-710 secret-scan patterns.

Each high-confidence pattern must fire on a synthetic token: a guard
that has only ever produced zero findings cannot distinguish a clean
tree from a broken regex. Findings must carry no secret values.
"""

from __future__ import annotations

import importlib.util
import json
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
    # No PGP-armour control: the pattern is declined (see generator), so no
    # control may name it, or the suite would pin a shape the scan must not
    # flag.
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




class CommitBoundViewTests(unittest.TestCase):
    """`--check` compares the tracked T54 record with its working-tree-only
    counters masked: an untracked non-ignored file must not read as drift
    of the commit-bound record (Ariadne P3 / Nabu P4 at 178873d7)."""

    def test_untracked_counters_are_masked_for_comparison(self) -> None:
        tracked = supply.canonical_json(
            {"a": 1, "untracked_bytes_scanned": 0, "untracked_files_scanned": 0}
        )
        working = supply.canonical_json(
            {"a": 1, "untracked_bytes_scanned": 288_875, "untracked_files_scanned": 19}
        )
        self.assertNotEqual(tracked, working)
        self.assertEqual(supply.commit_bound_view(tracked), supply.commit_bound_view(working))

    def test_commit_bound_fields_still_drift(self) -> None:
        tracked = supply.canonical_json({"candidate_file_manifest_sha256": "a" * 64})
        working = supply.canonical_json({"candidate_file_manifest_sha256": "b" * 64})
        self.assertNotEqual(supply.commit_bound_view(tracked), supply.commit_bound_view(working))

    def test_non_json_payloads_compare_byte_for_byte(self) -> None:
        self.assertEqual(supply.commit_bound_view(b"not json"), b"not json")

    def test_check_decision_masks_untracked_but_refuses_edits_and_dirty_mints(self) -> None:
        # The `--check` decision `main()` applies (Ariadne P4 at 92fa6646:
        # the wiring, not only the view): a working tree with an added
        # untracked file is not drift; a modified tracked input is; and a
        # tracked record that is not the generator's canonical bytes, or that
        # was minted from a dirty tree (nonzero untracked counters), is drift
        # as well (Vulcan P4 at 92fa6646).
        tracked = supply.canonical_json(
            {"a": 1, "untracked_bytes_scanned": 0, "untracked_files_scanned": 0}
        )
        working = supply.canonical_json(
            {"a": 1, "untracked_bytes_scanned": 288_875, "untracked_files_scanned": 19}
        )
        self.assertFalse(supply.record_drifted(tracked, working))
        edited = supply.canonical_json(
            {"a": 2, "untracked_bytes_scanned": 288_875, "untracked_files_scanned": 19}
        )
        self.assertTrue(supply.record_drifted(tracked, edited))
        reformatted = json.dumps(json.loads(tracked), indent=2).encode()
        self.assertNotEqual(reformatted, tracked)
        self.assertTrue(supply.record_drifted(reformatted, working))
        dirty_mint = supply.canonical_json(
            {"a": 1, "untracked_bytes_scanned": 10, "untracked_files_scanned": 1}
        )
        self.assertTrue(supply.record_drifted(dirty_mint, working))
        self.assertTrue(supply.record_drifted(b"not json", b"not json"))
        # A counter is clear only as the integer zero: absent, false and 0.0
        # are not counted zeros (Vulcan P4 carried c04539b9..76ae15ab).
        for counters in (
            {"untracked_bytes_scanned": 0},
            {"untracked_bytes_scanned": False, "untracked_files_scanned": 0},
            {"untracked_bytes_scanned": 0.0, "untracked_files_scanned": 0},
        ):
            tolerant = supply.canonical_json({"a": 1, **counters})
            self.assertTrue(supply.record_drifted(tolerant, working), counters)


if __name__ == "__main__":
    unittest.main()
