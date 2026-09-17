from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from bench.live.tooling import (
    build_prompt,
    prompt_template_digest,
    stage_tooling,
    tooling_digest,
)


class ToolingTests(unittest.TestCase):
    def test_prompt_is_identical_across_arms_and_binds_task_and_seed(self) -> None:
        prompts = {
            build_prompt("S2B-REPAIR-001", 17, arm)
            for arm in ("raw_files", "sley_1_2_0", "sley_2_0")
        }
        self.assertEqual(len(prompts), 1)
        prompt = prompts.pop()
        self.assertIn('"id":"S2B-REPAIR-001"', prompt.decode())
        self.assertIn("Trial seed: 17", prompt.decode())
        self.assertEqual(len(prompt_template_digest()), 64)

    def test_raw_and_legacy_tooling_are_deterministic_and_legacy_launcher_is_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for arm in ("raw_files", "sley_1_2_0"):
                first = root / f"{arm}-one"
                second = root / f"{arm}-two"
                first.mkdir()
                second.mkdir()
                stage_tooling(arm, first)
                stage_tooling(arm, second)
                self.assertEqual(tooling_digest(arm), tooling_digest(arm))
                self.assertEqual(
                    (first / ".sley-live/TOOLING.md").read_bytes(),
                    (second / ".sley-live/TOOLING.md").read_bytes(),
                )
            launcher = root / "sley_1_2_0-one/.sley-live/sley-tool"
            self.assertTrue(launcher.stat().st_mode & 0o111)

    def test_unknown_task_arm_and_existing_control_directory_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.mkdir(exist_ok=True)
            with self.assertRaises(ValueError):
                build_prompt("NOPE", 1, "raw_files")
            with self.assertRaises(ValueError):
                stage_tooling("sley_2_0", root)
            (root / ".sley-live").mkdir()
            with self.assertRaises(ValueError):
                stage_tooling("raw_files", root)


if __name__ == "__main__":
    unittest.main()
