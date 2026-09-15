"""Offline tests of the S20-710 full standards SBOM and unsigned provenance."""

from __future__ import annotations

import importlib.util
import json
import subprocess
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


sbom = load("build_standards_sbom")
provenance = load("build_release_provenance")


def attested_test_candidate() -> dict:
    """A synthetic candidate the real gates admit.

    Repair round 7b test hygiene: the suite never depends on the
    operator tree's untracked runtime evidence (which may be a stale or
    dirty build). The commit and digest come from the tracked
    reproducibility report, so the real HEAD and attestation gates run
    against filed values; git_head is pinned to the attested commit for
    the same reason records-only descendants exist.
    """
    report = json.loads(sbom.REPRO_REPORT.read_text(encoding="utf-8"))
    attested = [
        attestation
        for attestation in report.get("attestations", [])
        if isinstance(attestation, dict)
        and attestation.get("reproducibility") == "REPRODUCIBLE"
        and attestation.get("working_tree_clean") is True
    ]
    assert attested, "no clean REPRODUCIBLE attestation in the tracked report"
    first = attested[0]
    return {
        "artifact_name": first["artifact_name"],
        "artifact_sha256": first["artifact_sha256"],
        "artifact_size_bytes": first["artifact_size_bytes"],
        "commit": first["commit"],
        "manifest_digest": first["manifest_digest"],
        "member_count": first.get("member_count", 14),
        "toolchain": first["toolchain"],
        "working_tree_clean": True,
        "result": "PASS",
        "reproducibility": {"result": "REPRODUCIBLE"},
        "invocation": "build_release_candidate.py --timeout-seconds=900 --require-clean --no-keep",
    }


def patch_candidate(test: unittest.TestCase, *modules) -> dict:
    """Point builder modules at the attested synthetic candidate."""
    candidate = attested_test_candidate()
    for module in modules:
        original_load = module.load_candidate
        module.load_candidate = lambda: dict(candidate)
        test.addCleanup(setattr, module, "load_candidate", original_load)
        original_head = module.git_head
        module.git_head = lambda: candidate["commit"]
        test.addCleanup(setattr, module, "git_head", original_head)
    return candidate


