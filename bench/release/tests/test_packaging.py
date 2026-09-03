"""Offline tests of the S20-720 packaging primitives: determinism, manifest, scan, comparison."""

from __future__ import annotations

import importlib.util
import io
import os
import tarfile
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("build_release_candidate", ROOT / "scripts/build_release_candidate.py")
assert SPEC is not None and SPEC.loader is not None
packaging = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(packaging)


def stage_tree(root: Path, *, secret: bool = False, path_leak: bool = False) -> Path:
    stage = root / "stage"
    (stage / "bin").mkdir(parents=True)
    # The planted key prefix is assembled at runtime so the source itself
    # carries no secret-shaped literal for the T54 scan.
    planted = ("AK" + "IA" + "1234567890ABCDEF").encode("ascii") if secret else b""
    (stage / "bin/sley").write_bytes(b"\x7fELF fake binary " + planted + (str(ROOT).encode() if path_leak else b""))
    (stage / "demo").mkdir()
    (stage / "demo/run_demo.py").write_text("print('demo')\n", encoding="utf-8")
    (stage / "LICENSE-PENDING.txt").write_text("pending\n", encoding="utf-8")
    return stage


class PackagingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_deterministic_tar_ignores_times_owners_and_order(self) -> None:
        stage = stage_tree(self.root)
        first = packaging.deterministic_tar(stage, self.root / "a.tar.gz")
        for path in stage.rglob("*"):
            os.utime(path, (time.time() + 1_000, time.time() + 1_000))
        second = packaging.deterministic_tar(stage, self.root / "b.tar.gz")
        self.assertEqual(first, second)
        with tarfile.open(fileobj=io.BytesIO(first), mode="r:gz") as archive:
            names = archive.getnames()
            infos = {info.name: info for info in archive.getmembers()}
        self.assertEqual(names, sorted(names))
        self.assertTrue(all(info.mtime == 0 and info.uid == 0 and info.gid == 0 for info in infos.values()))
        self.assertEqual(infos[f"{packaging.ARTIFACT_STEM}/bin/sley"].mode, 0o755)
        self.assertEqual(infos[f"{packaging.ARTIFACT_STEM}/LICENSE-PENDING.txt"].mode, 0o644)
        self.assertEqual(first[:2], b"\x1f\x8b")
        self.assertEqual(first[4:8], b"\x00\x00\x00\x00", "gzip mtime is zero")

    def test_manifest_digest_is_canonical_and_detects_changes(self) -> None:
        stage = stage_tree(self.root)
        toolchain = {"cargo": "cargo 1.93.0", "rustc": "rustc 1.93.0"}
        manifest = packaging.build_manifest(stage, commit="a" * 40, toolchain=toolchain)
        self.assertEqual(manifest["contract"], packaging.MANIFEST_CONTRACT)
        self.assertEqual(manifest["member_count"], 3)
        self.assertEqual([entry["path"] for entry in manifest["files"]], ["LICENSE-PENDING.txt", "bin/sley", "demo/run_demo.py"])
        again = packaging.build_manifest(stage, commit="a" * 40, toolchain=toolchain)
        self.assertEqual(manifest["manifest_digest"], again["manifest_digest"])
        (stage / "MANIFEST.json").write_bytes(packaging.canonical(manifest))
        packaging.verify_manifest(stage, manifest)
        (stage / "bin/sley").write_bytes(b"changed")
        with self.assertRaises(packaging.PackageError) as error:
            packaging.verify_manifest(stage, manifest)
        self.assertEqual(error.exception.code, packaging.PackageErrorCode.MANIFEST_INVALID)
        tampered = dict(manifest, commit="b" * 40)
        with self.assertRaises(packaging.PackageError):
            packaging.verify_manifest(stage_tree(self.root / "other"), tampered)

    def test_scan_finds_local_paths_and_secret_patterns(self) -> None:
        clean = stage_tree(self.root / "clean")
        self.assertEqual(packaging.scan_forbidden_content(clean, (str(ROOT), "/home/")), [])
        leaking = stage_tree(self.root / "leak", secret=True, path_leak=True)
        findings = packaging.scan_forbidden_content(leaking, (str(ROOT), "/home/"))
        patterns = {finding["pattern"] for finding in findings}
        self.assertIn("AK" + "IA", patterns)
        self.assertIn(str(ROOT), patterns)
        self.assertTrue(all(finding["member"] == "bin/sley" for finding in findings))

    def test_compare_artifacts_distinguishes_content_from_archive_metadata(self) -> None:
        stage = stage_tree(self.root)
        first = packaging.deterministic_tar(stage, self.root / "a.tar.gz")
        self.assertEqual(packaging.compare_artifacts(first, first)["result"], "REPRODUCIBLE")
        (stage / "bin/sley").write_bytes(b"different binary")
        second = packaging.deterministic_tar(stage, self.root / "b.tar.gz")
        comparison = packaging.compare_artifacts(first, second)
        self.assertEqual(comparison["result"], "NOT_REPRODUCIBLE")
        self.assertEqual(comparison["differing_members"], [f"{packaging.ARTIFACT_STEM}/bin/sley"])
        self.assertFalse(comparison["archive_only"])
        # Same members, different archive bytes (a non-deterministic gzip header) is archive-only.
        import gzip
        with tarfile.open(fileobj=io.BytesIO(first), mode="r:gz") as archive:
            raw = io.BytesIO()
            with tarfile.open(fileobj=raw, mode="w", format=tarfile.PAX_FORMAT) as copy:
                for info in archive.getmembers():
                    info.mtime = 12_345
                    copy.addfile(info, archive.extractfile(info) if info.isfile() else None)
        metadata_only = io.BytesIO()
        with gzip.GzipFile(filename="", mode="wb", fileobj=metadata_only, mtime=99) as handle:
            handle.write(raw.getvalue())
        comparison = packaging.compare_artifacts(first, metadata_only.getvalue())
        self.assertEqual(comparison["result"], "NOT_REPRODUCIBLE")
        self.assertTrue(comparison["archive_only"])
        self.assertEqual(comparison["differing_members"], [])


if __name__ == "__main__":
    unittest.main()
