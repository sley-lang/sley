"""One-attempt transaction for the executable live succession campaign."""

from __future__ import annotations

import json
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Mapping

from bench.live.artifacts import ArtifactStore
from bench.live.attempts import append_attempt, build_attempt
from bench.live.environment import environment_snapshot_bytes
from bench.live.manifest import canonical_json_bytes, read_manifest
from bench.live.metrics import derive_provider_observation
from bench.live.oracle import OracleError, run_fixture_oracle
from bench.live.process import ProcessCapture, run_provider_process
from bench.live.provider import CodexEvents, CodexExecAdapter, ProviderError, parse_codex_jsonl
from bench.live.snapshot import encode_snapshot, snapshot_directory
from bench.live.taskpacks import stage_initial
from bench.live.tooling import build_prompt, stage_tooling


MAX_PROVIDER_OUTPUT_BYTES = 512 * 1024 * 1024
ORACLE_REPORT_CONTRACT = "sley2.live-oracle-report.v1"
_COLLATERAL_CODES = frozenset(
    {
        "ORACLE_BOOL_COMPAT_FIELD",
        "ORACLE_DUPLICATE_IMPL",
        "ORACLE_IMPL_TOUCHED",
        "ORACLE_PUBLIC_DELETED",
        "ORACLE_REACHABLE_DIGEST_CHANGED",
        "ORACLE_SIGNATURE_CHANGED",
    }
)


class CampaignError(ValueError):
    """A live attempt could not be staged or durably recorded."""


def _utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _artifact(store: ArtifactStore, payload: bytes | None) -> str | None:
    return None if payload is None else store.put(payload).sha256


def _empty_metrics() -> dict[str, Any]:
    plan = Path(__file__).resolve().parents[1] / "benchmark-plan.json"
    names = json.loads(plan.read_text(encoding="utf-8"))["metrics"]
    metrics: dict[str, Any] = {name: 0 for name in names}
    metrics["accepted_change_tokens"] = None
    metrics["attempted_tasks"] = 1
    metrics["strict_accepted_correctness"] = False
    return metrics


def _oracle_report(
    verdict: Mapping[str, Any],
    *,
    task_id: str,
    arm_id: str,
    snapshots_complete: bool,
) -> dict[str, Any]:
    accepted = verdict["status"] == "accepted"
    code = verdict["code"]
    return {
        "arm_id": arm_id,
        "code": code,
        "collateral_semantic_changes": 0 if accepted or code not in _COLLATERAL_CODES else 1,
        "contract": ORACLE_REPORT_CONTRACT,
        "invalid_committed_states": 0,
        "mutation_reconstructable": snapshots_complete,
        # The oracle is an independently launched, immutable executable input;
        # the candidate cannot replace or suppress it.
        "required_check_bypassed": False,
        "stale_candidates": 1 if accepted and task_id == "S2B-STALE-001" else 0,
        "stale_candidates_incorrectly_accepted": 0,
        "status": verdict["status"],
        "task_id": task_id,
    }


def _apply_observation(
    metrics: dict[str, Any],
    events: CodexEvents,
    prompt: bytes,
    capture: ProcessCapture,
) -> None:
    metrics.update(derive_provider_observation(events, prompt))
    metrics["wall_time"] = capture.wall_time_ms
    metrics["peak_memory"] = capture.peak_memory_bytes