class StandardsSbomTests(unittest.TestCase):
    def setUp(self) -> None:
        patch_candidate(self, sbom)
        self.cyclonedx, self.spdx, self.counts = sbom.build_documents()

    def test_documents_refuse_a_candidate_no_attestation_names(self) -> None:
        original = sbom.load_candidate
        sbom.load_candidate = lambda: {
            "commit": "0" * 40,
            "artifact_sha256": "f" * 64,
            "artifact_name": "sley-test.tar.gz",
        }
        self.addCleanup(setattr, sbom, "load_candidate", original)
        with self.assertRaises(sbom.SbomError) as error:
            sbom.build_documents()
        self.assertEqual(error.exception.code, sbom.SbomErrorCode.INVENTORY_INVALID)

    def test_documents_refuse_a_non_pass_record(self) -> None:
        candidate = attested_test_candidate()
        candidate["result"] = "FAIL"
        original = sbom.load_candidate
        sbom.load_candidate = lambda: candidate
        self.addCleanup(setattr, sbom, "load_candidate", original)
        with self.assertRaises(sbom.SbomError) as error:
            sbom.build_documents()
        self.assertEqual(error.exception.code, sbom.SbomErrorCode.INVENTORY_INVALID)

    def test_documents_refuse_a_manifest_size_mismatch(self) -> None:
        candidate = attested_test_candidate()
        candidate["artifact_size_bytes"] += 1
        original = sbom.load_candidate
        sbom.load_candidate = lambda: candidate
        self.addCleanup(setattr, sbom, "load_candidate", original)
        with self.assertRaises(sbom.SbomError) as error:
            sbom.build_documents()
        self.assertEqual(error.exception.code, sbom.SbomErrorCode.INVENTORY_INVALID)

    def test_documents_refuse_a_manifest_digest_mismatch(self) -> None:
        candidate = attested_test_candidate()
        candidate["manifest_digest"] = "e" * 64
        original = sbom.load_candidate
        sbom.load_candidate = lambda: candidate
        self.addCleanup(setattr, sbom, "load_candidate", original)
        with self.assertRaises(sbom.SbomError) as error:
            sbom.build_documents()
        self.assertEqual(error.exception.code, sbom.SbomErrorCode.INVENTORY_INVALID)

    def test_cyclonedx_is_a_1_6_bom_with_a_derived_serial_number(self) -> None:
        self.assertEqual(self.cyclonedx["bomFormat"], "CycloneDX")
        self.assertEqual(self.cyclonedx["specVersion"], "1.6")
        serial = self.cyclonedx["serialNumber"]
        self.assertTrue(serial.startswith("urn:uuid:"))
        uuid = serial.removeprefix("urn:uuid:")
        self.assertEqual([len(part) for part in uuid.split("-")], [8, 4, 4, 4, 12])
        self.assertEqual(uuid[14], "8")
        self.assertEqual(uuid[19], "8")
        # The serial is a function of the rest of the document.
        without = {key: value for key, value in self.cyclonedx.items() if key != "serialNumber"}
        self.assertEqual(serial, f"urn:uuid:{sbom.derived_uuid(sbom.digest_of(without))}")

    def test_every_component_carries_a_purl_version_and_license(self) -> None:
        for component in self.cyclonedx["components"]:
            self.assertTrue(component["purl"])
            self.assertTrue(component["version"])
            self.assertEqual(len(component["licenses"]), 1)
            self.assertTrue(component["licenses"][0]["expression"])
        self.assertEqual(len(self.cyclonedx["components"]), self.counts["components"])

    def test_multi_artifact_components_record_a_count_instead_of_a_hash(self) -> None:
        multi = [
            component
            for component in self.cyclonedx["components"]
            if any(
                property["name"] == "sley2:locked-artifact-digests"
                for property in component["properties"]
            )
        ]
        self.assertTrue(multi, "the Python dependencies lock several platform wheels")
        for component in multi:
            self.assertNotIn("hashes", component)

    def test_spdx_is_2_3_with_a_fixed_instant_and_an_extracted_apache_reference(self) -> None:
        self.assertEqual(self.spdx["spdxVersion"], "SPDX-2.3")
        self.assertEqual(self.spdx["dataLicense"], "CC0-1.0")
        self.assertEqual(self.spdx["creationInfo"]["created"], "1970-01-01T00:00:00Z")
        self.assertTrue(self.spdx["documentNamespace"].startswith("urn:sley2:spdx:"))
        extracted = self.spdx["hasExtractedLicensingInfos"]
        self.assertEqual([entry["licenseId"] for entry in extracted], [sbom.ROOT_LICENSE])
        for package in self.spdx["packages"]:
            self.assertEqual(package["licenseConcluded"], "NOASSERTION")
            self.assertEqual(package["copyrightText"], "NOASSERTION")

    def test_spdx_describes_the_candidate_and_mirrors_the_dependency_graph(self) -> None:
        describes = [
            relationship
            for relationship in self.spdx["relationships"]
            if relationship["relationshipType"] == "DESCRIBES"
        ]
        self.assertEqual(len(describes), 1)
        depends = [
            relationship
            for relationship in self.spdx["relationships"]
            if relationship["relationshipType"] == "DEPENDS_ON"
        ]
        self.assertEqual(len(depends), self.counts["relationships"])

    def test_no_document_carries_a_host_path_or_wall_clock(self) -> None:
        for document in (self.cyclonedx, self.spdx):
            text = sbom.canonical(document)
            for marker in ("/home/", "greyforge", "file://"):
                self.assertNotIn(marker, text)

    def test_incomplete_components_fail_closed(self) -> None:
        for missing in ("bom_ref", "name", "version", "ecosystem", "license_declared"):
            package = {
                "bom_ref": "pkg:cargo/x@1",
                "name": "x",
                "version": "1",
                "ecosystem": "cargo",
                "license_declared": "MIT",
            }
            del package[missing]
            with self.assertRaises(sbom.SbomError) as error:
                sbom.component_facts(package)
            self.assertEqual(error.exception.code, sbom.SbomErrorCode.COMPONENT_INCOMPLETE)

    def test_documents_are_a_pure_function_of_the_evidence(self) -> None:
        again_bom, again_spdx, _ = sbom.build_documents()
        self.assertEqual(self.cyclonedx, again_bom)
        self.assertEqual(self.spdx, again_spdx)


