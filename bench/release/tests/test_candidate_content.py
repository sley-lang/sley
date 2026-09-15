"""Artifact checks use actual archive bytes and the selected candidate identity."""

import copy
import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path

from bench.release.tests.test_packaging import packaging, stage_tree

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("candidate_content", ROOT / "scripts/build_candidate_content_report.py")
assert SPEC is not None and SPEC.loader is not None
content = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(content)


class ContentTests(unittest.TestCase):
    def fixture(self, root, *, leaked=False, extra=False):
        stage = stage_tree(root)
        for relative in packaging.CONFORMANCE_SUBSET:
            shutil.copytree(ROOT / relative, stage / relative)
        for name in ("SBOM.json", "LICENSES.json"):
            (stage / name).write_text("{}\n")
        if extra:
            (stage / "cache.pyc").write_bytes(b"cache")
        if leaked:
            (stage / "bin/sley").write_bytes(b"fixture /home/example/build/source.rs")
        attestation = copy.deepcopy(json.loads((ROOT / "evidence/release/reproducibility-report.json").read_text())["attestations"][0])
        manifest = packaging.build_manifest(
            stage, commit=attestation["commit"], toolchain=attestation["toolchain"],
            working_tree_clean=True, blockers=[],
        )
        (stage / "MANIFEST.json").write_bytes(packaging.canonical(manifest) + b"\n")
        artifact = root / packaging.ARTIFACT_NAME
        data = packaging.deterministic_tar(stage, artifact)
        attestation.update(artifact_sha256=packaging.sha256_bytes(data), artifact_size_bytes=len(data), manifest_digest=manifest["manifest_digest"])
        return artifact, attestation

    def test_checks_archive_and_refuses_changed_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact, attestation = self.fixture(Path(directory))
            report = content.build_report(artifact, attestation)
            self.assertEqual(report["result"], "PASS")
            self.assertEqual(report["checks"], {"manifest": True, "forbidden_content": True})
            for key, value in (("artifact_sha256", "0" * 64), ("artifact_size_bytes", 1), ("manifest_digest", "0" * 64), ("commit", "0" * 40)):
                with self.subTest(key=key), self.assertRaises(ValueError):
                    content.build_report(artifact, dict(attestation, **{key: value}))
            artifact.write_bytes(artifact.read_bytes() + b"changed")
            with self.assertRaises(ValueError):
                content.build_report(artifact, attestation)

    def test_local_path_in_matching_archive_fails_content_check(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact, attestation = self.fixture(Path(directory), leaked=True)
            report = content.build_report(artifact, attestation)
            self.assertEqual(report["result"], "FAIL")
            self.assertEqual(report["checks"], {"manifest": True, "forbidden_content": False})

    def test_extra_member_fails_even_with_a_matching_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact, attestation = self.fixture(Path(directory), extra=True)
            with self.assertRaisesRegex(ValueError, "member set"):
                content.build_report(artifact, attestation)

    def test_new_primary_selects_build_despite_carried_old_secondary(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact, attestation = self.fixture(Path(directory))
            current = dict(attestation, host_label="primary")
            older = dict(attestation, host_label="secondary", commit="0" * 40)
            repro = content.reproducibility.build_report([current, older])
            self.assertIsNone(content.reproducibility.select_attestation(repro))
            candidate = dict(current, contract="s20-720-release-candidate-v1", result="PASS")
            self.assertEqual(content.build_for_candidate(artifact, candidate, repro)["result"], "PASS")
            with self.assertRaisesRegex(ValueError, "absent or failed"):
                content.build_for_candidate(artifact, dict(candidate, result="FAIL"), repro)


if __name__ == "__main__":
    unittest.main()