def execute_attempt(
    *,
    run_directory: Path,
    store: ArtifactStore,
    adapter: CodexExecAdapter,
    task_id: str,
    arm_id: str,
    seed: int,
    workspace_parent: Path,
    provider_runner: Callable[..., ProcessCapture] = run_provider_process,
    oracle_runner: Callable[..., tuple[dict[str, Any], bytes, bytes]] = run_fixture_oracle,
    utc_now: Callable[[], str] = _utc_now,
) -> dict[str, Any]:
    """Execute and append one slot; completed provider calls are never retried."""

    run = Path(run_directory)
    manifest = read_manifest(run / "run_manifest.json")
    if adapter.model != manifest["model_exact_version"] or adapter.reasoning_effort != manifest["model_configuration"]["reasoning_effort"]:
        raise CampaignError("LIVE_CAMPAIGN_PROVIDER_MISMATCH")
    parent = Path(workspace_parent)
    parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    prompt = build_prompt(task_id, seed, arm_id)
    environment_payload = environment_snapshot_bytes(manifest)
    started = utc_now()
    metrics = _empty_metrics()
    status = "harness_failure"
    failure_code: str | None = "LIVE_CAMPAIGN_INTERNAL"
    capture: ProcessCapture | None = None
    events: CodexEvents | None = None
    final_message: bytes | None = None
    oracle_report_payload: bytes | None = None
    oracle_stdout: bytes | None = None
    oracle_stderr: bytes | None = None
    after_payload: bytes | None = None

    with tempfile.TemporaryDirectory(prefix="attempt-", dir=parent) as temporary:
        workspace = Path(temporary) / "candidate"
        stage_initial(arm_id, task_id, workspace)
        stage_tooling(arm_id, workspace)
        before_snapshot = snapshot_directory(workspace)
        before_payload = encode_snapshot(before_snapshot)
        try:
            capture = provider_runner(
                adapter.command(workspace),
                prompt,
                timeout_ms=manifest["wall_time_budget"],
                max_output_bytes=MAX_PROVIDER_OUTPUT_BYTES,
                environment=None,
            )
            try:
                after_snapshot = snapshot_directory(workspace)
                after_payload = encode_snapshot(after_snapshot)
                metrics["canonical_storage_bytes"] = after_snapshot["total_bytes"]
            except Exception:
                failure_code = "LIVE_WORKSPACE_AFTER_INVALID"

            if capture.timed_out:
                status = "timeout"
                failure_code = "LIVE_PROVIDER_TIMEOUT"
                metrics["wall_time"] = capture.wall_time_ms
            elif capture.exit_code != 0:
                status = "harness_failure"
                failure_code = "LIVE_PROVIDER_EXIT_NONZERO"
                metrics["wall_time"] = capture.wall_time_ms
            else:
                try:
                    events = parse_codex_jsonl(capture.stdout)
                    _apply_observation(metrics, events, prompt, capture)
                    if events.final_message is not None:
                        final_message = events.final_message.encode("utf-8")
                except ProviderError:
                    status = "harness_failure"
                    failure_code = "LIVE_PROVIDER_EVENT_INVALID"
                else:
                    if (
                        metrics["model_input_tokens"] > manifest["context_budget"]
                        or metrics["tool_calls"] > manifest["action_budget"]
                    ):
                        status = "harness_failure"
                        failure_code = "LIVE_PROVIDER_BUDGET_EXCEEDED"
                    elif after_payload is not None:
                        oracle_started = time.monotonic_ns()
                        try:
                            verdict, oracle_stdout, oracle_stderr = oracle_runner(
                                arm_id=arm_id,
                                task_id=task_id,
                                candidate=workspace,
                            )
                        except OracleError:
                            status = "harness_failure"
                            failure_code = "LIVE_ORACLE_INVALID"
                        else:
                            metrics["execution_latency"] = max(
                                0, (time.monotonic_ns() - oracle_started) // 1_000_000
                            )
                            report = _oracle_report(
                                verdict,
                                task_id=task_id,
                                arm_id=arm_id,
                                snapshots_complete=True,
                            )
                            oracle_report_payload = canonical_json_bytes(report) + b"\n"
                            status = verdict["status"]
                            failure_code = None if status == "accepted" else verdict["code"]
                            metrics["accepted_correct_changes"] = 1 if status == "accepted" else 0
                            metrics["strict_accepted_correctness"] = status == "accepted"
                            metrics["invalid_candidates"] += 1 if status == "rejected" else 0
                            for field in (
                                "collateral_semantic_changes",
                                "invalid_committed_states",
                                "stale_candidates",
                                "stale_candidates_incorrectly_accepted",
                            ):
                                metrics[field] = report[field]
        except Exception as error:
            # Staging errors occur before this boundary; after provider launch,
            # retain the attempt rather than losing the denominator slot.
            status = "harness_failure"
            failure_code = f"LIVE_PROVIDER_PROCESS_FAILURE_{type(error).__name__.upper()}"

        ended = utc_now()
        artifacts = {
            "environment_snapshot_sha256": _artifact(store, environment_payload),
            "final_message_sha256": _artifact(store, final_message),
            "oracle_report_sha256": _artifact(store, oracle_report_payload),
            "oracle_stderr_sha256": _artifact(store, oracle_stderr),
            "oracle_stdout_sha256": _artifact(store, oracle_stdout),
            "prompt_sha256": _artifact(store, prompt),
            "provider_events_sha256": _artifact(store, b"" if capture is None else capture.stdout),
            "provider_stderr_sha256": _artifact(store, b"" if capture is None else capture.stderr),
            "workspace_after_sha256": _artifact(store, after_payload),
            "workspace_before_sha256": _artifact(store, before_payload),
        }
        attempt_id = f"{manifest['run_id']}.{arm_id}.{task_id.lower()}.{seed}"
        record = build_attempt(
            manifest=manifest,
            attempt_id=attempt_id,
            task_id=task_id,
            arm_id=arm_id,
            seed=seed,
            started_at_utc=started,
            ended_at_utc=ended,
            status=status,
            failure_code=failure_code,
            provider_exit_code=None if capture is None else capture.exit_code,
            artifacts=artifacts,
            metrics=metrics,
        )
        append_attempt(run, record)
        return record