class LicenseExpressionTests(unittest.TestCase):
    """The section 2 normalization and grammar rule: pure functions, no evidence."""

    def test_slash_separator_normalizes_to_or(self) -> None:
        self.assertEqual(sbom.normalize_license("MIT/Apache-2.0"), "MIT OR Apache-2.0")

    def test_plain_and_approved_declarations_pass_through(self) -> None:
        self.assertEqual(sbom.normalize_license("MIT OR Apache-2.0"), "MIT OR Apache-2.0")
        self.assertEqual(sbom.normalize_license(sbom.ROOT_LICENSE), sbom.ROOT_LICENSE)

    def test_empty_slash_part_fails_closed(self) -> None:
        for bad in ("MIT/", "/MIT", "MIT//Apache-2.0"):
            with self.assertRaises(sbom.SbomError) as error:
                sbom.normalize_license(bad)
            self.assertEqual(error.exception.code, sbom.SbomErrorCode.COMPONENT_INCOMPLETE)

    def test_every_inventory_expression_is_usable(self) -> None:
        inventory = json.loads(sbom.INVENTORY.read_text(encoding="utf-8"))
        self.assertTrue(inventory["packages"])
        for package in inventory["packages"]:
            normalized = sbom.normalize_license(package["license_declared"])
            self.assertTrue(
                sbom.valid_spdx_expression(normalized),
                f"{package['bom_ref']} declares {package['license_declared']!r}",
            )

    def test_grammar_rejects_non_expressions(self) -> None:
        for bad in (
            "",
            "MIT/Apache-2.0",
            "MIT OR",
            "OR MIT",
            "(MIT",
            "MIT)",
            "()",
            "MIT or Apache-2.0",
            "MIT WITH",
            "MIT WITH (Apache-2.0)",
            "MIT AND OR Apache-2.0",
        ):
            self.assertFalse(sbom.valid_spdx_expression(bad), bad)

    def test_grammar_accepts_with_parens_and_plus(self) -> None:
        for good in (
            "Apache-2.0",
            "MIT OR Apache-2.0",
            "Apache-2.0 WITH LLVM-exception",
            "(MIT OR Apache-2.0) AND Unicode-3.0",
            sbom.ROOT_LICENSE,
            "GPL-2.0+",
        ):
            self.assertTrue(sbom.valid_spdx_expression(good), good)

    def test_component_facts_normalizes_and_rejects(self) -> None:
        package = {
            "bom_ref": "pkg:cargo/unicode-normalization@0.1.24",
            "name": "unicode-normalization",
            "version": "0.1.24",
            "ecosystem": "cargo",
            "license_declared": "MIT/Apache-2.0",
        }
        self.assertEqual(sbom.component_facts(package)["license"], "MIT OR Apache-2.0")
        package["license_declared"] = "MIT/"
        with self.assertRaises(sbom.SbomError) as error:
            sbom.component_facts(package)
        self.assertEqual(error.exception.code, sbom.SbomErrorCode.COMPONENT_INCOMPLETE)


class SpdxNamespaceTests(unittest.TestCase):
    """The section 3 candidate binding: synthetic candidates, no evidence."""

    FACTS = [
        {
            "purl": "pkg:cargo/x@1",
            "name": "x",
            "version": "1",
            "ecosystem": "cargo",
            "license": "MIT",
            "disposition": "OK",
            "source": "registry+https://example.invalid/x",
            "workspace": False,
            "single_digest": "a" * 64,
            "locked_artifact_digests": None,
        }
    ]

    def document(self, artifact_sha256: str) -> dict:
        return sbom.spdx(
            self.FACTS,
            [],
            {"artifact_name": "sley-test.tar.gz", "artifact_sha256": artifact_sha256},
            "b" * 64,
        )

    def test_namespace_binds_inventory_and_candidate(self) -> None:
        namespace = self.document("c" * 64)["documentNamespace"]
        self.assertEqual(namespace, f"urn:sley2:spdx:{'b' * 64}:{'c' * 64}")

    def test_same_inventory_with_another_candidate_is_another_document(self) -> None:
        first = self.document("c" * 64)["documentNamespace"]
        second = self.document("d" * 64)["documentNamespace"]
        self.assertNotEqual(first, second)

    def test_namespace_is_deterministic(self) -> None:
        self.assertEqual(
            self.document("c" * 64)["documentNamespace"],
            self.document("c" * 64)["documentNamespace"],
        )


