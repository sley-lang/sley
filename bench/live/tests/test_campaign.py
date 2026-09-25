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

    def test_budget_exceeded_attempt_is_retained_with_observed_metrics(self) -> None:
        # A completed provider run that overspent the frozen context
        # budget is a harness failure that stays in the denominator with
        # the tokens it actually used; it must never be dropped because
        # the record validator rejects its honest metrics.
        over = json.loads(provider_stream().splitlines()[-1])
        over["usage"]["input_tokens"] = 131_073

        def provider(argv, prompt, **kwargs):
            lines = provider_stream().splitlines()[:-1]
            stream = b"".join(item + b"\n" for item in lines)
            stream += json.dumps(over, sort_keys=True).encode() + b"\n"
            return ProcessCapture(stream, b"", 0, False, 25)

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
        self.assertEqual(record["status"], "harness_failure")
        self.assertEqual(record["failure_code"], "LIVE_PROVIDER_BUDGET_EXCEEDED")
        self.assertEqual(record["metrics"]["model_input_tokens"], 131_073)
        self.assertEqual(len(verify_attempts(self.run, self.store)), 1)

    def test_unparseable_stream_keeps_its_measured_wall_time(self) -> None:
        def provider(argv, prompt, **kwargs):
            # Exit 0 but no terminal usage record: the stream cannot parse.
            return ProcessCapture(b'{"type":"turn.started"}\n', b"", 0, False, 4321, 777)

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
        self.assertEqual(record["failure_code"], "LIVE_PROVIDER_EVENT_INVALID")
        self.assertEqual(record["metrics"]["wall_time"], 4321)
        self.assertEqual(record["metrics"]["peak_memory"], 777)
        self.assertEqual(record["metrics"]["model_input_tokens"], 0)

    def test_over_budget_metrics_are_refused_on_any_other_outcome(self) -> None:
        from bench.live.attempts import AttemptError, validate_attempt

        def provider(argv, prompt, **kwargs):
            return ProcessCapture(b'{"type":"turn.started"}\n', b"", 1, False, 25)

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
        self.assertEqual(record["failure_code"], "LIVE_PROVIDER_EXIT_NONZERO")
        forged = json.loads(json.dumps(record))
        forged["metrics"]["model_input_tokens"] = 131_073
        forged["metrics"]["total_observable_tokens"] = 131_073
        with self.assertRaisesRegex(AttemptError, "context_budget"):
            validate_attempt(forged, manifest())


# NOTE: the sley_2_0 workspace-copy evidence route retired when the
# arm moved to confined mediated attempts (bench/live/mediated_attempt.py):
# candidate-side files can never be acceptance evidence. The sley_2_0
# campaign-path proofs (legitimate acceptance, capture/oracle gate
# ordering, forgery resistance, ingress invalidation, timeouts,
# cumulative budgets, access restrictions) live in
# bench/live/tests/test_mediated_attempt.py and enter through
# execute_attempt like every other test here.


if __name__ == "__main__":
    unittest.main()
