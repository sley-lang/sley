"""SYNTHETIC TEST FIXTURES ONLY: live-shaped run directories for accounting.

Every run built here is written to a temporary directory, carries a
``synthetic-fixture-`` run id and ``environment_manifest.synthetic_fixture``,
and is never a model trial or campaign evidence. Records pass the real
``verify_attempts`` path: real frozen starting-state snapshots, the real
environment receipt, schema-valid provider streams whose usage equals the
recorded metrics, canonical oracle reports and stdout lines, and (sley_2_0)
runner-style evidence completion bindings.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable

from bench.live.artifacts import ArtifactStore
from bench.live.attempts import append_attempt, build_attempt
from bench.live.campaign import _completion_payload, _empty_metrics
from bench.live.environment import environment_snapshot_bytes
from bench.live.live_claims import _initial_snapshot
from bench.live.manifest import build_manifest, canonical_json_bytes, write_manifest_once
from bench.live.oracle import ARM_NAMES
from bench.live.taskpacks import TASK_IDS, arm_fixture_digest
from bench.live.tooling import build_prompt

ARMS = ("raw_files", "sley_1_2_0", "sley_2_0")
CONTEXT_BUDGET = 100_000
ACTION_BUDGET = 50
WALL_BUDGET = 600_000
_STARTS: dict[tuple[str, str], bytes] = {}


def _sha(label: str) -> str:
    return hashlib.sha256(label.encode()).hexdigest()


@dataclass
class Slot:
    """One synthetic attempt outcome and the metrics it claims."""

    status: str = "rejected"          # accepted | rejected | timeout | harness_failure
    code: str | None = "ORACLE_SYNTHETIC_MISMATCH"
    input_tokens: int = 1_000
    output_tokens: int = 100
    tool_calls: int = 3
    context_bytes: int = 1_000
    repair_loops: int = 1
    wall_time: int = 1_000
    invalid_candidates: int = 0
    collateral: int = 0
    invalid_committed: int = 0
    stale: int = 0
    stale_bad: int = 0
    human: int = 0
    stream: str = "completed"          # completed | failed | none
    extra: dict[str, Any] = field(default_factory=dict)


def manifest(run_id: str, *, label: str = "CAMPAIGN", seeds: tuple[int, ...] = (7,)) -> dict[str, Any]:
    return build_manifest(
        run_id=run_id,
        created_at_utc="2026-09-24T00:00:00Z",
        repo_commit="5" * 40,
        model_exact_version="synthetic-fixture-model",
        model_tier="small",
        reasoning_effort="medium",
        trial_count=len(seeds),
        random_seeds=list(seeds),
        context_budget=CONTEXT_BUDGET,
        action_budget=ACTION_BUDGET,
        wall_time_budget=WALL_BUDGET,
        retry_policy={"provider_attempts": 1, "retryable_failures": []},
        hardware_manifest={"node": "synthetic-fixture"},
        cache_state="synthetic-fixture",
        environment_manifest={
            "label": label,
            "provider_environment": {"HOME": "/synthetic", "PATH": "/usr/bin:/bin"},
            "synthetic_fixture": "TEST ONLY - not a model trial",
        },
        arm_fixture_digests={arm: arm_fixture_digest(arm) for arm in ARMS},
        tool_description_digests={arm: _sha(arm + "-tools") for arm in ARMS},
        oracle_digest=_sha("oracle"),
        prompt_template_digest=_sha("prompt"),
        provider_executable_sha256=_sha("provider"),
        provider_version="synthetic-fixture",
    )


def _events(slot: Slot) -> bytes:
    if slot.stream == "none":
        return b""
    lines: list[dict[str, Any]] = [{"type": "thread.started", "thread_id": "synthetic"}, {"type": "turn.started"}]
    for index in range(slot.tool_calls if slot.stream == "completed" else 0):
        lines.append({"type": "item.completed", "item": {
            "id": f"t{index}", "type": "command_execution", "command": "true",
            "aggregated_output": "", "status": "completed", "exit_code": 0}})
    if slot.stream == "completed":
        lines.append({"type": "item.completed", "item": {"id": "m", "type": "agent_message", "text": "synthetic"}})
        lines.append({"type": "turn.completed", "usage": {
            "input_tokens": slot.input_tokens, "cached_input_tokens": 0,
            "output_tokens": slot.output_tokens, "reasoning_output_tokens": 0}})
    else:
        lines.append({"type": "turn.failed", "error": {"message": "synthetic provider failure"}})
    return b"".join(json.dumps(line, sort_keys=True).encode() + b"\n" for line in lines)


def _start(arm: str, task: str) -> bytes:
    key = (arm, task)
    if key not in _STARTS:
        _STARTS[key] = _initial_snapshot(arm, task)
    return _STARTS[key]


def append_slot(run: Path, value: dict[str, Any], store: ArtifactStore, arm: str, task: str,
                seed: int, slot: Slot) -> dict[str, Any]:
    judged = slot.status in {"accepted", "rejected"}
    completed = slot.stream == "completed"
    metrics = _empty_metrics()
    metrics.update({
        "context_bytes": slot.context_bytes,
        "repair_loops": slot.repair_loops,
        "wall_time": slot.wall_time,
        "invalid_candidates": slot.invalid_candidates,
        "human_interventions": slot.human,
        "model_input_tokens": slot.input_tokens if completed else 0,
        "model_output_tokens": slot.output_tokens if completed else 0,
        "tool_calls": slot.tool_calls if completed else 0,
    })
    metrics["total_observable_tokens"] = metrics["model_input_tokens"] + metrics["model_output_tokens"]
    metrics.update(slot.extra)
    events = _events(slot)
    put = lambda payload: None if payload is None else store.put(payload).sha256  # noqa: E731
    attempt_id = f"{value['run_id']}.{arm}.{task.lower()}.{seed}"
    artifacts: dict[str, str | None] = {
        "agent_transcript_sha256": None, "agent_usage_sha256": None,
        "evidence_completion_sha256": None, "final_candidate_sha256": None,
        "environment_snapshot_sha256": put(environment_snapshot_bytes(value)),
        "final_message_sha256": put(b"synthetic") if completed else None,
        "oracle_report_sha256": None, "oracle_stderr_sha256": None, "oracle_stdout_sha256": None,
        "prompt_sha256": put(build_prompt(task, seed, arm)),
        "provider_events_sha256": put(events),
        "provider_stderr_sha256": put(b""),
        "workspace_after_sha256": put(_start(arm, task)),
        "workspace_before_sha256": put(_start(arm, task)),
    }
    if judged:
        metrics["accepted_correct_changes"] = 1 if slot.status == "accepted" else 0
        metrics["strict_accepted_correctness"] = slot.status == "accepted"
        report = {
            "arm_id": arm, "code": None if slot.status == "accepted" else slot.code,
            "collateral_semantic_changes": slot.collateral,
            "contract": "sley2.live-oracle-report.v1",
            "invalid_committed_states": slot.invalid_committed,
            "mutation_reconstructable": True, "required_check_bypassed": False,
            "stale_candidates": slot.stale,
            "stale_candidates_incorrectly_accepted": slot.stale_bad,
            "status": slot.status, "task_id": task,
        }
        for name, key in (("collateral_semantic_changes", "collateral_semantic_changes"),
                          ("invalid_committed_states", "invalid_committed_states"),
                          ("stale_candidates", "stale_candidates"),
                          ("stale_candidates_incorrectly_accepted", "stale_candidates_incorrectly_accepted")):
            metrics[name] = report[key]
        artifacts["oracle_report_sha256"] = put(canonical_json_bytes(report) + b"\n")
        artifacts["oracle_stdout_sha256"] = put(json.dumps({
            "arm": ARM_NAMES[arm], "code": report["code"], "detail": "synthetic fixture",
            "status": slot.status, "task_id": task}, sort_keys=True).encode() + b"\n")
        artifacts["oracle_stderr_sha256"] = put(b"")
        if arm == "sley_2_0":
            evidence = {"agent_transcript_sha256": b"synthetic transcript\n",
                        "agent_usage_sha256": b'{"totals":{}}\n',
                        "final_candidate_sha256": b"synthetic final\n"}
            digests = {slot_name: put(data) for slot_name, data in evidence.items()}
            artifacts.update(digests)
            artifacts["evidence_completion_sha256"] = put(_completion_payload(
                attempt_id=attempt_id, evidence=evidence, digests=digests, provider_events=events))
    record = build_attempt(
        manifest=value, attempt_id=attempt_id, task_id=task, arm_id=arm, seed=seed,
        started_at_utc="2026-09-24T00:00:01Z", ended_at_utc="2026-09-24T00:00:02Z",
        status=slot.status, failure_code=None if slot.status == "accepted" else (slot.code or "SYNTHETIC"),
        provider_exit_code=0 if judged else (124 if slot.status == "timeout" else 1),
        artifacts=artifacts, metrics=metrics)
    append_attempt(run, record)
    return record


def build_run(root: Path, name: str, plan: Callable[[str, int, str], Slot | None], *,
              label: str = "CAMPAIGN", seeds: tuple[int, ...] = (7,)) -> Path:
    """Write a synthetic run: ``plan(arm, task_index, task_id)`` gives each
    slot's outcome, or None to leave the slot unrun (a PARTIAL arm)."""

    run = Path(root) / name
    value = manifest(f"synthetic-fixture-{name}", label=label, seeds=seeds)
    write_manifest_once(run / "run_manifest.json", value)
    store = ArtifactStore(run / "artifacts")
    for seed in seeds:
        for index, task in enumerate(TASK_IDS):
            for arm in ARMS:
                slot = plan(arm, index, task)
                if slot is not None:
                    append_slot(run, value, store, arm, task, seed, slot)
    return run