class CheckSemanticsTests(unittest.TestCase):
    """The section 5 fail-closed rule: missing evidence is missing input."""

    def test_missing_sbom_evidence_is_not_ahead(self) -> None:
        original = sbom.load_candidate

        def missing() -> dict:
            raise sbom.SbomError(sbom.SbomErrorCode.INVENTORY_MISSING, "gone")

        sbom.load_candidate = missing
        self.addCleanup(setattr, sbom, "load_candidate", original)
        self.assertFalse(sbom.candidate_evidence_mismatch())

    def test_missing_provenance_evidence_is_not_ahead(self) -> None:
        original = provenance.load_candidate

        def missing() -> dict:
            raise provenance.ProvenanceError(
                provenance.ProvenanceErrorCode.EVIDENCE_MISSING, "gone"
            )

        provenance.load_candidate = missing
        self.addCleanup(setattr, provenance, "load_candidate", original)
        self.assertFalse(provenance.candidate_evidence_mismatch())

    def test_differing_evidence_is_ahead(self) -> None:
        sbom_original = sbom.load_candidate
        provenance_original = provenance.load_candidate
        sbom.load_candidate = lambda: {"commit": "0" * 40, "artifact_sha256": "0" * 64}
        provenance.load_candidate = lambda: {
            "commit": "0" * 40,
            "artifact_sha256": "0" * 64,
        }
        self.addCleanup(setattr, sbom, "load_candidate", sbom_original)
        self.addCleanup(setattr, provenance, "load_candidate", provenance_original)
        self.assertTrue(sbom.candidate_evidence_mismatch())
        self.assertTrue(provenance.candidate_evidence_mismatch())

    def test_matching_evidence_is_not_ahead(self) -> None:
        commit, digest = sbom.tracked_candidate_facts()
        sbom_original = sbom.load_candidate
        provenance_original = provenance.load_candidate
        sbom.load_candidate = lambda: {"commit": commit, "artifact_sha256": digest}
        provenance.load_candidate = lambda: {"commit": commit, "artifact_sha256": digest}
        self.addCleanup(setattr, sbom, "load_candidate", sbom_original)
        self.addCleanup(setattr, provenance, "load_candidate", provenance_original)
        self.assertFalse(sbom.candidate_evidence_mismatch())
        self.assertFalse(provenance.candidate_evidence_mismatch())


