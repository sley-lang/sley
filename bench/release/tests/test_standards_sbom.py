"""Offline tests of the S20-710 full standards SBOM and unsigned provenance."""

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


sbom = load("build_standards_sbom")
provenance = load("build_release_provenance")


class StandardsSbomTests(unittest.TestCase):
    def setUp(self) -> None:
        self.cyclonedx, self.spdx, self.counts = sbom.build_documents()

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

    def test_spdx_is_2_3_with_a_fixed_instant_and_an_extracted_proprietary_reference(self) -> None:
        self.assertEqual(self.spdx["spdxVersion"], "SPDX-2.3")
        self.assertEqual(self.spdx["dataLicense"], "CC0-1.0")
        self.assertEqual(self.spdx["creationInfo"]["created"], "1970-01-01T00:00:00Z")
        self.assertTrue(self.spdx["documentNamespace"].startswith("urn:sley2:spdx:"))
        extracted = self.spdx["hasExtractedLicensingInfos"]
        self.assertEqual([entry["licenseId"] for entry in extracted], [sbom.PROPRIETARY])
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

    def test_plain_and_proprietary_declarations_pass_through(self) -> None:
        self.assertEqual(sbom.normalize_license("MIT OR Apache-2.0"), "MIT OR Apache-2.0")
        self.assertEqual(sbom.normalize_license(sbom.PROPRIETARY), sbom.PROPRIETARY)

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
            sbom.PROPRIETARY,
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
        self.assertFalse(sbom.local_build_ahead())

    def test_missing_provenance_evidence_is_not_ahead(self) -> None:
        original = provenance.load_candidate

        def missing() -> dict:
            raise provenance.ProvenanceError(
                provenance.ProvenanceErrorCode.EVIDENCE_MISSING, "gone"
            )

        provenance.load_candidate = missing
        self.addCleanup(setattr, provenance, "load_candidate", original)
        self.assertFalse(provenance.local_build_ahead())

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
        self.assertTrue(sbom.local_build_ahead())
        self.assertTrue(provenance.local_build_ahead())

    def test_matching_evidence_is_not_ahead(self) -> None:
        commit, digest = sbom.tracked_candidate_facts()
        sbom_original = sbom.load_candidate
        provenance_original = provenance.load_candidate
        sbom.load_candidate = lambda: {"commit": commit, "artifact_sha256": digest}
        provenance.load_candidate = lambda: {"commit": commit, "artifact_sha256": digest}
        self.addCleanup(setattr, sbom, "load_candidate", sbom_original)
        self.addCleanup(setattr, provenance, "load_candidate", provenance_original)
        self.assertFalse(sbom.local_build_ahead())
        self.assertFalse(provenance.local_build_ahead())


class ProvenanceTests(unittest.TestCase):
    def setUp(self) -> None:
        # The statement is derived against the *derived* CycloneDX document, so
        # the suite never depends on whether the tracked documents have caught
        # up with an untracked local candidate build. The subject-authority
        # gates are satisfied with the live candidate: HEAD coherence reads
        # the candidate's own commit and the attested set carries it.
        derived_root = sbom.build_documents()[0]["metadata"]["component"]["hashes"][0]["content"]
        self.original_root = provenance.sbom_root_digest
        provenance.sbom_root_digest = lambda: derived_root
        self.addCleanup(setattr, provenance, "sbom_root_digest", self.original_root)
        candidate = json.loads(provenance.CANDIDATE.read_text(encoding="utf-8"))
        self.original_head = provenance.git_head
        provenance.git_head = lambda: candidate["commit"]
        self.addCleanup(setattr, provenance, "git_head", self.original_head)
        self.original_attested = provenance.attested_candidates
        provenance.attested_candidates = lambda: [
            {
                "commit": candidate["commit"],
                "artifact_sha256": candidate["artifact_sha256"],
                "reproducibility": "REPRODUCIBLE",
                "working_tree_clean": True,
            }
        ]
        self.addCleanup(setattr, provenance, "attested_candidates", self.original_attested)
        self.document = provenance.build_file()
        self.statement = self.document["statement"]

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
        candidate = json.loads(provenance.CANDIDATE.read_text(encoding="utf-8"))
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
        self.assertIn("root_license_text_operator_approval", attestation["blockers"])
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
        provenance.git_head = lambda: "1" * 40
        with self.assertRaises(provenance.ProvenanceError) as error:
            provenance.build_statement()
        self.assertEqual(
            error.exception.code, provenance.ProvenanceErrorCode.EVIDENCE_INVALID
        )

    def test_an_unattested_candidate_refuses(self) -> None:
        provenance.attested_candidates = lambda: []
        with self.assertRaises(provenance.ProvenanceError) as error:
            provenance.build_statement()
        self.assertEqual(
            error.exception.code, provenance.ProvenanceErrorCode.SUBJECT_MISMATCH
        )

    def test_the_invocation_is_copied_from_the_candidate_evidence(self) -> None:
        candidate = json.loads(provenance.CANDIDATE.read_text(encoding="utf-8"))
        candidate["invocation"] = "build_release_candidate.py --require-clean (test)"
        original = provenance.load_candidate
        provenance.load_candidate = lambda: candidate
        self.addCleanup(setattr, provenance, "load_candidate", original)
        statement = provenance.build_statement()
        external = statement["predicate"]["buildDefinition"]["externalParameters"]
        self.assertEqual(
            external["invocation"], "build_release_candidate.py --require-clean (test)"
        )


class ValidateTrackedTests(unittest.TestCase):
    """The ahead-state rule: tracked documents are verified, not skipped."""

    def test_the_current_pair_validates(self) -> None:
        self.assertEqual(sbom.validate_tracked(), [])
        self.assertEqual(provenance.validate_tracked(), [])

    def test_a_rewritten_namespace_fails_the_sbom_validation(self) -> None:
        original = sbom.SPDX.read_text(encoding="utf-8")
        document = json.loads(original)
        document["documentNamespace"] = "urn:sley2:spdx:tampered"
        sbom.SPDX.write_text(sbom.canonical(document), encoding="utf-8")
        self.addCleanup(sbom.SPDX.write_text, original)
        self.assertIn("namespace-not-attestation-bound", sbom.validate_tracked())

    def test_a_rewritten_subject_fails_the_provenance_validation(self) -> None:
        original = provenance.PROVENANCE.read_text(encoding="utf-8")
        document = json.loads(original)
        document["statement"]["subject"][0]["digest"]["sha256"] = "f" * 64
        provenance.PROVENANCE.write_text(provenance.canonical(document), encoding="utf-8")
        self.addCleanup(provenance.PROVENANCE.write_text, original)
        self.assertIn("statement-digest", provenance.validate_tracked())


if __name__ == "__main__":
    unittest.main()
