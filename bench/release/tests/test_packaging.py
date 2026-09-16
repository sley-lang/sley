"""Offline tests of the S20-720 packaging primitives: determinism, manifest, scan, comparison."""

from __future__ import annotations

import importlib.util
import io
import os
import re
import subprocess
import tarfile
import tempfile
import time
import unittest
from unittest.mock import patch
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("build_release_candidate", ROOT / "scripts/build_release_candidate.py")
assert SPEC is not None and SPEC.loader is not None
packaging = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(packaging)
DEMO_SPEC = importlib.util.spec_from_file_location("run_demo", ROOT / "bench/release/run_demo.py")
assert DEMO_SPEC is not None and DEMO_SPEC.loader is not None
demo = importlib.util.module_from_spec(DEMO_SPEC)
DEMO_SPEC.loader.exec_module(demo)


def stage_tree(root: Path, *, secret: bool = False, path_leak: bool = False, remap_leak: bool = False) -> Path:
    stage = root / "stage"
    (stage / "bin").mkdir(parents=True)
    # The planted key prefix is assembled at runtime so the source itself
    # carries no secret-shaped literal for the T54 scan.
    planted = ("AK" + "IA" + "1234567890ABCDEF").encode("ascii") if secret else b""
    content = b"\x7fELF fake binary " + planted + (str(ROOT).encode() if path_leak else b"")
    if remap_leak:
        content += b"\x00/home-remapped/sley2/crates/sley-cli/src/main.rs\x00"
    else:
        content += b"\x00/sley2/crates/sley-cli/src/main.rs\x00"
    (stage / "bin/sley").write_bytes(content)
    (stage / "demo").mkdir()
    (stage / "demo/run_demo.py").write_text("print('demo')\n", encoding="utf-8")
    (stage / "LICENSE").write_text("license\n", encoding="utf-8")
    (stage / "NOTICE").write_text("notice\n", encoding="utf-8")
    return stage


class PackagingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def stage_actual(self, stage: Path) -> dict:
        binary = self.root / "sley-fixture"
        binary.write_bytes(b"fixture binary")
        return packaging.stage_artifact(binary, stage, commit="a" * 40,
            toolchain={"cargo": "fixture", "rustc": "fixture"},
            working_tree_clean=True, blockers=[])

    def test_stage_and_git_fixture_enumeration_share_the_member_owner(self) -> None:
        stage = self.root / "actual"
        self.stage_actual(stage)
        fixtures = subprocess.check_output(["git", "ls-tree", "-r", "--name-only",
            "HEAD", "--", *packaging.CONFORMANCE_SUBSET], cwd=ROOT, text=True).splitlines()
        actual = {path.relative_to(stage).as_posix() for path in stage.rglob("*") if path.is_file()}
        self.assertEqual(actual, packaging.expected_artifact_members(fixtures))

    def test_staging_refuses_extra_and_missing_fixed_members(self) -> None:
        build_manifest = packaging.build_manifest
        for mutation in ("extra", "missing"):
            def mutate_stage(stage, **kwargs):
                if mutation == "extra":
                    (stage / "unexpected.txt").write_text("unexpected")
                else:
                    (stage / "LICENSE").unlink()
                return build_manifest(stage, **kwargs)
            with self.subTest(mutation=mutation), patch.object(packaging, "build_manifest", side_effect=mutate_stage):
                with self.assertRaises(packaging.PackageError) as error:
                    self.stage_actual(self.root / mutation)
                self.assertEqual(int(error.exception.code), 72007)
                self.assertIn("owned member set", error.exception.detail)

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
        self.assertEqual(infos[f"{packaging.ARTIFACT_STEM}/LICENSE"].mode, 0o644)
        self.assertEqual(infos[f"{packaging.ARTIFACT_STEM}/NOTICE"].mode, 0o644)
        self.assertEqual(first[:2], b"\x1f\x8b")
        self.assertEqual(first[4:8], b"\x00\x00\x00\x00", "gzip mtime is zero")

    def test_manifest_digest_is_canonical_and_detects_changes(self) -> None:
        stage = stage_tree(self.root)
        toolchain = {"cargo": "cargo 1.93.0", "rustc": "rustc 1.93.0"}
        manifest = packaging.build_manifest(
            stage,
            commit="a" * 40,
            toolchain=toolchain,
            working_tree_clean=True,
            blockers=["root_license_text_operator_approval"],
        )
        self.assertEqual(manifest["contract"], packaging.MANIFEST_CONTRACT)
        self.assertEqual(manifest["member_count"], 4)
        self.assertEqual([entry["path"] for entry in manifest["files"]], ["LICENSE", "NOTICE", "bin/sley", "demo/run_demo.py"])
        again = packaging.build_manifest(
            stage,
            commit="a" * 40,
            toolchain=toolchain,
            working_tree_clean=True,
            blockers=["root_license_text_operator_approval"],
        )
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

    def test_manifest_carries_its_non_release_status_inside_the_digest(self) -> None:
        stage = stage_tree(self.root)
        toolchain = {"cargo": "cargo 1.93.0", "rustc": "rustc 1.93.0"}
        manifest = packaging.build_manifest(
            stage,
            commit="a" * 40,
            toolchain=toolchain,
            working_tree_clean=False,
            blockers=["root_license_text_operator_approval", "council_reviews"],
        )
        self.assertFalse(manifest["ga_claimed"])
        self.assertFalse(manifest["publication_authorized"])
        self.assertEqual(
            manifest["blockers"],
            ["root_license_text_operator_approval", "council_reviews"],
        )
        self.assertFalse(manifest["working_tree_clean"])
        (stage / "MANIFEST.json").write_bytes(packaging.canonical(manifest))
        packaging.verify_manifest(stage, manifest)
        # Flipping the cleanliness flag without re-digesting is detected,
        # and a manifest that omits the status fields is refused outright.
        dirty = dict(manifest)
        dirty["working_tree_clean"] = True
        with self.assertRaises(packaging.PackageError):
            packaging.verify_manifest(stage, dirty)
        stripped = {key: value for key, value in manifest.items() if key != "working_tree_clean"}
        with self.assertRaises(packaging.PackageError) as error:
            packaging.verify_manifest(stage, stripped)
        self.assertEqual(error.exception.code, packaging.PackageErrorCode.MANIFEST_INVALID)

    def test_remap_order_is_most_general_first(self) -> None:
        flags = packaging.remap_flags(
            Path("/home/greyforge/sley2"),
            Path("/home/greyforge/.cargo"),
            Path("/home/greyforge"),
        )
        self.assertEqual(
            flags,
            [
                "--remap-path-prefix=/home/greyforge=/home-remapped",
                "--remap-path-prefix=/home/greyforge/.cargo/registry/src=/cargo/registry/src",
                "--remap-path-prefix=/home/greyforge/sley2=/sley2",
            ],
        )
        # rustc applies the last matching rule: the tree rule must come
        # last or the home rule shadows it back into a leak the scan cannot
        # see (/home-remapped contains neither the tree path nor /home/).
        positions = {flag: index for index, flag in enumerate(flags)}
        self.assertLess(positions[flags[0]], positions[flags[1]])
        self.assertLess(positions[flags[1]], positions[flags[2]])
        self.assertIn("/sley2", flags[2])

    def test_scan_sees_the_remap_residue_the_old_needles_missed(self) -> None:
        needles = (str(ROOT), "/home/", "/home-remapped", "greyforge")
        leaking = stage_tree(self.root / "leak", remap_leak=True)
        patterns = {finding["pattern"] for finding in packaging.scan_forbidden_content(leaking, needles)}
        self.assertIn("/home-remapped", patterns)
        clean = stage_tree(self.root / "clean")
        self.assertEqual(packaging.scan_forbidden_content(clean, needles), [])

    def test_demo_limits_match_the_governed_bridge_limits(self) -> None:
        bridge = (ROOT / "crates/sley-json-bridge/src/lib.rs").read_text(encoding="utf-8")
        match = re.search(r"const LIMIT_FIELDS: \[&str; \d+\] = \[(.*?)\];", bridge, re.S)
        self.assertIsNotNone(match)
        assert match is not None
        governed = set(re.findall(r'"([a-z_0-9]+)"', match.group(1)))
        self.assertEqual(set(demo.ZERO_LIMITS), governed)

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