class ProvenanceTests(unittest.TestCase):
    def setUp(self) -> None:
        # The statement is derived against the *derived* CycloneDX document, so
        # the suite never depends on whether the tracked documents have caught
        # up with an untracked local candidate build. The subject-authority
        # gates run for real against the tracked attestation set; only
        # git_head is pinned to the attested commit (records-only
        # descendants move HEAD past the mint by design).
        patch_candidate(self, sbom, provenance)
        derived_root = sbom.build_documents()[0]["metadata"]["component"]["hashes"][0]["content"]
        self.original_root = provenance.sbom_root_digest
        provenance.sbom_root_digest = lambda: derived_root
        self.addCleanup(setattr, provenance, "sbom_root_digest", self.original_root)
        self.document = provenance.build_file()
        self.statement = self.document["statement"]

    def test_a_candidate_without_invocation_refuses(self) -> None:
        candidate = attested_test_candidate()
        del candidate["invocation"]
        original = provenance.load_candidate
        provenance.load_candidate = lambda: candidate
        self.addCleanup(setattr, provenance, "load_candidate", original)
        with self.assertRaises(provenance.ProvenanceError) as error:
            provenance.build_statement()
        self.assertEqual(
            error.exception.code, provenance.ProvenanceErrorCode.EVIDENCE_INVALID
        )

    def test_a_dirty_candidate_refuses_at_derivation(self) -> None:
        candidate = attested_test_candidate()
        candidate["working_tree_clean"] = False
        original = provenance.load_candidate
        provenance.load_candidate = lambda: candidate
        self.addCleanup(setattr, provenance, "load_candidate", original)
        with self.assertRaises(provenance.ProvenanceError) as error:
            provenance.build_statement()
        self.assertEqual(
            error.exception.code, provenance.ProvenanceErrorCode.EVIDENCE_INVALID
        )

    def test_make_target_derives_from_the_recorded_invocation(self) -> None:
        external = self.statement["predicate"]["buildDefinition"]["externalParameters"]
        self.assertEqual(external["make_target"], "release-candidate-smoke")
        candidate = attested_test_candidate()
        candidate["invocation"] = "build_release_candidate.py --timeout-seconds=60 --require-clean --no-keep"
        original = provenance.load_candidate
        provenance.load_candidate = lambda: candidate
        self.addCleanup(setattr, provenance, "load_candidate", original)
        statement = provenance.build_statement()
        direct = statement["predicate"]["buildDefinition"]["externalParameters"]
        self.assertEqual(direct["make_target"], "build_release_candidate.py direct")

    def test_statement_is_in_toto_v1_with_a_slsa_predicate(self) -> None:
        self.assertEqual(self.statement["_type"], provenance.STATEMENT_TYPE)
        self.assertEqual(self.statement["predicateType"], provenance.PREDICATE_TYPE)
        self.assertEqual(len(self.statement["subject"]), 1)
        self.assertEqual(
            self.statement["predicate"]["buildDefinition"]["buildType"], provenance.BUILD_TYPE
        )
        self.assertEqual(
            self.statement["predicate"]["runDetails"]["builder"]["id"], provenance.BUILDER_ID
        )

    def test_the_subject_agrees_with_the_candidate_and_the_sbom_root(self) -> None:
        candidate = attested_test_candidate()
        self.assertEqual(
            self.statement["subject"][0]["digest"]["sha256"], candidate["artifact_sha256"]
        )
        self.assertEqual(provenance.sbom_root_digest(), candidate["artifact_sha256"])

    def test_a_disagreeing_sbom_root_fails_closed(self) -> None:
        provenance.sbom_root_digest = lambda: "f" * 64
        with self.assertRaises(provenance.ProvenanceError) as error:
            provenance.build_statement()
        self.assertEqual(
            error.exception.code, provenance.ProvenanceErrorCode.SUBJECT_MISMATCH
        )

    def test_resolved_dependencies_cover_the_locks_inventory_and_both_sboms(self) -> None:
        names = {
            entry.get("name", entry.get("uri"))
            for entry in self.statement["predicate"]["buildDefinition"]["resolvedDependencies"]
        }
        self.assertEqual(
            names,
            {
                "urn:sley2:git-commit",
                "Cargo.lock",
                "oracle/scb1/uv.lock",
                "evidence/security/T52/pre-release-inventory.json",
                "evidence/release/sbom/cyclonedx-1.6.json",
                "evidence/release/sbom/spdx-2.3.json",
            },
        )

    def test_the_attestation_records_that_nothing_is_signed_or_published(self) -> None:
        attestation = self.document["attestation"]
        self.assertFalse(attestation["signed"])
        self.assertIsNone(attestation["signature_algorithm"])
        self.assertIsNone(attestation["transparency_log"])
        self.assertFalse(attestation["publication_authorized"])
        self.assertIn("final_argus_and_vulcan_dispositions", attestation["blockers"])
        self.assertIn("second_host_attestation_operator_lane", attestation["blockers"])
        self.assertNotIn("root_license_text_operator_approval", attestation["blockers"])
        self.assertEqual(
            self.document["statement_digest"], provenance.digest_of(self.statement)
        )

    def test_no_timestamp_or_host_path_appears(self) -> None:
        text = provenance.canonical(self.document)
        for marker in ("/home/", "greyforge", "startedOn", "finishedOn", "file://"):
            self.assertNotIn(marker, text)

    def test_the_statement_is_a_pure_function_of_the_evidence(self) -> None:
        self.assertEqual(self.document, provenance.build_file())

    def test_a_candidate_from_another_commit_refuses(self) -> None:
        # Amended contract (revision 5): a commit behind HEAD is admitted
        # only as a provable records-closure, so this refusal case names a
        # commit no tree contains and the closure check fails closed.
        candidate = attested_test_candidate()
        candidate["commit"] = "0" * 40
        original = provenance.load_candidate
        provenance.load_candidate = lambda: candidate
        self.addCleanup(setattr, provenance, "load_candidate", original)
        provenance.git_head = lambda: "1" * 40
        with self.assertRaises(provenance.ProvenanceError) as error:
            provenance.build_statement()
        self.assertEqual(
            error.exception.code, provenance.ProvenanceErrorCode.EVIDENCE_INVALID
        )

    def test_an_unattested_candidate_refuses(self) -> None:
        original = provenance.attested_candidates
        provenance.attested_candidates = lambda: []
        self.addCleanup(setattr, provenance, "attested_candidates", original)
        with self.assertRaises(provenance.ProvenanceError) as error:
            provenance.build_statement()
        self.assertEqual(
            error.exception.code, provenance.ProvenanceErrorCode.SUBJECT_MISMATCH
        )

    def test_the_invocation_is_copied_from_the_candidate_evidence(self) -> None:
        candidate = attested_test_candidate()
        candidate["invocation"] = "build_release_candidate.py --require-clean (test)"
        original = provenance.load_candidate
        provenance.load_candidate = lambda: candidate
        self.addCleanup(setattr, provenance, "load_candidate", original)
        statement = provenance.build_statement()
        external = statement["predicate"]["buildDefinition"]["externalParameters"]
        self.assertEqual(
            external["invocation"], "build_release_candidate.py --require-clean (test)"
        )
        self.assertEqual(external["make_target"], "build_release_candidate.py direct")


