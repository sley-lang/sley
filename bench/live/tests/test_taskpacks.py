from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from bench.live.manifest import ARMS
from bench.live.snapshot import snapshot_directory
from bench.live.taskpacks import (
    IMPLEMENTED_ARMS,
    TASK_IDS,
    arm_fixture_digest,
    stage_initial,
)


class TaskPackTests(unittest.TestCase):
    def test_raw_and_legacy_catalogs_cover_the_frozen_corpus(self) -> None:
        self.assertEqual(IMPLEMENTED_ARMS, frozenset({"raw_files", "sley_1_2_0", "sley_2_0"}))
        self.assertEqual(len(TASK_IDS), 15)
        self.assertEqual(IMPLEMENTED_ARMS, ARMS - {"zerolang"})
        for arm in sorted(IMPLEMENTED_ARMS):
            with self.subTest(arm=arm):
                self.assertEqual(len(arm_fixture_digest(arm)), 64)

    def test_staging_is_deterministic_and_excludes_transient_bytecode(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            first = Path(temporary) / "first"
            second = Path(temporary) / "second"
            stage_initial("raw_files", "S2B-REPAIR-001", first)
            stage_initial("raw_files", "S2B-REPAIR-001", second)
            self.assertEqual(snapshot_directory(first), snapshot_directory(second))
            paths = [entry["path"] for entry in snapshot_directory(first)["entries"]]
            self.assertIn("program.py", paths)
            self.assertFalse(any("__pycache__" in path or path.endswith(".pyc") for path in paths))

    def test_create_starts_blank_but_keeps_raw_acceptance_tests(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            raw = Path(temporary) / "raw"
            legacy = Path(temporary) / "legacy"
            stage_initial("raw_files", "S2B-CREATE-001", raw)
            stage_initial("sley_1_2_0", "S2B-CREATE-001", legacy)
            self.assertFalse((raw / "program.py").exists())
            self.assertTrue((raw / "test_program.py").is_file())
            self.assertEqual(list(legacy.iterdir()), [])

    def test_unknown_arm_and_task_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            for arm, task in (("sley_9_9", "S2B-REPAIR-001"), ("raw_files", "NOPE")):
                with self.subTest(arm=arm, task=task):
                    with self.assertRaises(ValueError):
                        stage_initial(arm, task, Path(temporary) / f"{arm}-{task}")

    def test_sley2_stages_pack_and_blank_repo_deterministically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            first = Path(temporary) / "first"
            second = Path(temporary) / "second"
            stage_initial("sley_2_0", "S2B-REPAIR-001", first)
            stage_initial("sley_2_0", "S2B-REPAIR-001", second)
            self.assertEqual(snapshot_directory(first), snapshot_directory(second))
            self.assertTrue((first / "base.pack").is_file())
            self.assertTrue((first / "repo").is_dir())
            genesis = Path(temporary) / "genesis"
            stage_initial("sley_2_0", "S2B-CREATE-001", genesis)
            self.assertEqual(list((genesis / "repo").iterdir()), [])
            # Genesis pack: scaffolding only, no program entities (the
            # program is authored through the trial surface, never
            # pre-seeded). Digest-pinned to the frozen fixture.
            self.assertTrue((genesis / "base.pack").is_file())
            import hashlib

            pack = (genesis / "base.pack").read_bytes()
            fixture = (Path(__file__).resolve().parents[3] / "bench" / "fixtures"
                       / "sley2" / "S2B-CREATE-001" / "base.pack").read_bytes()
            self.assertEqual(pack, fixture)


if __name__ == "__main__":
    unittest.main()
