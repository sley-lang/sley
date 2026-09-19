"""Offline tests of the S20-730 reproducibility and independent conformance evidence."""

from __future__ import annotations

import importlib.util
import hashlib
import json
import re
import subprocess
import sys
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
candidate_mechanics = load("build_release_candidate")


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

    def test_attestation_refuses_foreign_or_non_string_artifact_name(self) -> None:
        valid = repro.local_attestation("primary", self.write_evidence())
        for name in (None, "foreign.tar.gz", 42):
            with self.subTest(name=name), self.assertRaises(repro.ReproError):
                repro.validate_attestation(dict(valid, artifact_name=name))

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

    def test_a_re_mint_supersedes_attestations_of_another_commit(self) -> None:
        local = repro.local_attestation("primary", self.write_evidence())
        tracked = self.write_tracked(
            [local, self.other_attestation("secondary", "c" * 40, "d" * 64)]
        )
        superseded: list[dict] = []
        carried = repro.carried_attestations(tracked, {"primary"}, local["commit"], superseded)
        self.assertEqual(carried, [])
        self.assertEqual(
            superseded,
            [
                {
                    "host_label": "secondary",
                    "commit": "c" * 40,
                    "artifact_sha256": "d" * 64,
                    "reason": "attests a commit the re-mint superseded",
                }
            ],
        )
        report = repro.build_report([local], superseded)
        self.assertEqual(report["distinct_hosts"], 1)
        self.assertEqual(report["superseded_attestations"], superseded)
        # The same commit's other host is still carried.
        same = self.other_attestation("secondary", local["commit"], local["artifact_sha256"])
        tracked = self.write_tracked([local, same])
        superseded = []
        carried = repro.carried_attestations(tracked, {"primary"}, local["commit"], superseded)
        self.assertEqual([item["host_label"] for item in carried], ["secondary"])
        self.assertEqual(superseded, [])

    def test_the_supersession_listing_persists_until_the_host_re_attests(self) -> None:
        # Revision 11 (Ariadne P2 at 92fa6646): the listing survived exactly
        # one build. A plain rebuild of the same candidate must carry the
        # tracked listing forward; only the superseded host's re-attestation
        # of the current commit retires its entry.
        local = repro.local_attestation("primary", self.write_evidence())
        entry = {
            "host_label": "secondary",
            "commit": "c" * 40,
            "artifact_sha256": "d" * 64,
            "reason": "attests a commit the re-mint superseded",
        }
        report = repro.build_report([local], [entry])
        path = self.root / "reproducibility-report.json"
        path.write_text(repro.canonical(report), encoding="utf-8")
        superseded: list[dict] = []
        carried = repro.carried_attestations(path, {"primary"}, local["commit"], superseded)
        self.assertEqual(carried, [])
        self.assertEqual(superseded, [entry])
        # A second plain rebuild is idempotent: no duplicate, no drop.
        path.write_text(repro.canonical(repro.build_report([local], superseded)), encoding="utf-8")
        superseded = []
        repro.carried_attestations(path, {"primary"}, local["commit"], superseded)
        self.assertEqual(superseded, [entry])
        # The secondary re-attests the current commit explicitly: retired.
        superseded = []
        repro.carried_attestations(path, {"primary", "secondary"}, local["commit"], superseded)
        self.assertEqual(superseded, [])
        # ... or its re-attestation of the current commit is already tracked.
        same = self.other_attestation("secondary", local["commit"], local["artifact_sha256"])
        path.write_text(repro.canonical(repro.build_report([local, same], [entry])), encoding="utf-8")
        superseded = []
        carried = repro.carried_attestations(path, {"primary"}, local["commit"], superseded)
        self.assertEqual([item["host_label"] for item in carried], ["secondary"])
        self.assertEqual(superseded, [])
        # A malformed tracked entry fails closed.
        broken = repro.build_report([local], [dict(entry, commit="zz")])
        path.write_text(repro.canonical(broken), encoding="utf-8")
        with self.assertRaises(repro.ReproError):
            repro.carried_attestations(path, {"primary"}, local["commit"], [])

    def test_a_tracked_report_is_verified_before_it_is_carried(self) -> None:
        # Vulcan P4 at c04539b9: a hand-edited tracked report (stale digest)
        # or a listing entry naming the current commit / a same-key digest
        # conflict is refused rather than laundered by a plain rebuild.
        local = repro.local_attestation("primary", self.write_evidence())
        entry = {"host_label": "secondary", "commit": "c" * 40, "artifact_sha256": "d" * 64,
                 "reason": "attests a commit the re-mint superseded"}
        path = self.root / "reproducibility-report.json"
        report = repro.build_report([local], [entry])
        report["distinct_hosts"] = 7  # digest no longer recomputes
        path.write_text(repro.canonical(report), encoding="utf-8")
        with self.assertRaises(repro.ReproError) as error:
            repro.carried_attestations(path, {"primary"}, local["commit"], [])
        self.assertEqual(error.exception.code, repro.ReproErrorCode.ATTESTATION_INVALID)
        current = repro.build_report([local], [dict(entry, commit=local["commit"])])
        path.write_text(repro.canonical(current), encoding="utf-8")
        with self.assertRaises(repro.ReproError) as error:
            repro.carried_attestations(path, {"primary"}, local["commit"], [])
        self.assertEqual(error.exception.code, repro.ReproErrorCode.ATTESTATION_CONFLICT)
        conflict = repro.build_report([local], [entry, dict(entry, artifact_sha256="e" * 64)])
        path.write_text(repro.canonical(conflict), encoding="utf-8")
        with self.assertRaises(repro.ReproError) as error:
            repro.carried_attestations(path, {"primary"}, local["commit"], [])
        self.assertEqual(error.exception.code, repro.ReproErrorCode.ATTESTATION_CONFLICT)

    def test_verify_report_requires_and_shapes_the_supersession_listing(self) -> None:
        # Revision 11 (Ariadne P4 at 92fa6646): the hermetic gate validates
        # the listing's shape, not only the attestations.
        local = repro.local_attestation("primary", self.write_evidence())
        entry = {
            "host_label": "secondary",
            "commit": "c" * 40,
            "artifact_sha256": "d" * 64,
            "reason": "attests a commit the re-mint superseded",
        }
        self.assertEqual(repro.verify_report(repro.build_report([local], [entry])), [])
        missing = repro.build_report([local])
        del missing["superseded_attestations"]
        missing["report_digest"] = repro.digest_of(
            {key: value for key, value in missing.items() if key != "report_digest"}
        )
        self.assertIn("no superseded_attestations list", repro.verify_report(missing))
        for field, value in [("commit", "c" * 39), ("artifact_sha256", "D" * 64),
                             ("host_label", ""), ("reason", 7)]:
            report = repro.build_report([local], [dict(entry, **{field: value})])
            self.assertTrue(
                any(problem.startswith("superseded attestation invalid") for problem in repro.verify_report(report)),
                field,
            )
        extra = repro.build_report([local], [dict(entry, note="x")])
        self.assertTrue(repro.verify_report(extra))

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

    def test_an_explicit_attest_file_of_another_commit_is_refused(self) -> None:
        # Revision 11 (Vulcan P4 at 92fa6646): the builder refuses, rather
        # than merging, an explicit attestation of a commit other than the
        # one the fresh local attestation names.
        local = repro.local_attestation("primary", self.write_evidence())
        same = self.other_attestation("secondary", local["commit"], local["artifact_sha256"])
        repro.explicit_of_current_commit([same], local["commit"])
        other = self.other_attestation("secondary", "e" * 40, "f" * 64)
        with self.assertRaises(repro.ReproError) as error:
            repro.explicit_of_current_commit([same, other], local["commit"])
        self.assertEqual(error.exception.code, repro.ReproErrorCode.ATTESTATION_CONFLICT)

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

    def test_the_admissibility_predicate_is_the_attestation_contract(self) -> None:
        attestation = repro.local_attestation("primary", self.write_evidence())
        self.assertTrue(repro.admissible_attestation(attestation))
        for denied in (
            {**attestation, "working_tree_clean": False},
            {**attestation, "reproducibility": "DIFFERS"},
            {**attestation, "differing_members": ["bin/sley"]},
            {**attestation, "toolchain": {"cargo": "", "rustc": "rustc 1.93.0"}},
            {**attestation, "toolchain": None},
            {**attestation, "commit": "not-hex"},
            {key: value for key, value in attestation.items() if key != "manifest_digest"},
            {"reproducibility": "REPRODUCIBLE", "working_tree_clean": True},
            None,
            [],
        ):
            self.assertFalse(repro.admissible_attestation(denied), denied)

    def test_admissible_attestations_filter_a_report_and_a_commit(self) -> None:
        first = repro.local_attestation("primary", self.write_evidence())
        second = repro.local_attestation("secondary", self.write_evidence())
        other = repro.local_attestation("tertiary", self.write_evidence(commit="d" * 40))
        report = repro.build_report([first, second, other])
        report["attestations"].append({"host_label": "forged", "reproducibility": "REPRODUCIBLE"})
        labels = [item["host_label"] for item in repro.admissible_attestations(report)]
        self.assertEqual(labels, ["primary", "secondary", "tertiary"])
        self.assertEqual(
            [item["host_label"] for item in repro.admissible_attestations(report, "d" * 40)],
            ["tertiary"],
        )
        self.assertEqual(repro.admissible_attestations(report, "e" * 40), [])
        self.assertEqual(repro.admissible_attestations(None), [])
        self.assertEqual(repro.admissible_attestations({"attestations": "x"}), [])

    def test_candidate_selection_has_one_owner_and_fails_closed_on_ties(self) -> None:
        first = repro.local_attestation("primary", self.write_evidence())
        second = repro.local_attestation("secondary", self.write_evidence())
        stale = repro.local_attestation("archive", self.write_evidence(commit="d" * 40))
        report = repro.build_report([stale, first, second])
        # attestations[0] is the alphabetically first label ("archive"),
        # which names the wrong commit; the selection follows the hosts.
        self.assertEqual(report["attestations"][0]["host_label"], "archive")
        selected = repro.select_attestation(report)
        self.assertIsNotNone(selected)
        self.assertEqual((selected["commit"], selected["host_label"]), ("b" * 40, "primary"))
        # A candidate evidence record binds the 4-tuple exactly.
        candidate = evidence_record()
        self.assertEqual(repro.select_attestation(report, candidate)["host_label"], "primary")
        self.assertIsNone(repro.select_attestation(report, {**candidate, "artifact_size_bytes": 1}))
        self.assertEqual(
            repro.select_attestation(report, {**candidate, "commit": "d" * 40})["host_label"],
            "archive",
        )
        self.assertIsNone(repro.select_attestation(report, {**candidate, "commit": "e" * 40}))
        self.assertIsNone(repro.select_attestation(report, {"commit": "b" * 40}))
        self.assertTrue(repro.binds_candidate(first, candidate))
        self.assertFalse(repro.binds_candidate(stale, candidate))
        # Two single-host commits tie: no current candidate, never a guess.
        tie = repro.build_report([stale, first])
        self.assertIsNone(repro.select_attestation(tie))
        self.assertEqual(repro.select_attestation(tie, commit="d" * 40)["host_label"], "archive")
        self.assertIsNone(repro.select_attestation({"attestations": []}))

    def test_each_recorded_candidate_is_selectable_with_its_binding(self) -> None:
        report = json.loads(repro.REPORT.read_text(encoding="utf-8"))
        for candidate in repro.admissible_attestations(report):
            selected = repro.select_attestation(report, candidate=candidate)
            self.assertIsNotNone(selected)
            self.assertTrue(repro.binds_candidate(selected, candidate))
            self.assertIn(selected["commit"], report["commits"])
        self.assertEqual(
            len(repro.admissible_attestations(report)), report["distinct_hosts"]
        )

    def test_realized_codes_recorded_is_derived_from_the_threat_report(self) -> None:
        # Vulcan P4 at 92fa6646: the summary counter had no in-tree
        # derivation; the sync script now counts the report's rows.
        sync = load("sync_evidence_counters")
        report = {"rows": [{"realized_code_recorded": True}, {"realized_code_recorded": False},
                           {"nested": [{"realized_code_recorded": True}]}]}
        self.assertEqual(sync.realized_codes_recorded(report), 2)
        sync.SUMMARY = self.root / "summary.json"
        sync.REPRO = self.root / "absent-repro.json"
        sync.THREAT_REPORT = self.root / "threat.json"
        sync.THREAT_REPORT.write_text(json.dumps(report))
        sync.SUMMARY.write_text(json.dumps({"threat_coverage": {"realized_codes_recorded": 25}}))
        self.assertEqual(sync.main(), 0)
        self.assertEqual(json.loads(sync.SUMMARY.read_text())["threat_coverage"]["realized_codes_recorded"], 2)

    def test_the_attestation_chain_is_derived_from_report_history(self) -> None:
        sync = load("sync_evidence_counters")
        versions = [
            {"commits": {"a" * 40: {"hosts": ["primary"]}}},
            {"commits": {"a" * 40: {"hosts": ["primary", "secondary"]}}},
            {"commits": {"b" * 40: {"hosts": ["primary"]}}},
            {"commits": {"a" * 40: {"hosts": ["secondary"]}}},
        ]
        self.assertEqual(
            sync.attestation_chain(versions),
            [
                {"commit": "a" * 40, "hosts": ["primary", "secondary"]},
                {"commit": "b" * 40, "hosts": ["primary"]},
                {"commit": "a" * 40, "hosts": ["secondary"]},
            ],
        )

    def test_summary_mirrors_follow_single_and_two_host_reports(self) -> None:
        sync = load("sync_evidence_counters")
        sync.SUMMARY = self.root / "summary.json"
        sync.REPRO = self.root / "repro.json"
        sync.SUMMARY.write_text(json.dumps({"reproducibility_and_independent_conformance": {}}))
        primary = repro.local_attestation("primary", self.write_evidence())
        secondary = dict(primary, host_label="secondary")
        for attestations in ([primary], [primary, secondary]):
            report = repro.build_report(attestations)
            sync.REPRO.write_text(repro.canonical(report))
            self.assertEqual(sync.main(), 0)
            actual = json.loads(sync.SUMMARY.read_text())["reproducibility_and_independent_conformance"]
            self.assertEqual(actual, {
                "reproducibility_result": report["result"],
                "second_host_status": report["second_host"]["status"],
                "attested_hosts": len(attestations),
                "required_hosts": report["required_hosts"],
                "attested_commit": primary["commit"],
                "blockers": report["blockers"],
            })
        before = sync.SUMMARY.read_bytes()
        sync.REPRO.write_text(repro.canonical(dict(report, report_digest="0" * 64)))
        with self.assertRaises(ValueError):
            sync.main()
        self.assertEqual(sync.SUMMARY.read_bytes(), before)

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
        # entity-read (S20-310 contract) and exec-package-envelope (its only
        # corpus is EXEC_PACKAGE_V2) check their vectors at v2; sibling
        # v2 corpora are digested, summed, and declared without a depth
        # claim (contract revision 5).
        self.assertEqual(
            conformance.CORPUS_VERSION,
            {"entity-read": "v2", "exec-package-envelope": "v2"},
        )
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

    def test_recorded_paths_equal_the_tracked_conformance_set(self) -> None:
        report = conformance.build_report()
        recorded = set()
        for family in report["fixtures"]:
            recorded.add(family["directory"] + "/SHA256SUMS")
            for entry in family["files"]:
                recorded.add(family["directory"] + "/" + entry["name"])
            for version, sibling in family["siblings"].items():
                sibling_dir = f"conformance/{family['directory'].split('/')[1]}/{version}"
                recorded.add(sibling_dir + "/SHA256SUMS")
                for entry in sibling["files"]:
                    recorded.add(sibling_dir + "/" + entry["name"])
        tracked = subprocess.run(
            ["git", "ls-files", "conformance"],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=True,
        ).stdout.split()
        self.assertEqual(sorted(recorded), sorted(tracked))

    def test_a_declared_oracle_script_missing_from_disk_is_coded(self) -> None:
        with self.assertRaises(conformance.ConformanceError) as error:
            conformance.runner_label("python3 scripts/check_no_such_oracle_script.py")
        self.assertEqual(error.exception.code, conformance.ConformanceErrorCode.ORACLE_DRIFT)
        self.assertIn("check_no_such_oracle_script.py", error.exception.detail)

    def test_an_io_failure_in_main_prints_a_coded_object(self) -> None:
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        unwritable = Path(temp.name) / "not-a-directory"
        unwritable.write_text("", encoding="utf-8")
        completed = subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts/build_independent_conformance_report.py"),
                "--output",
                str(unwritable / "report.json"),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 1, completed.stderr)
        payload = json.loads(completed.stdout)
        self.assertEqual(payload["result"], "FAIL")
        self.assertEqual(payload["code"], int(conformance.ConformanceErrorCode.FIXTURE_UNREADABLE))
        self.assertEqual(payload["name"], "FIXTURE_UNREADABLE")

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
    def test_compile_time_embedded_inputs_lie_inside_the_artifact_surface(self) -> None:
        # Every include_str!/include_bytes! target under crates/ that
        # resolves outside crates/ and is not test-only code is a release
        # input: a change to it changes bin/sley, so it must be inside
        # ARTIFACT_INPUT_PATHS or the freshness rule is evadable (Vulcan P2
        # at a809906). Test-only embeds (tests/ directories, *tests.rs
        # modules, and embeds below a #[cfg(test)] attribute) do not reach
        # the binary and are excluded; doc attributes embed READMEs inside
        # crates/ and resolve there.
        surface = candidate_mechanics.ARTIFACT_INPUT_PATHS
        tracked = set(
            subprocess.run(
                ["git", "ls-files"], cwd=ROOT, check=True, capture_output=True, text=True
            ).stdout.split()
        )
        pattern = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"')
        embedded: set[str] = set()
        for source in sorted((ROOT / "crates").rglob("*.rs")):
            relative = source.relative_to(ROOT)
            if "tests" in relative.parts or relative.name.endswith("tests.rs"):
                continue
            test_only_from = None
            for number, line in enumerate(source.read_text(encoding="utf-8").splitlines(), 1):
                if test_only_from is None and "#[cfg(test)]" in line:
                    test_only_from = number
                for match in pattern.finditer(line):
                    if "#![doc" in line or (test_only_from is not None and number > test_only_from):
                        continue
                    target = (source.parent / match.group(1)).resolve()
                    if target.is_relative_to(ROOT / "crates"):
                        continue
                    embedded.add(str(target.relative_to(ROOT)))
        # Positive control: the three known embeds are found, so the scan
        # cannot pass vacuously.
        self.assertIn("docs/spec/SSMC1_EPOCH1_SCHEMA.txt", embedded)
        self.assertIn("conformance/smp1-json-bridge/v2/methods.json", embedded)
        self.assertIn("conformance/smp1-json-bridge/v3/methods.json", embedded)
        for path in sorted(embedded):
            self.assertIn(path, tracked, f"embedded input {path} is not tracked")
            self.assertTrue(
                any(path == entry or path.startswith(entry + "/") for entry in surface),
                f"embedded release input {path} is outside ARTIFACT_INPUT_PATHS",
            )
        for path in candidate_mechanics.EMBEDDED_INPUT_PATHS:
            self.assertIn(path, surface)

    def test_the_release_build_environment_is_scrubbed(self) -> None:
        env = {
            "PATH": "/usr/bin",
            "CARGO_HOME": "/cargo",
            "CC": "gcc",
            "LDFLAGS": "-L/opt",
            "CARGO_ENCODED_RUSTFLAGS": "-Copt-level=1",
            "RUSTC": "/opt/rustc",
            "RUSTC_WRAPPER": "sccache",
            "CARGO_PROFILE_RELEASE_OPT_LEVEL": "1",
            "CARGO_PROFILE_RELEASE_LTO": "off",
            "CARGO_PROFILE_DEV_OPT_LEVEL": "0",
        }
        scrubbed = candidate_mechanics.scrub_build_env(env)
        self.assertEqual(
            scrubbed,
            {"PATH": "/usr/bin", "CARGO_HOME": "/cargo", "CARGO_PROFILE_DEV_OPT_LEVEL": "0"},
        )
        for name in ("CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER"):
            self.assertIn(name, candidate_mechanics.SCRUBBED_BUILD_ENV)
        self.assertIn("CARGO_PROFILE_RELEASE_", candidate_mechanics.SCRUBBED_BUILD_ENV_PREFIXES)

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