class ValidateTrackedTests(unittest.TestCase):
    """The mismatch-state rule: tracked documents are verified, not skipped.

    Repair round 7b test hygiene: these cases rewrite the tracked
    documents, so they run against private copies under a temporary
    directory. A killed run can no longer leave the tree modified.
    """

    def setUp(self) -> None:
        self.work = tempfile.TemporaryDirectory()
        self.addCleanup(self.work.cleanup)
        self.paths: dict[str, Path] = {}
        for module, name in (
            (sbom, "CYCLONEDX"),
            (sbom, "SPDX"),
            (provenance, "PROVENANCE"),
        ):
            original: Path = getattr(module, name)
            copy = Path(self.work.name) / original.name
            copy.write_text(original.read_text(encoding="utf-8"), encoding="utf-8")
            setattr(module, name, copy)
            self.addCleanup(setattr, module, name, original)
            self.paths[f"{module.__name__}.{name}"] = copy

    def tracked(self, key: str) -> dict:
        return json.loads(self.paths[key].read_text(encoding="utf-8"))

    def rewrite(self, key: str, document: dict) -> None:
        module_name, _, attr = key.partition(".")
        module = {"build_standards_sbom": sbom, "build_release_provenance": provenance}[
            module_name
        ]
        self.paths[key].write_text(module.canonical(document), encoding="utf-8")

    def test_the_current_pair_validates(self) -> None:
        self.assertEqual(sbom.validate_tracked(), [])
        self.assertEqual(provenance.validate_tracked(), [])

    def test_a_rewritten_namespace_fails_the_sbom_validation(self) -> None:
        document = self.tracked("build_standards_sbom.SPDX")
        document["documentNamespace"] = "urn:sley2:spdx:tampered"
        self.rewrite("build_standards_sbom.SPDX", document)
        self.assertIn("namespace-not-attestation-bound", sbom.validate_tracked())

    def test_a_rewritten_subject_fails_the_provenance_binding(self) -> None:
        # The digest is recomputed so only the attestation binding can
        # fail: this isolates subject-not-attestation-bound from
        # statement-digest.
        document = self.tracked("build_release_provenance.PROVENANCE")
        document["statement"]["subject"][0]["digest"]["sha256"] = "f" * 64
        document["statement_digest"] = provenance.digest_of(document["statement"])
        self.rewrite("build_release_provenance.PROVENANCE", document)
        self.assertEqual(
            provenance.validate_tracked(), ["subject-not-attestation-bound"]
        )

    def test_the_real_attestation_filter_carries_the_filed_candidate(self) -> None:
        # Positive control: the unpatched attested_candidates() admits the
        # tracked (commit, subject). Membership (not equality) so a future
        # dirty filing exercises the filter instead of breaking the control.
        admitted = {
            (attestation.get("commit"), attestation.get("artifact_sha256"))
            for attestation in provenance.attested_candidates()
        }
        report = json.loads(sbom.REPRO_REPORT.read_text(encoding="utf-8"))
        filed = [
            (attestation.get("commit"), attestation.get("artifact_sha256"))
            for attestation in report.get("attestations", [])
            if attestation.get("reproducibility") == "REPRODUCIBLE"
            and attestation.get("working_tree_clean") is True
        ]
        self.assertTrue(filed)
        for pair in filed:
            self.assertIn(pair, admitted)

    def test_admission_predicate_directly(self) -> None:
        self.assertTrue(
            provenance.is_admittable(
                {"reproducibility": "REPRODUCIBLE", "working_tree_clean": True}
            )
        )
        for denied in (
            {"reproducibility": "REPRODUCIBLE", "working_tree_clean": False},
            {"reproducibility": "DIFFERS", "working_tree_clean": True},
            {"reproducibility": "REPRODUCIBLE"},
            {},
            None,
            [],
            "REPRODUCIBLE",
        ):
            self.assertFalse(provenance.is_admittable(denied), denied)

    def test_unattested_pair_is_not_admitted(self) -> None:
        # Negative control: removing the filter must fail this test, so a
        # filter deletion cannot pass silently.
        admitted = {
            (attestation.get("commit"), attestation.get("artifact_sha256"))
            for attestation in provenance.attested_candidates()
        }
        self.assertNotIn(("0" * 40, "f" * 64), admitted)


