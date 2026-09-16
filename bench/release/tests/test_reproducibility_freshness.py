"""Freshness checks exercise real isolated git histories and toolchain inputs."""

import json
import hashlib
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from bench.release.tests.test_reproducibility import load, evidence_record, repro

checker = load("check_reproducibility_and_independent_conformance")


class FreshnessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        (self.root / "source.txt").write_text("first\n")
        self.commit()
        self.source = self.git("rev-parse", "HEAD").strip()
        self.report = {"attestations": [{"commit": self.source}]}
        self.surface = ("source.txt",)
        self.patch = patch.object(checker, "ROOT", self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)

    def git(self, *args):
        return subprocess.check_output(["git", "-c", "user.name=Fixture", "-c",
            "user.email=fixture@example.invalid", *args], cwd=self.root, text=True)

    def commit(self):
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")

    def test_records_only_descendant_is_fresh(self):
        (self.root / "record.json").write_text('{}\n')
        self.commit()
        self.assertEqual(checker.history_problems(self.report, self.surface), [])

    def test_committed_source_change_is_stale(self):
        (self.root / "source.txt").write_text("changed\n")
        self.commit()
        self.assertTrue(any(":stale:" in x for x in checker.history_problems(self.report, self.surface)))

    def test_uncommitted_source_change_is_stale(self):
        (self.root / "source.txt").write_text("changed\n")
        self.assertTrue(any("uncommitted-surface" in x for x in checker.history_problems(self.report, self.surface)))

    def test_unrelated_attestation_and_unavailable_history_refuse(self):
        other = {"attestations": [{"commit": "f" * 40}]}
        self.assertTrue(any("not-in-history" in x for x in checker.history_problems(other, self.surface)))
        (self.root / ".git").rename(self.root / "saved-git")
        self.assertEqual(checker.history_problems(self.report, self.surface), ["reproducibility-report:history-unavailable"])

    def test_toolchain_change_and_unavailability_refuse(self):
        evidence = self.root / "evidence.json"
        evidence.write_text(json.dumps(evidence_record()))
        attestation = repro.local_attestation("primary", evidence)
        report = {"attestations": [attestation]}
        # Module lookup stays on the real source tree; only git tests patch ROOT.
        with patch.object(checker, "ROOT", Path(__file__).resolve().parents[3]):
            self.assertEqual(checker.toolchain_problems(report, attestation["toolchain"]), [])
            self.assertTrue(any("toolchain-changed" in x for x in checker.toolchain_problems(report, {"cargo": "other", "rustc": "other"})))
            self.assertEqual(checker.toolchain_problems(report, None), ["reproducibility-report:toolchain-unavailable"])

    def test_receipt_binds_retained_bytes_and_the_actual_merge(self):
        evidence = self.root / "evidence.json"
        evidence.write_text(json.dumps(evidence_record(commit=self.source)))
        primary = repro.local_attestation("primary", evidence)
        secondary = dict(primary, host_label="secondary")
        report = repro.build_report([primary, secondary])
        relative = "evidence/release/attestations/secondary.json"
        path = self.root / relative
        path.parent.mkdir(parents=True)
        path.write_text(repro.canonical(secondary))
        report_path = self.root / "evidence/release/reproducibility-report.json"
        report_path.write_text(repro.canonical(report))
        self.commit()
        merged = self.git("rev-parse", "HEAD").strip()
        row = {"candidate_commit": self.source, "date": "2026-09-15",
               "transport_lane": "fixture", "lab_checkout": "fixture",
               "lab_commit": self.source, "bundle_commit": self.source,
               "lab_evidence_sha256": "a" * 64, "artifact_sha256": primary["artifact_sha256"],
               "attestation_path": relative, "attestation_file_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
               "merge_commit": merged}
        ledger_path = self.root / "evidence/release/second-host-lane-records.json"
        with patch.object(checker, "load_module", return_value=repro):
            for changes in ({}, {"attestation_file_sha256": "0" * 64},
                            {"lab_commit": "f" * 40}, {"merge_commit": self.source},
                            {"attestation_path": "../../outside.json"}):
                ledger = {"contract": "sley2.second-host-lane-records.v1", "records": [dict(row, **changes)]}
                ledger_path.write_text(json.dumps(ledger))
                with self.subTest(changes=changes):
                    self.assertEqual(bool(checker.lane_record_problems(report)), bool(changes))
            for ledger in (None, {"contract": "sley2.second-host-lane-records.v1", "records": []}):
                ledger_path.write_text(json.dumps(ledger))
                self.assertTrue(checker.lane_record_problems(report))
