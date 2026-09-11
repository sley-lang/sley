"""Offline tests of the S20-730 reproducibility and independent conformance evidence."""

from __future__ import annotations

import importlib.util
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"scripts/{name}.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


repro = load("build_reproducibility_report")
conformance = load("build_independent_conformance_report")


def evidence_record(**overrides) -> dict:
    record = {
        "artifact_name": "sley-2.0.0-linux-x86_64.tar.gz",
        "artifact_sha256": "a" * 64,
        "artifact_size_bytes": 2_050_866,
        "commit": "b" * 40,
        "manifest_digest": "c" * 64,
        "member_count": 14,
        "toolchain": {"cargo": "cargo 1.93.0", "rustc": "rustc 1.93.0"},
        "reproducibility": {"result": "REPRODUCIBLE", "differing_members": [], "archive_only": False},
        "working_tree_clean": True,
        "result": "PASS",
    }
    record.update(overrides)
    return record


class ReproducibilityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def write_evidence(self, **overrides) -> Path:
        path = self.root / "evidence.json"
        path.write_text(json.dumps(evidence_record(**overrides)), encoding="utf-8")
        return path

    def test_attestation_derives_from_a_passing_reproducible_record(self) -> None:
        attestation = repro.local_attestation("primary", self.write_evidence())
        self.assertEqual(attestation["contract"], repro.ATTESTATION_CONTRACT)
        self.assertEqual(attestation["reproducibility"], "REPRODUCIBLE")
        self.assertEqual(attestation["commit"], "b" * 40)
        # No host name, path, or time enters the attestation.
        self.assertEqual(
            set(attestation) - {
                "contract",
                "host_label",
                "commit",
                "artifact_name",
                "artifact_sha256",
                "artifact_size_bytes",
                "manifest_digest",
                "member_count",
                "toolchain",
                "reproducibility",
                "working_tree_clean",
                "differing_members",
            },
            set(),
        )

    def test_missing_failed_or_nonreproducible_evidence_fails_closed(self) -> None:
        with self.assertRaises(repro.ReproError) as missing:
            repro.local_attestation("primary", self.root / "absent.json")
        self.assertEqual(missing.exception.code, repro.ReproErrorCode.EVIDENCE_MISSING)
        for overrides, code in (
            ({"result": "FAIL"}, repro.ReproErrorCode.EVIDENCE_INVALID),
            (
                {"reproducibility": {"result": "ARCHIVE_ONLY", "differing_members": ["bin/sley"]}},
                repro.ReproErrorCode.EVIDENCE_INVALID,
            ),
        ):
            with self.assertRaises(repro.ReproError) as error:
                repro.local_attestation("primary", self.write_evidence(**overrides))
            self.assertEqual(error.exception.code, code)

    def test_one_host_reports_single_host_and_names_the_gated_lane(self) -> None:
        report = repro.build_report([repro.local_attestation("primary", self.write_evidence())])
        self.assertEqual(report["result"], "SINGLE_HOST_REPRODUCIBLE")
        self.assertEqual(report["second_host"]["status"], "GATED_OPERATOR_LANE")
        self.assertIn("second_host_attestation_operator_lane", report["blockers"])
        self.assertFalse(report["ga_claimed"])
        self.assertFalse(report["publication_authorized"])
        self.assertEqual(report["report_digest"], repro.digest_of({k: v for k, v in report.items() if k != "report_digest"}))

    def test_two_agreeing_hosts_report_multi_host(self) -> None:
        first = repro.local_attestation("primary", self.write_evidence())
        second = dict(first, host_label="secondary")
        report = repro.build_report([first, second])
        self.assertEqual(report["result"], "MULTI_HOST_REPRODUCIBLE")
        self.assertEqual(report["second_host"]["status"], "ATTESTED")
        self.assertNotIn("second_host_attestation_operator_lane", report["blockers"])
        self.assertEqual(report["commits"]["b" * 40]["hosts"], ["primary", "secondary"])

    def test_conflicting_digests_and_duplicate_labels_fail_closed(self) -> None:
        first = repro.local_attestation("primary", self.write_evidence())
        conflicting = dict(first, host_label="secondary", artifact_sha256="d" * 64)
        with self.assertRaises(repro.ReproError) as conflict:
            repro.build_report([first, conflicting])
        self.assertEqual(conflict.exception.code, repro.ReproErrorCode.ATTESTATION_CONFLICT)
        with self.assertRaises(repro.ReproError) as duplicate:
            repro.build_report([first, dict(first)])
        self.assertEqual(duplicate.exception.code, repro.ReproErrorCode.ATTESTATION_INVALID)

    def test_a_dirty_or_incomplete_attestation_is_refused(self) -> None:
        base = repro.local_attestation("primary", self.write_evidence())
        for mutation in (
            {"working_tree_clean": False},
            {"commit": "not hex"},
            {"artifact_sha256": "short"},
            {"reproducibility": "ARCHIVE_ONLY"},
            {"differing_members": ["bin/sley"]},
        ):
            with self.assertRaises(repro.ReproError) as error:
                repro.validate_attestation(dict(base, **mutation))
            self.assertEqual(error.exception.code, repro.ReproErrorCode.ATTESTATION_INVALID)
        with self.assertRaises(repro.ReproError):
            repro.validate_attestation({key: value for key, value in base.items() if key != "toolchain"})

    def other_attestation(self, label: str, commit: str, digest: str) -> dict:
        base = repro.local_attestation("primary", self.write_evidence())
        return dict(base, host_label=label, commit=commit, artifact_sha256=digest)

    def write_tracked(self, attestations: list[dict]) -> Path:
        report = repro.build_report(attestations)
        path = self.root / "reproducibility-report.json"
        path.write_text(repro.canonical(report), encoding="utf-8")
        return path

    def test_a_rebuild_carries_attestations_no_fresh_input_supersedes(self) -> None:
        tracked = self.write_tracked(
            [
                repro.local_attestation("primary", self.write_evidence()),
                self.other_attestation("secondary", "c" * 40, "d" * 64),
            ]
        )
        carried = repro.carried_attestations(tracked, {"primary"})
        self.assertEqual([item["host_label"] for item in carried], ["secondary"])

    def test_the_fresh_local_label_supersedes_its_tracked_attestation(self) -> None:
        tracked = self.write_tracked(
            [self.other_attestation("primary", "c" * 40, "d" * 64)]
        )
        self.assertEqual(repro.carried_attestations(tracked, {"primary"}), [])

    def test_an_explicit_attest_file_wins_over_the_tracked_label(self) -> None:
        tracked = self.write_tracked(
            [self.other_attestation("secondary", "c" * 40, "d" * 64)]
        )
        replacement = self.other_attestation("secondary", "e" * 40, "f" * 64)
        carried = repro.carried_attestations(tracked, {"primary", "secondary"})
        self.assertEqual(carried, [])
        report = repro.build_report(
            [repro.local_attestation("primary", self.write_evidence()), replacement, *carried]
        )
        self.assertEqual(report["commits"]["e" * 40]["hosts"], ["secondary"])

    def test_a_malformed_tracked_report_fails_closed(self) -> None:
        path = self.root / "reproducibility-report.json"
        path.write_text('{"contract": "something-else"}', encoding="utf-8")
        with self.assertRaises(repro.ReproError) as error:
            repro.carried_attestations(path, set())
        self.assertEqual(error.exception.code, repro.ReproErrorCode.ATTESTATION_INVALID)

    def test_a_missing_tracked_report_carries_nothing(self) -> None:
        self.assertEqual(
            repro.carried_attestations(self.root / "absent.json", set()), []
        )

    def test_a_hand_edited_report_fails_verification(self) -> None:
        report = repro.build_report([repro.local_attestation("primary", self.write_evidence())])
        self.assertEqual(repro.verify_report(report), [])
        tampered = dict(report, result="MULTI_HOST_REPRODUCIBLE")
        self.assertTrue(
            any("report_digest" in problem for problem in repro.verify_report(tampered))
        )
        malformed = dict(report, attestations=[{"host_label": "primary"}])
        self.assertTrue(
            any("attestation invalid" in problem for problem in repro.verify_report(malformed))
        )
        self.assertTrue(repro.verify_report({"contract": "other"}))


