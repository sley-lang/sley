from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

from bench.live.artifacts import ArtifactStore
from bench.live.attempts import verify_attempts
from bench.live.campaign import execute_attempt
from bench.live.manifest import write_manifest_once
from bench.live.process import ProcessCapture
from bench.live.provider import CodexExecAdapter
from bench.live.tests.test_attempts import manifest


def provider_stream() -> bytes:
    values = [
        {"type": "thread.started", "thread_id": "thread-1"},
        {"type": "turn.started"},
        {
            "type": "item.completed",
            "item": {
                "id": "tool-1",
                "type": "command_execution",
                "command": "cat program.py && python3 -m unittest",
                "aggregated_output": "OK\n",
                "status": "completed",
                "exit_code": 0,
            },
        },
        {
            "type": "item.completed",
            "item": {"id": "message-1", "type": "agent_message", "text": "done"},
        },
        {
            "type": "turn.completed",
            "usage": {
                "input_tokens": 100,
                "cached_input_tokens": 20,
                "output_tokens": 25,
                "reasoning_output_tokens": 5,
            },
        },
    ]
    return b"".join(json.dumps(value, sort_keys=True).encode() + b"\n" for value in values)


class CampaignAttemptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.run = self.root / "run"
        self.run.mkdir()
        write_manifest_once(self.run / "run_manifest.json", manifest())
        self.store = ArtifactStore(self.run / "artifacts")
        self.adapter = CodexExecAdapter("/opt/codex", "gpt-5.6-sol", "medium")
        self.times = iter(["2026-09-17T12:01:00Z", "2026-09-17T12:01:01Z"])

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_complete_attempt_is_snapshotted_judged_appended_and_verified(self) -> None:
        observed_environment = None

        def provider(argv, prompt, **kwargs):
            nonlocal observed_environment
            observed_environment = kwargs["environment"]
            workspace = Path(argv[argv.index("--cd") + 1])
            source = Path(__file__).resolve().parents[2] / "fixtures/raw/S2B-REPAIR-001/fixture"
            shutil.copy2(source / "program.py", workspace / "program.py")
            shutil.copy2(source / "test_program.py", workspace / "test_program.py")
            return ProcessCapture(provider_stream(), b"provider note\n", 0, False, 25)

        record = execute_attempt(
            run_directory=self.run,
            store=self.store,
            adapter=self.adapter,
            task_id="S2B-REPAIR-001",
            arm_id="raw_files",
            seed=17,
            workspace_parent=self.root / "workspaces",
            provider_runner=provider,
            utc_now=lambda: next(self.times),
        )
        self.assertEqual(record["status"], "accepted")
        self.assertIsNone(record["failure_code"])
        self.assertEqual(record["metrics"]["compile_or_check_attempts"], 1)
        self.assertEqual(record["metrics"]["files_inspected"], 1)
        verified = verify_attempts(self.run, self.store)
        self.assertEqual(len(verified), 1)
        self.assertEqual(verified[0]["evidence_status"], "VERIFIED_LIVE_EVIDENCE")
        self.assertNotEqual(
            record["artifacts"]["workspace_before_sha256"],
            record["artifacts"]["workspace_after_sha256"],
        )
        self.assertIsNotNone(record["artifacts"]["oracle_stdout_sha256"])
        self.assertIsNotNone(record["artifacts"]["oracle_stderr_sha256"])
        self.assertEqual(
            observed_environment,
            {"HOME": "/home/benchmark", "LANG": "C.UTF-8", "PATH": "/usr/bin:/bin"},
        )

    def test_timeout_is_retained_without_an_oracle_claim(self) -> None:
        def provider(argv, prompt, **kwargs):
            return ProcessCapture(b'{"type":"turn.started"}\n', b"timed out\n", 124, True, 1_800_001)

        record = execute_attempt(
            run_directory=self.run,
            store=self.store,
            adapter=self.adapter,
            task_id="S2B-REPAIR-001",
            arm_id="raw_files",
            seed=17,
            workspace_parent=self.root / "workspaces",
            provider_runner=provider,
            utc_now=lambda: next(self.times),
        )
        self.assertEqual(record["status"], "timeout")
        self.assertEqual(record["failure_code"], "LIVE_PROVIDER_TIMEOUT")
        self.assertIsNone(record["artifacts"]["oracle_report_sha256"])
        self.assertEqual(len(verify_attempts(self.run, self.store)), 1)


if __name__ == "__main__":
    unittest.main()