class InvocationTests(unittest.TestCase):
    """The recorded invocation replays: only accepted flags, every path."""

    def test_no_keep_is_an_accepted_flag(self) -> None:
        arguments = packaging.build_parser().parse_args(
            ["--timeout-seconds=900", "--require-clean", "--no-keep"]
        )
        self.assertFalse(arguments.keep)
        self.assertTrue(arguments.require_clean)

    def test_recorded_invocations_parse(self) -> None:
        parser = packaging.build_parser()
        for invocation in (
            "build_release_candidate.py --timeout-seconds=900 --require-clean --no-keep",
            "build_release_candidate.py --timeout-seconds=900 --require-clean --keep",
            "build_release_candidate.py --timeout-seconds=60 --allow-dirty --no-keep",
        ):
            words = invocation.split()[1:]
            arguments = parser.parse_args(words)
            self.assertIsInstance(arguments.timeout_seconds, int)

    def test_porcelain_sees_untracked_files(self) -> None:
        # The whole-tree property the worktree procedure rests on: the
        # exact `git status --porcelain` command git_state() uses must
        # report an untracked file, so --require-clean refuses it.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            setup = (
                ["git", "init", "-q", "."],
                ["git", "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"],
            )
            for argv in setup:
                done = packaging.run(list(argv), cwd=root, env=None, timeout=60)
                self.assertEqual(done.returncode, 0, argv)
            (root / "probe.txt").write_text("untracked\n", encoding="utf-8")
            scan = packaging.run(["git", "status", "--porcelain"], cwd=root, env=None, timeout=60)
            self.assertEqual(scan.returncode, 0)
            self.assertIn("probe.txt", scan.stdout)


class FailureEvidenceTests(unittest.TestCase):
    """PackageError failures keep the partial record with failure attached."""

    def test_main_writes_partial_evidence_with_failure_attached(self) -> None:
        import tempfile

        partial = {
            "contract": "s20-720-release-candidate-v1",
            "commit": "0" * 40,
            "artifact_sha256": "f" * 64,
            "artifact_size_bytes": 1,
        }
        original = packaging.build_candidate

        def failing(**kwargs):
            raise packaging.PackageError(
                packaging.PackageErrorCode.NOT_REPRODUCIBLE,
                '["member"]',
                evidence=dict(partial),
            )

        packaging.build_candidate = failing
        try:
            with tempfile.TemporaryDirectory() as tmp:
                code = packaging.main(
                    [
                        "--timeout-seconds=900",
                        "--require-clean",
                        "--no-keep",
                        "--evidence-dir",
                        tmp,
                    ]
                )
                self.assertEqual(code, 1)
                evidence = __import__("json").loads(
                    (Path(tmp) / "evidence.json").read_text(encoding="utf-8")
                )
        finally:
            packaging.build_candidate = original
        self.assertEqual(evidence["result"], "FAIL")
        self.assertEqual(evidence["artifact_sha256"], "f" * 64)
        self.assertEqual(
            evidence["invocation"],
            "build_release_candidate.py --timeout-seconds=900 --require-clean --no-keep",
        )
        self.assertEqual(evidence["failure"]["symbol"], "PACKAGE_NOT_REPRODUCIBLE")


if __name__ == "__main__":
    unittest.main()