class IndependentConformanceTests(unittest.TestCase):
    def test_report_covers_every_fixture_family_from_tracked_files(self) -> None:
        report = conformance.build_report()
        self.assertEqual(report["contract"], conformance.REPORT_CONTRACT)
        directories = sorted(
            path.name for path in (ROOT / "conformance").iterdir() if path.is_dir()
        )
        self.assertEqual(report["fixture_directories"], len(directories))
        # The pinned corpus version per family is itself pinned: only
        # entity-read checks its vectors at v2 (S20-310 contract); sibling
        # v2 corpora are digested, summed, and declared without a depth
        # claim (contract revision 5).
        self.assertEqual(conformance.CORPUS_VERSION, {"entity-read": "v2"})
        self.assertEqual(
            [family["directory"] for family in report["fixtures"]],
            [
                f"conformance/{name}/{conformance.CORPUS_VERSION.get(name, 'v1')}"
                for name in directories
            ],
        )
        self.assertEqual(
            report["independently_checked"] + len(report["native_only"]),
            report["fixture_directories"],
        )

    def test_every_declared_oracle_command_is_in_the_make_recipe(self) -> None:
        recipe = conformance.conformance_recipe()
        for family in conformance.build_report()["fixtures"]:
            coverage = family["coverage"]
            if coverage["kind"] == "independent_oracle":
                self.assertIn(coverage["command"], recipe)
            else:
                self.assertEqual(coverage["kind"], "native_only")
                self.assertTrue(coverage["note"])

    def test_the_report_is_a_pure_function_of_the_tree(self) -> None:
        self.assertEqual(conformance.build_report(), conformance.build_report())

    def test_an_undeclared_family_fails_closed(self) -> None:
        recipe = conformance.conformance_recipe()
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp) / "brand-new"
            (directory / "v1").mkdir(parents=True)
            (directory / "v1/accepted.json").write_text("{}", encoding="utf-8")
            with self.assertRaises(conformance.ConformanceError) as error:
                conformance.family_record(directory, recipe)
            self.assertEqual(error.exception.code, conformance.ConformanceErrorCode.ORACLE_DRIFT)

    def test_a_malformed_fixture_fails_closed(self) -> None:
        recipe = conformance.conformance_recipe()
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp) / "scb1"
            (directory / "v1").mkdir(parents=True)
            (directory / "v1/accepted.json").write_text("{not json", encoding="utf-8")
            with self.assertRaises(conformance.ConformanceError) as error:
                conformance.family_record(directory, recipe)
            self.assertEqual(
                error.exception.code, conformance.ConformanceErrorCode.FIXTURE_UNREADABLE
            )

    def test_mismatched_sums_fail_closed(self) -> None:
        recipe = conformance.conformance_recipe()
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp) / "scb1"
            (directory / "v1").mkdir(parents=True)
            (directory / "v1/accepted.json").write_text('{"contract": "x"}', encoding="utf-8")
            (directory / "v1/SHA256SUMS").write_text(f"{'e' * 64}  accepted.json\n", encoding="utf-8")
            with self.assertRaises(conformance.ConformanceError) as error:
                conformance.family_record(directory, recipe)
            self.assertEqual(error.exception.code, conformance.ConformanceErrorCode.SUMS_MISMATCH)

    def test_a_missing_pinned_version_directory_fails_closed(self) -> None:
        recipe = conformance.conformance_recipe()
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp) / "scb1"
            directory.mkdir(parents=True)
            with self.assertRaises(conformance.ConformanceError) as error:
                conformance.family_record(directory, recipe)
            self.assertEqual(
                error.exception.code, conformance.ConformanceErrorCode.FIXTURE_UNREADABLE
            )

    def test_a_version_without_a_manifest_fails_closed(self) -> None:
        recipe = conformance.conformance_recipe()
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp) / "scb1"
            (directory / "v1").mkdir(parents=True)
            (directory / "v1/accepted.json").write_text('{"contract": "x"}', encoding="utf-8")
            with self.assertRaises(conformance.ConformanceError) as error:
                conformance.family_record(directory, recipe)
            self.assertEqual(
                error.exception.code, conformance.ConformanceErrorCode.FIXTURE_UNREADABLE
            )

    def test_conflicting_duplicate_manifest_lines_fail_closed(self) -> None:
        recipe = conformance.conformance_recipe()
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp) / "scb1"
            (directory / "v1").mkdir(parents=True)
            (directory / "v1/accepted.json").write_text('{"contract": "x"}', encoding="utf-8")
            digest = hashlib.sha256(b'{"contract": "x"}').hexdigest()
            (directory / "v1/SHA256SUMS").write_text(
                f"{digest}  accepted.json\n{'e' * 64}  accepted.json\n", encoding="utf-8"
            )
            with self.assertRaises(conformance.ConformanceError) as error:
                conformance.family_record(directory, recipe)
            self.assertEqual(error.exception.code, conformance.ConformanceErrorCode.SUMS_MISMATCH)

    def test_a_corpus_pin_outside_coverage_fails_closed(self) -> None:
        pinned = dict(conformance.CORPUS_VERSION)
        conformance.CORPUS_VERSION["brand-new"] = "v1"
        try:
            with self.assertRaises(conformance.ConformanceError) as error:
                conformance.validate_corpus_versions()
            self.assertEqual(error.exception.code, conformance.ConformanceErrorCode.ORACLE_DRIFT)
        finally:
            conformance.CORPUS_VERSION.clear()
            conformance.CORPUS_VERSION.update(pinned)

    def test_tracked_sibling_versions_are_enumerated_without_depth(self) -> None:
        recipe = conformance.conformance_recipe()
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp) / "scb1"
            for version in ("v1", "v2"):
                (directory / version).mkdir(parents=True)
                (directory / f"{version}/accepted.json").write_text(
                    '{"contract": "x"}', encoding="utf-8"
                )
            digest = hashlib.sha256(b'{"contract": "x"}').hexdigest()
            for version in ("v1", "v2"):
                (directory / f"{version}/SHA256SUMS").write_text(
                    f"{digest}  accepted.json\n", encoding="utf-8"
                )
            family = conformance.family_record(directory, recipe)
            self.assertEqual(family["directory"], "conformance/scb1/v1")
            self.assertEqual(family["tracked_versions"], ["v1", "v2"])
            self.assertEqual(set(family["siblings"]), {"v2"})
            sibling = family["siblings"]["v2"]
            self.assertEqual(sibling["coverage"]["kind"], "tracked_sibling")
            self.assertEqual(sibling["coverage"]["pinned_version"], "v1")

    def test_tracked_corpus_directories_bind_version_enumeration(self) -> None:
        report = conformance.build_report()
        counted = sum(len(family["tracked_versions"]) for family in report["fixtures"])
        self.assertEqual(report["tracked_corpus_directories"], counted)
        on_disk = sum(
            len([path for path in (ROOT / "conformance" / family["directory"].split("/")[1]).iterdir() if path.is_dir()])
            for family in report["fixtures"]
        )
        self.assertEqual(report["tracked_corpus_directories"], on_disk)

    def test_a_non_version_family_entry_fails_closed(self) -> None:
        recipe = conformance.conformance_recipe()
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp) / "scb1"
            (directory / "v1").mkdir(parents=True)
            (directory / "v1/accepted.json").write_text('{"contract": "x"}', encoding="utf-8")
            digest = hashlib.sha256(b'{"contract": "x"}').hexdigest()
            (directory / "v1/SHA256SUMS").write_text(
                f"{digest}  accepted.json\n", encoding="utf-8"
            )
            (directory / "notes").mkdir(parents=True)
            with self.assertRaises(conformance.ConformanceError) as error:
                conformance.family_record(directory, recipe)
            self.assertEqual(
                error.exception.code, conformance.ConformanceErrorCode.FIXTURE_UNREADABLE
            )

    def test_a_nested_version_entry_fails_closed(self) -> None:
        recipe = conformance.conformance_recipe()
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp) / "scb1"
            (directory / "v1").mkdir(parents=True)
            (directory / "v1/accepted.json").write_text('{"contract": "x"}', encoding="utf-8")
            digest = hashlib.sha256(b'{"contract": "x"}').hexdigest()
            (directory / "v1/SHA256SUMS").write_text(
                f"{digest}  accepted.json\n", encoding="utf-8"
            )
            (directory / "v1/archive").mkdir(parents=True)
            with self.assertRaises(conformance.ConformanceError) as error:
                conformance.family_record(directory, recipe)
            self.assertEqual(
                error.exception.code, conformance.ConformanceErrorCode.FIXTURE_UNREADABLE
            )


