"""Native-emitted query context, not mutation labels, drives the oracle."""

import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))

import check_root_backed_query_vector as oracle


class RootQueryOracleEvidence(unittest.TestCase):
    def setUp(self):
        self.accepted = json.loads(oracle.ACCEPTED.read_text())
        self.rejected = json.loads(oracle.REJECTED.read_text())
        self.source = json.loads(oracle.SOURCE.read_text())["vectors"][0]
        self.snapshot = json.loads(oracle.SNAPSHOT_SOURCE.read_text())["vectors"][0]

    def run_oracle(self):
        with tempfile.TemporaryDirectory() as directory:
            accepted = Path(directory) / "accepted.json"
            rejected = Path(directory) / "rejected.json"
            accepted.write_text(json.dumps(self.accepted))
            rejected.write_text(json.dumps(self.rejected))
            output = io.StringIO()
            with patch.object(oracle, "ACCEPTED", accepted), patch.object(
                oracle, "REJECTED", rejected
            ), contextlib.redirect_stdout(output):
                result = oracle.main()
            return result, json.loads(output.getvalue())["problems"]

    def row(self, name):
        return next(row for row in self.rejected["mutations"] if row["id"] == name)

    def honest_context(self):
        return {
            "snapshot_record_hex": self.snapshot["record_hex"],
            "snapshot_id": self.snapshot["snapshot_id"],
            "root_hex": self.accepted["context"]["root_hex"],
        }

    def test_frozen_fixture_passes(self):
        self.assertEqual(self.run_oracle(), (0, []))

    def test_rejection_names_are_not_semantic_authority(self):
        self.row("arm-1-snapshot-profile")["id"] = "restricted-context-renamed"
        self.row("binding-substituted-fact")["id"] = "substituted-context-renamed"
        self.assertEqual(self.run_oracle(), (0, []))

    def test_missing_actual_context_cannot_establish_rejection(self):
        for name in ("arm-1-snapshot-profile", "binding-substituted-fact"):
            with self.subTest(name=name):
                row = self.row(name)
                actual = row.pop("input_context")
                result, problems = self.run_oracle()
                self.assertEqual(result, 1)
                self.assertIn(f"{name}:accepted", problems)
                row["input_context"] = actual

    def test_descriptor_alone_is_not_evidence(self):
        row = self.row("arm-1-snapshot-profile")
        row.pop("input_context")
        row["tamper"] = {"arm": 1}
        result, problems = self.run_oracle()
        self.assertEqual(result, 1)
        self.assertIn("arm-1-snapshot-profile:legacy-descriptor-without-grounded-context", problems)

    def test_snapshot_bytes_and_id_are_both_verified(self):
        context = self.row("arm-1-snapshot-profile")["input_context"]
        context["snapshot_id"] = "00" * 32
        result, problems = self.run_oracle()
        self.assertEqual(result, 1)
        self.assertTrue(any(":invalid-input-evidence:" in p for p in problems), problems)

    def test_truncated_record_is_invalid_evidence_not_profile_refusal(self):
        context = self.row("arm-1-snapshot-profile")["input_context"]
        context["snapshot_record_hex"] = context["snapshot_record_hex"][:296]
        result, problems = self.run_oracle()
        self.assertEqual(result, 1)
        self.assertEqual(problems, ["arm-1-snapshot-profile:invalid-input-evidence:input-snapshot-size"])

    def test_self_hashed_changed_inventory_is_not_the_frozen_restriction(self):
        context = self.row("arm-1-snapshot-profile")["input_context"]
        record = bytearray.fromhex(context["snapshot_record_hex"])
        # Replace one entity identity in this benign conformance snapshot and
        # refresh its digest; matching hashes alone cannot prove the source graph.
        record[132] ^= 1
        record[-32:] = oracle.blake3.blake3(b"sley2.index-snapshot.v1" + record[:-32]).digest()
        context.update(snapshot_record_hex=record.hex(), snapshot_id=record[-32:].hex())
        result, problems = self.run_oracle()
        self.assertEqual(result, 1)
        self.assertTrue(any(":invalid-input-evidence:" in p for p in problems), problems)

    def test_validated_record_drives_profile_before_root_binding(self):
        root = oracle.Root(self.source, self.accepted["context"])
        row = self.row("arm-1-snapshot-profile")
        context = dict(row["input_context"], root_hex="00" * 32)
        with self.assertRaises(oracle.Failure) as caught:
            oracle.answer(root, self.accepted["context"], row, context)
        self.assertEqual(caught.exception.code, "QUERY_PROFILE_UNSUPPORTED")

    def rewrite_page(self, page):
        root = oracle.Root(self.source, self.accepted["context"])
        query_id, record, after = oracle.answer(root, self.accepted["context"], page)
        page.update(query_id=query_id.hex(), record_hex=record.hex(),
                    record_bytes=len(record), next_after=after)

    def test_overlapping_individually_valid_pages_do_not_form_one_walk(self):
        second = next(v for v in self.accepted["vectors"] if v["id"] == "page-namespaces-2")
        second["after"] = None
        self.rewrite_page(second)
        result, problems = self.run_oracle()
        self.assertEqual(result, 1)
        self.assertEqual(problems, ["page-namespaces:cursor-chain"])

    def test_individually_valid_page_with_different_limits_is_not_same_walk(self):
        second = next(v for v in self.accepted["vectors"] if v["id"] == "page-namespaces-2")
        second["limits"]["max_work"] -= 1
        self.rewrite_page(second)
        result, problems = self.run_oracle()
        self.assertEqual(result, 1)
        self.assertEqual(problems, ["page-namespaces:query-or-limits"])

    def test_actual_complete_snapshot_cannot_claim_restricted_profile_rejection(self):
        self.row("arm-1-snapshot-profile")["input_context"] = self.honest_context()
        result, problems = self.run_oracle()
        self.assertEqual(problems, ["arm-1-snapshot-profile:accepted"])

    def test_actual_unchanged_root_cannot_claim_substitution_rejection(self):
        self.row("binding-substituted-fact")["input_context"] = self.honest_context()
        result, problems = self.run_oracle()
        self.assertEqual(problems, ["binding-substituted-fact:accepted"])

    def test_pages_from_different_queries_do_not_form_one_cursor_walk(self):
        second = next(v for v in self.accepted["vectors"] if v["id"] == "page-namespaces-2")
        second["query"] = {"class": 5}
        self.rewrite_page(second)
        result, problems = self.run_oracle()
        self.assertEqual(problems, ["page-namespaces:query-or-limits"])


if __name__ == "__main__":
    unittest.main()
