from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import re

from bench.live import mediated_shim, mediated_sley, sley2_tool
from bench.live.tooling import (
    SLEY2_TOOLING,
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
            for arm in ("raw_files", "sley_1_2_0", "sley_2_0"):
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
            sley2_launcher = root / "sley_2_0-one/.sley-live/sley-tool"
            self.assertTrue(sley2_launcher.stat().st_mode & 0o111)

    def test_unknown_task_arm_and_existing_control_directory_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.mkdir(exist_ok=True)
            with self.assertRaises(ValueError):
                build_prompt("NOPE", 1, "raw_files")
            with self.assertRaises(ValueError):
                stage_tooling("sley_9_9", root)
            (root / ".sley-live").mkdir()
            with self.assertRaises(ValueError):
                stage_tooling("raw_files", root)


class Sley2CommandSurfacePinTests(unittest.TestCase):
    """The agent-facing TOOLING.md is the surface the live model reads: its
    documented command lines, the mediated gateway's allowed commands, the
    shim's phase sets, and the tool dispatcher's commands move together."""

    def documented(self) -> list[str]:
        return re.findall(r"^\.sley-live/sley-tool (\S+)", SLEY2_TOOLING, flags=re.M)

    def test_every_allowed_command_is_documented_and_nothing_else(self) -> None:
        documented = self.documented()
        self.assertEqual(len(documented), len(set(documented)), documented)
        for name in sorted(mediated_sley.ALLOWED_COMMANDS):
            with self.subTest(name=name):
                self.assertIn(f".sley-live/sley-tool {name}", SLEY2_TOOLING)
        self.assertEqual(set(documented), set(mediated_sley.ALLOWED_COMMANDS))

    def test_documented_argument_forms_for_open_and_revision(self) -> None:
        lines = set(re.findall(r"^\.sley-live/sley-tool .*$", SLEY2_TOOLING, flags=re.M))
        self.assertIn(".sley-live/sley-tool open", lines)
        self.assertIn(".sley-live/sley-tool revision TX_HEX", lines)
        self.assertIn(".sley-live/sley-tool sig ENTITY_HEX", lines)

    def test_shim_phases_enumerate_every_allowed_command(self) -> None:
        phased = (set(mediated_shim.READ_COMMANDS)
                  | set(mediated_shim.COMPOSE_COMMANDS) | {"finish"})
        self.assertEqual(set(mediated_sley.ALLOWED_COMMANDS) - phased, set())
        self.assertIn("open", mediated_shim.READ_COMMANDS)

    def test_tool_module_docstring_lists_open_and_revision_argument(self) -> None:
        doc = sley2_tool.__doc__ or ""
        self.assertRegex(doc, r"(?m)^  open\s")
        self.assertRegex(doc, r"(?m)^  revision <tx-hex>\s")


if __name__ == "__main__":
    unittest.main()