closure = load("records_closure")


def stub_closure(test: unittest.TestCase, status: "closure.ClosureStatus"):
    """Point both builders at one synthetic closure verdict."""
    module = sbom.records_closure
    original = module.closure_status
    module.closure_status = lambda attested: status
    test.addCleanup(setattr, module, "closure_status", original)
    return module


class RecordsClosureTests(unittest.TestCase):
    def test_builders_admit_exactly_the_live_closure_verdict(self) -> None:
        # Live coupling, state-independent: whatever the real tree says
        # about the attested candidate, both builders follow it. A tree
        # past a records-closure admits byte-identical derivation; any
        # other advanced tree refuses closed with the closure reason.
        # (A fixed is_closure assertion here would rot: the amendment
        # commit itself carries attestation-bound changes, so no live
        # checkout past it is a records-closure by design. git_head
        # stays real here: patch_candidate would pin it to the candidate
        # commit and bypass the closure path under test.)
        candidate = attested_test_candidate()
        for module in (sbom, provenance):
            original_load = module.load_candidate
            module.load_candidate = lambda current=candidate: dict(current)
            self.addCleanup(setattr, module, "load_candidate", original_load)
        status = closure.closure_status(candidate["commit"])
        if status.is_closure:
            sbom.build_documents()
            provenance.build_statement()
        elif (
            status.reason == "records-closure-not-advanced"
            and candidate["commit"] == sbom.git_head()
        ):
            # Mint moment: HEAD is the attested commit, so both builders
            # admit via the candidate==HEAD path (builder-contract
            # condition (a)), not the closure path. Pin admission here;
            # any other non-closure verdict must still refuse below.
            sbom.build_documents()
            provenance.build_statement()
        else:
            with self.assertRaises(sbom.SbomError) as sbom_error:
                sbom.build_documents()
            self.assertIn("records-closure-", sbom_error.exception.detail)
            with self.assertRaises(provenance.ProvenanceError) as provenance_error:
                provenance.build_statement()
            self.assertIn("records-closure-", provenance_error.exception.detail)

    def test_attestation_bound_change_is_ineligible(self) -> None:
        candidate = attested_test_candidate()
        status = closure.ClosureStatus(
            attested_commit=candidate["commit"],
            head="b" * 40,
            changed=["scripts/build_standards_sbom.py"],
            ineligible=["scripts/build_standards_sbom.py"],
        )
        self.assertFalse(status.is_closure)
        self.assertIn("records-closure-ineligible", status.reason)

    def test_bound_input_change_refuses(self) -> None:
        candidate = attested_test_candidate()
        status = closure.ClosureStatus(
            attested_commit=candidate["commit"],
            head="b" * 40,
            changed=["evidence/security/T52/pre-release-inventory.json"],
            bound_changed=["evidence/security/T52/pre-release-inventory.json"],
        )
        self.assertFalse(status.is_closure)
        self.assertIn("records-closure-bound-changed", status.reason)

    def test_unverifiable_git_refuses_closed(self) -> None:
        status = closure.closure_status("0" * 40)
        self.assertFalse(status.is_closure)
        self.assertIn("records-closure-unverifiable", status.reason)

    def test_builders_admit_an_eligible_closure_keeping_the_candidate_binding(self) -> None:
        # Compares the candidate-bound properties, subject, and SPDX version of
        # the admitted documents; byte identity of re-derived documents is the
        # drift tests' claim, not this one's (Vulcan P4, 2026-09-13).
        candidate = patch_candidate(self, sbom, provenance)
        descendant = "b" * 40
        sbom.git_head = lambda: descendant
        provenance.git_head = lambda: descendant
        stub_closure(
            self,
            closure.ClosureStatus(
                attested_commit=candidate["commit"],
                head=descendant,
                changed=["evidence/review/finding-register.json"],
            ),
        )
        expected_cyclonedx, expected_spdx, _ = sbom.build_documents()
        expected_statement = provenance.build_statement()
        self.assertEqual(
            expected_cyclonedx["metadata"]["properties"],
            sbom.build_documents()[0]["metadata"]["properties"],
        )
        self.assertEqual(
            expected_statement["subject"], provenance.build_statement()["subject"]
        )
        self.assertEqual(expected_spdx["spdxVersion"], "SPDX-2.3")

    def test_builders_refuse_an_ineligible_closure(self) -> None:
        candidate = patch_candidate(self, sbom, provenance)
        descendant = "b" * 40
        sbom.git_head = lambda: descendant
        provenance.git_head = lambda: descendant
        stub_closure(
            self,
            closure.ClosureStatus(
                attested_commit=candidate["commit"],
                head=descendant,
                changed=["crates/smp1/src/lib.rs"],
                ineligible=["crates/smp1/src/lib.rs"],
            ),
        )
        with self.assertRaises(sbom.SbomError) as sbom_error:
            sbom.build_documents()
        self.assertEqual(sbom_error.exception.code, sbom.SbomErrorCode.INVENTORY_INVALID)
        self.assertIn("records-closure-ineligible", sbom_error.exception.detail)
        with self.assertRaises(provenance.ProvenanceError) as provenance_error:
            provenance.build_statement()
        self.assertEqual(
            provenance_error.exception.code,
            provenance.ProvenanceErrorCode.EVIDENCE_INVALID,
        )
        self.assertIn("records-closure-ineligible", provenance_error.exception.detail)

    def test_builders_refuse_a_bound_artifact_change(self) -> None:
        candidate = patch_candidate(self, sbom, provenance)
        descendant = "c" * 40
        sbom.git_head = lambda: descendant
        provenance.git_head = lambda: descendant
        stub_closure(
            self,
            closure.ClosureStatus(
                attested_commit=candidate["commit"],
                head=descendant,
                changed=["evidence/security/T52/pre-release-inventory.json"],
                bound_changed=["evidence/security/T52/pre-release-inventory.json"],
            ),
        )
        with self.assertRaises(sbom.SbomError) as sbom_error:
            sbom.build_documents()
        self.assertIn("records-closure-bound-changed", sbom_error.exception.detail)
        with self.assertRaises(provenance.ProvenanceError):
            provenance.build_statement()


