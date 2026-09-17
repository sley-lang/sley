from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from bench.live.artifacts import ArtifactStore
from bench.live.attempts import (
    AttemptError,
    append_attempt,
    build_attempt,
    verify_attempts,
)
from bench.live.environment import environment_snapshot_bytes
from bench.live.manifest import build_manifest, canonical_json_bytes, write_manifest_once
from bench.live.snapshot import encode_snapshot, snapshot_directory


def sha(label: str) -> str:
    return hashlib.sha256(label.encode()).hexdigest()


def manifest() -> dict:
    return build_manifest(
        run_id="live-small-one",
        created_at_utc="2026-09-17T12:00:00Z",
        repo_commit="2" * 40,
        model_exact_version="gpt-5.6-sol",
        model_tier="small",
        reasoning_effort="medium",
        trial_count=1,
        random_seeds=[17],
        context_budget=131_072,
        action_budget=64,
        wall_time_budget=1_800_000,
        retry_policy={"provider_attempts": 1, "retryable_failures": []},
        hardware_manifest={"node": "greyarch"},
        cache_state="cold-per-trial",
        environment_manifest={
            "locale": "C.UTF-8",
            "provider_environment": {
                "HOME": "/home/benchmark",
                "LANG": "C.UTF-8",
                "PATH": "/usr/bin:/bin",
            },
        },
        arm_fixture_digests={arm: sha(arm) for arm in ("raw_files", "sley_1_2_0", "sley_2_0")},
        tool_description_digests={arm: sha(arm + "-tools") for arm in ("raw_files", "sley_1_2_0", "sley_2_0")},
        oracle_digest=sha("oracle"),
        prompt_template_digest=sha("prompt"),
        provider_executable_sha256=sha("codex"),
        provider_version="codex-cli 0.154.0",
    )


def provider_events() -> bytes:
    values = [
        {"type": "thread.started", "thread_id": "thread-1"},
        {"type": "turn.started"},
        {
            "type": "item.completed",
            "item": {"id": "tool-1", "type": "command_execution", "command": "python3 -m unittest", "status": "completed", "exit_code": 0},
        },
        {"type": "item.completed", "item": {"id": "message-1", "type": "agent_message", "text": "complete"}},
        {
            "type": "turn.completed",
            "usage": {"input_tokens": 100, "cached_input_tokens": 20, "output_tokens": 25, "reasoning_output_tokens": 5},
        },
    ]
    return b"".join(json.dumps(value, sort_keys=True).encode() + b"\n" for value in values)


def metrics(status: str = "accepted") -> dict:
    names = json.loads((Path(__file__).resolve().parents[2] / "benchmark-plan.json").read_text())["metrics"]
    result = {name: 0 for name in names}
    result.update(
        {
            "strict_accepted_correctness": status == "accepted",
            "attempted_tasks": 1,
            "accepted_correct_changes": 1 if status == "accepted" else 0,
            "accepted_change_tokens": None,
            "model_input_tokens": 100,
            "model_output_tokens": 25,
            "total_observable_tokens": 125,
            "tool_calls": 1,
            "wall_time": 1_000,
        }
    )
    return result


class AttemptLogTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.run = Path(self.temporary.name)
        write_manifest_once(self.run / "run_manifest.json", manifest())
        self.store = ArtifactStore(self.run / "artifacts")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def artifact(self, payload: bytes) -> str:
        return self.store.put(payload).sha256

    def snapshot(self, content: bytes) -> bytes:
        workspace = self.run / f"snapshot-{len(list(self.run.glob('snapshot-*')))}"
        workspace.mkdir()
        (workspace / "program.bin").write_bytes(content)
        return encode_snapshot(snapshot_directory(workspace))

    def accepted_attempt(self) -> dict:
        oracle = {
            "arm_id": "raw_files",
            "code": None,
            "collateral_semantic_changes": 0,
            "contract": "sley2.live-oracle-report.v1",
            "invalid_committed_states": 0,
            "mutation_reconstructable": True,
            "required_check_bypassed": False,
            "stale_candidates": 0,
            "stale_candidates_incorrectly_accepted": 0,
            "status": "accepted",
            "task_id": "S2B-REPAIR-001",
        }
        artifacts = {
            "environment_snapshot_sha256": self.artifact(environment_snapshot_bytes(manifest())),
            "final_message_sha256": self.artifact(b"complete"),
            "oracle_report_sha256": self.artifact(canonical_json_bytes(oracle) + b"\n"),
            "oracle_stderr_sha256": self.artifact(b""),
            "oracle_stdout_sha256": self.artifact(
                b'{"arm":"raw","code":null,"detail":"checked","status":"accepted","task_id":"S2B-REPAIR-001"}\n'
            ),
            "prompt_sha256": self.artifact(b"prompt"),
            "provider_events_sha256": self.artifact(provider_events()),
            "provider_stderr_sha256": self.artifact(b""),
            "workspace_after_sha256": self.artifact(self.snapshot(b"after")),
            "workspace_before_sha256": self.artifact(self.snapshot(b"before")),
        }
        return build_attempt(
            manifest=manifest(),
            attempt_id="raw-repair-17",
            task_id="S2B-REPAIR-001",
            arm_id="raw_files",
            seed=17,
            started_at_utc="2026-09-17T12:01:00Z",
            ended_at_utc="2026-09-17T12:01:01Z",
            status="accepted",
            failure_code=None,
            provider_exit_code=0,
            artifacts=artifacts,
            metrics=metrics(),
        )

    def test_append_and_verify_reconcile_captured_truth(self) -> None:
        record = self.accepted_attempt()
        digest = append_attempt(self.run, record)
        verified = verify_attempts(self.run, self.store)
        self.assertEqual(verified[0]["record_digest"], digest)
        self.assertEqual(verified[0]["evidence_status"], "VERIFIED_LIVE_EVIDENCE")

    def test_duplicate_slot_refuses(self) -> None:
        record = self.accepted_attempt()
        append_attempt(self.run, record)
        duplicate = dict(record)
        duplicate["attempt_id"] = "another-id"
        with self.assertRaisesRegex(AttemptError, "LIVE_ATTEMPT_DUPLICATE"):
            append_attempt(self.run, duplicate)

    def test_provider_metrics_and_oracle_identity_fail_closed(self) -> None:
        for mutation in ("tokens", "oracle"):
            with self.subTest(mutation=mutation):
                (self.run / "attempts.jsonl").unlink(missing_ok=True)
                record = self.accepted_attempt()
                if mutation == "tokens":
                    record["metrics"]["model_input_tokens"] = 99
                    record["metrics"]["total_observable_tokens"] = 124
                else:
                    wrong = {
                        "arm_id": "raw_files",
                        "code": None,
                        "collateral_semantic_changes": 0,
                        "contract": "sley2.live-oracle-report.v1",
                        "invalid_committed_states": 0,
                        "mutation_reconstructable": True,
                        "required_check_bypassed": False,
                        "stale_candidates": 0,
                        "stale_candidates_incorrectly_accepted": 0,
                        "status": "accepted",
                        "task_id": "S2B-CREATE-001",
                    }
                    record["artifacts"]["oracle_report_sha256"] = self.artifact(canonical_json_bytes(wrong) + b"\n")
                append_attempt(self.run, record)
                with self.assertRaises(AttemptError):
                    verify_attempts(self.run, self.store)

    def test_missing_artifact_is_detected(self) -> None:
        record = self.accepted_attempt()
        append_attempt(self.run, record)
        self.store.path_for(record["artifacts"]["workspace_after_sha256"]).unlink()
        with self.assertRaisesRegex(AttemptError, "LIVE_ATTEMPT_ARTIFACT_INVALID"):
            verify_attempts(self.run, self.store)

    def test_noncanonical_workspace_snapshot_is_detected(self) -> None:
        record = self.accepted_attempt()
        record["artifacts"]["workspace_after_sha256"] = self.artifact(b"not a snapshot")
        append_attempt(self.run, record)
        with self.assertRaisesRegex(AttemptError, "LIVE_ATTEMPT_SNAPSHOT_INVALID"):
            verify_attempts(self.run, self.store)

    def test_environment_snapshot_must_match_the_frozen_manifest(self) -> None:
        record = self.accepted_attempt()
        record["artifacts"]["environment_snapshot_sha256"] = self.artifact(b"{}")
        append_attempt(self.run, record)
        with self.assertRaisesRegex(AttemptError, "LIVE_ATTEMPT_ENVIRONMENT_INVALID"):
            verify_attempts(self.run, self.store)

    def test_unhashable_enums_fail_as_attempt_errors(self) -> None:
        for field, value in (("arm_id", []), ("status", [])):
            with self.subTest(field=field):
                record = self.accepted_attempt()
                record[field] = value
                with self.assertRaisesRegex(AttemptError, "LIVE_ATTEMPT_INVALID"):
                    append_attempt(self.run, record)


if __name__ == "__main__":
    unittest.main()
