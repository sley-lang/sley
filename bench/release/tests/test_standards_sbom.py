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


class ProvenanceTests(unittest.TestCase):
    def setUp(self) -> None:
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


if __name__ == "__main__":
    unittest.main()