checker = load("check_standards_sbom_and_provenance")


class CheckerFoldTests(unittest.TestCase):
    """The checker folds only the builders' own closure refusals (Vulcan P3, 2026-09-15)."""

    def test_closure_refusal_is_named_by_the_builder_detail(self) -> None:
        stdout = json.dumps(
            {
                "result": "FAIL",
                "code": 4,
                "name": "INVENTORY_INVALID",
                "detail": "candidate commit abc is not HEAD; records-closure-ineligible: scripts/x.py; rebuild",
            }
        )
        self.assertEqual(checker.builder_refusal_label("sbom", stdout), "sbom:closure-refusal")

    def test_tracked_invalid_and_drift_keep_their_own_labels(self) -> None:
        invalid = json.dumps(
            {"mode": "check", "result": "FAIL", "state": "MISMATCH_TRACKED_INVALID", "problems": ["x"]}
        )
        drift = json.dumps(
            {"mode": "check", "result": "FAIL", "code": 7, "name": "DOCUMENT_DRIFT", "detail": "differs"}
        )
        self.assertEqual(checker.builder_refusal_label("provenance", invalid), "provenance:tracked-invalid")
        self.assertEqual(checker.builder_refusal_label("provenance", drift), "provenance:drift")
        self.assertEqual(checker.builder_refusal_label("sbom", ""), "sbom:drift")
        self.assertEqual(checker.builder_refusal_label("sbom", "not json"), "sbom:drift")

    def test_ineligible_closure_folds_only_closure_refusals(self) -> None:
        problems = ["sbom:closure-refusal", "provenance:tracked-invalid", "provenance:drift"]
        self.assertEqual(
            checker.fold_closure_refusals(problems, closure_ineligible=True),
            ["provenance:tracked-invalid", "provenance:drift"],
        )
        self.assertEqual(
            checker.fold_closure_refusals(["sbom:closure-refusal", "provenance:closure-refusal"], True),
            [],
        )

    def test_eligible_closure_folds_nothing(self) -> None:
        problems = ["sbom:closure-refusal", "provenance:tracked-invalid"]
        self.assertEqual(
            checker.fold_closure_refusals(problems, closure_ineligible=False),
            ["sbom:drift", "provenance:tracked-invalid"],
        )


if __name__ == "__main__":
    unittest.main()