class CoverageDepthTests(unittest.TestCase):
    def test_every_independent_family_declares_a_valid_depth(self) -> None:
        report = conformance.build_report()
        depths = {"semantic": [], "codec_and_identity": []}
        for family in report["fixtures"]:
            coverage = family["coverage"]
            if coverage["kind"] == "native_only":
                continue
            self.assertIn(coverage.get("depth"), ("semantic", "codec_and_identity"), family["directory"])
            depths[coverage["depth"]].append(family["directory"])
        self.assertEqual(
            report["coverage_depths"],
            {depth: sorted(directories) for depth, directories in depths.items()},
        )

    def test_the_semantic_families_recompute_outcomes(self) -> None:
        report = conformance.build_report()
        semantic = set(report["coverage_depths"]["semantic"])
        self.assertEqual(
            semantic,
            {
                "conformance/complete-entity-impact/v1",
                "conformance/entity-read/v2",
                "conformance/merge/v1",
                "conformance/root-backed-query/v1",
                "conformance/semantic-comparison/v1",
            },
        )

    def test_the_documented_codec_families_stay_codec(self) -> None:
        report = conformance.build_report()
        codec = set(report["coverage_depths"]["codec_and_identity"])
        self.assertIn("conformance/vm-extended/v1", codec)
        self.assertIn("conformance/release-demo/v1", codec)
        self.assertEqual(
            len(report["coverage_depths"]["semantic"])
            + len(report["coverage_depths"]["codec_and_identity"]),
            report["independently_checked"],
        )

    def test_depth_coverage_matches_the_declared_map(self) -> None:
        independent = {name for name, command in conformance.COVERAGE.items() if command}
        self.assertEqual(set(conformance.DEPTH), independent)


if __name__ == "__main__":
    unittest.main()
