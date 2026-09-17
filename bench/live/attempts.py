"""Append-only attempt custody and verification for live S20-640 runs."""

from __future__ import annotations

import fcntl
import hashlib
import json
import os
import re
import stat
from datetime import datetime
from pathlib import Path
from typing import Any, Mapping

from bench.live.artifacts import ArtifactError, ArtifactStore
from bench.live.environment import EnvironmentError, validate_environment_snapshot
from bench.live.manifest import (
    ARMS,
    canonical_json_bytes,
    manifest_digest,
    read_manifest,
)
from bench.live.oracle import OracleError, parse_fixture_oracle
from bench.live.provider import ProviderError, parse_codex_jsonl
from bench.live.snapshot import SnapshotError, decode_snapshot


ROOT = Path(__file__).resolve().parents[2]
PLAN = ROOT / "bench" / "benchmark-plan.json"
CORPUS = ROOT / "bench" / "corpus" / "v1" / "tasks.json"
MANIFEST_NAME = "run_manifest.json"
ATTEMPTS_NAME = "attempts.jsonl"
CONTRACT = "sley2.live-attempt.v1"
DOMAIN = b"sley2.live-attempt.v1\0"
CAPTURE_STATUS = "UNVERIFIED_LIVE_CAPTURE"
VERIFIED_STATUS = "VERIFIED_LIVE_EVIDENCE"
STATUSES = frozenset({"accepted", "rejected", "timeout", "harness_failure"})
ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9._-]{0,191}\Z")
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
UTC_SECOND = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\Z")
ARTIFACT_FIELDS = frozenset(
    {
        "environment_snapshot_sha256",
        "final_message_sha256",
        "oracle_report_sha256",
        "oracle_stderr_sha256",
        "oracle_stdout_sha256",
        "prompt_sha256",
        "provider_events_sha256",
        "provider_stderr_sha256",
        "workspace_after_sha256",
        "workspace_before_sha256",
    }
)
INPUT_FIELDS = frozenset(
    {
        "arm_id",
        "artifacts",
        "attempt_id",
        "capture_status",
        "contract",
        "ended_at_utc",
        "failure_code",
        "metrics",
        "provider_exit_code",
        "run_id",
        "run_manifest_digest",
        "seed",
        "started_at_utc",
        "status",
        "task_id",
    }
)
CHAIN_FIELDS = frozenset({"previous_record_digest", "record_digest"})
ORACLE_FIELDS = frozenset(
    {
        "arm_id",
        "code",
        "collateral_semantic_changes",
        "contract",
        "invalid_committed_states",
        "mutation_reconstructable",
        "required_check_bypassed",
        "stale_candidates",
        "stale_candidates_incorrectly_accepted",
        "status",
        "task_id",
    }
)


class AttemptError(ValueError):
    """A live attempt record, chain, or referenced artifact is invalid."""


def _fail(symbol: str, detail: str = "") -> None:
    raise AttemptError(symbol if not detail else f"{symbol}: {detail}")


def _load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise AttemptError(f"LIVE_ATTEMPT_INVALID: {error}") from error
    if not isinstance(value, dict):
        _fail("LIVE_ATTEMPT_INVALID", path.name)
    return value


def _task_ids() -> set[str]:
    corpus = _load(CORPUS)
    return {task["id"] for task in corpus.get("tasks", []) if isinstance(task, dict)}


def _metric_names() -> set[str]:
    plan = _load(PLAN)
    return set(plan.get("metrics", []))


def _parse_utc(value: Any, field: str) -> datetime:
    if not isinstance(value, str) or UTC_SECOND.fullmatch(value) is None:
        _fail("LIVE_ATTEMPT_INVALID", field)
    try:
        return datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise AttemptError(f"LIVE_ATTEMPT_INVALID: {field}") from error


def _validate_metrics(metrics: Any, manifest: Mapping[str, Any], status: str) -> None:
    if not isinstance(metrics, dict) or set(metrics) != _metric_names():
        _fail("LIVE_ATTEMPT_METRIC_INVALID", "field set")
    for name, value in metrics.items():
        if name == "strict_accepted_correctness":
            if not isinstance(value, bool):
                _fail("LIVE_ATTEMPT_METRIC_INVALID", name)
        elif name == "accepted_change_tokens":
            if value is not None:
                _fail("LIVE_ATTEMPT_METRIC_INVALID", name)
        elif isinstance(value, bool) or not isinstance(value, int) or value < 0:
            _fail("LIVE_ATTEMPT_METRIC_INVALID", name)
    accepted = status == "accepted"
    if metrics["attempted_tasks"] != 1:
        _fail("LIVE_ATTEMPT_METRIC_INVALID", "attempted_tasks")
    if metrics["accepted_correct_changes"] != (1 if accepted else 0):
        _fail("LIVE_ATTEMPT_METRIC_INVALID", "accepted_correct_changes")
    if metrics["strict_accepted_correctness"] is not accepted:
        _fail("LIVE_ATTEMPT_METRIC_INVALID", "strict_accepted_correctness")
    if metrics["total_observable_tokens"] != metrics["model_input_tokens"] + metrics["model_output_tokens"]:
        _fail("LIVE_ATTEMPT_METRIC_INVALID", "total_observable_tokens")
    if metrics["model_input_tokens"] > manifest["context_budget"]:
        _fail("LIVE_ATTEMPT_CONTROL_MISMATCH", "context_budget")
    if metrics["tool_calls"] > manifest["action_budget"]:
        _fail("LIVE_ATTEMPT_CONTROL_MISMATCH", "action_budget")
    if status != "timeout" and metrics["wall_time"] > manifest["wall_time_budget"]:
        _fail("LIVE_ATTEMPT_CONTROL_MISMATCH", "wall_time_budget")


def _validate_artifacts(artifacts: Any, status: str) -> None:
    if not isinstance(artifacts, dict) or set(artifacts) != ARTIFACT_FIELDS:
        _fail("LIVE_ATTEMPT_INVALID", "artifact field set")
    required = {
        "environment_snapshot_sha256",
        "prompt_sha256",
        "provider_events_sha256",
        "provider_stderr_sha256",
        "workspace_before_sha256",
    }
    if status in {"accepted", "rejected"}:
        required |= {
            "oracle_report_sha256",
            "oracle_stderr_sha256",
            "oracle_stdout_sha256",
            "workspace_after_sha256",
        }
    for field, value in artifacts.items():
        if value is None:
            if field in required:
                _fail("LIVE_ATTEMPT_INVALID", f"missing {field}")
            continue
        if not isinstance(value, str) or HEX_64.fullmatch(value) is None:
            _fail("LIVE_ATTEMPT_INVALID", field)


def validate_attempt(record: Mapping[str, Any], manifest: Mapping[str, Any]) -> None:
    if not isinstance(record, Mapping) or set(record) != INPUT_FIELDS:
        _fail("LIVE_ATTEMPT_INVALID", "field set")
    # Canonicalization is also a bounded-value validation.
    canonical_json_bytes(dict(record))
    if record["contract"] != CONTRACT or record["run_id"] != manifest["run_id"]:
        _fail("LIVE_ATTEMPT_INVALID", "contract/run")
    if record["run_manifest_digest"] != manifest_digest(manifest):
        _fail("LIVE_ATTEMPT_INVALID", "manifest digest")
    if not isinstance(record["attempt_id"], str) or ATTEMPT_ID.fullmatch(record["attempt_id"]) is None:
        _fail("LIVE_ATTEMPT_INVALID", "attempt_id")
    if not isinstance(record["task_id"], str) or record["task_id"] not in _task_ids():
        _fail("LIVE_ATTEMPT_INVALID", "task_id")
    if not isinstance(record["arm_id"], str) or record["arm_id"] not in ARMS:
        _fail("LIVE_ATTEMPT_INVALID", "arm_id")
    if record["seed"] not in manifest["random_seeds"]:
        _fail("LIVE_ATTEMPT_INVALID", "seed")
    started = _parse_utc(record["started_at_utc"], "started_at_utc")
    ended = _parse_utc(record["ended_at_utc"], "ended_at_utc")
    if ended < started:
        _fail("LIVE_ATTEMPT_INVALID", "time ordering")
    status = record["status"]
    if not isinstance(status, str) or status not in STATUSES:
        _fail("LIVE_ATTEMPT_INVALID", "status")
    failure = record["failure_code"]
    if status == "accepted":
        if failure is not None:
            _fail("LIVE_ATTEMPT_INVALID", "accepted failure")
    elif not isinstance(failure, str) or not failure:
        _fail("LIVE_ATTEMPT_INVALID", "missing failure")
    exit_code = record["provider_exit_code"]
    if exit_code is not None and (isinstance(exit_code, bool) or not isinstance(exit_code, int)):
        _fail("LIVE_ATTEMPT_INVALID", "provider_exit_code")
    if status in {"accepted", "rejected"} and exit_code != 0:
        _fail("LIVE_ATTEMPT_INVALID", "provider exit")
    if record["capture_status"] != CAPTURE_STATUS:
        _fail("LIVE_ATTEMPT_INVALID", "capture_status")
    _validate_artifacts(record["artifacts"], status)
    _validate_metrics(record["metrics"], manifest, status)


def build_attempt(
    *,
    manifest: Mapping[str, Any],
    attempt_id: str,
    task_id: str,
    arm_id: str,
    seed: int,
    started_at_utc: str,
    ended_at_utc: str,
    status: str,
    failure_code: str | None,
    provider_exit_code: int | None,
    artifacts: Mapping[str, str | None],
    metrics: Mapping[str, Any],
) -> dict[str, Any]:
    record = {
        "arm_id": arm_id,
        "artifacts": dict(artifacts),
        "attempt_id": attempt_id,
        "capture_status": CAPTURE_STATUS,
        "contract": CONTRACT,
        "ended_at_utc": ended_at_utc,
        "failure_code": failure_code,
        "metrics": dict(metrics),
        "provider_exit_code": provider_exit_code,
        "run_id": manifest["run_id"],
        "run_manifest_digest": manifest_digest(manifest),
        "seed": seed,
        "started_at_utc": started_at_utc,
        "status": status,
        "task_id": task_id,
    }
    validate_attempt(record, manifest)
    return record


def _record_digest(record_without_digest: Mapping[str, Any]) -> str:
    return hashlib.sha256(DOMAIN + canonical_json_bytes(dict(record_without_digest))).hexdigest()


def _read_all(descriptor: int) -> bytes:
    os.lseek(descriptor, 0, os.SEEK_SET)
    chunks: list[bytes] = []
    while True:
        chunk = os.read(descriptor, 65_536)
        if not chunk:
            return b"".join(chunks)
        chunks.append(chunk)


def _parse_records(raw: bytes, manifest: Mapping[str, Any]) -> list[dict[str, Any]]:
    if not raw:
        return []
    if not raw.endswith(b"\n"):
        _fail("LIVE_ATTEMPT_CHAIN_INVALID", "truncated")
    previous = manifest_digest(manifest)
    attempts: list[dict[str, Any]] = []
    ids: set[str] = set()
    slots: set[tuple[str, str, int]] = set()
    for index, line in enumerate(raw.splitlines()):
        try:
            record = json.loads(line)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise AttemptError(f"LIVE_ATTEMPT_CHAIN_INVALID: record {index}") from error
        if not isinstance(record, dict) or canonical_json_bytes(record) != line:
            _fail("LIVE_ATTEMPT_CHAIN_INVALID", f"noncanonical {index}")
        if set(record) != INPUT_FIELDS | CHAIN_FIELDS:
            _fail("LIVE_ATTEMPT_CHAIN_INVALID", f"field set {index}")
        body = {field: record[field] for field in INPUT_FIELDS}
        validate_attempt(body, manifest)
        if record["previous_record_digest"] != previous:
            _fail("LIVE_ATTEMPT_CHAIN_INVALID", f"previous {index}")
        digest_body = dict(body)
        digest_body["previous_record_digest"] = previous
        expected = _record_digest(digest_body)
        if record["record_digest"] != expected:
            _fail("LIVE_ATTEMPT_CHAIN_INVALID", f"digest {index}")
        slot = (record["arm_id"], record["task_id"], record["seed"])
        if record["attempt_id"] in ids or slot in slots:
            _fail("LIVE_ATTEMPT_DUPLICATE", record["attempt_id"])
        ids.add(record["attempt_id"])
        slots.add(slot)
        previous = expected
        attempts.append(record)
    if len(attempts) > manifest["scheduled_attempts"]:
        _fail("LIVE_ATTEMPT_LIMIT", str(len(attempts)))
    return attempts


def _open_regular(path: Path, flags: int) -> int:
    open_flags = flags
    if hasattr(os, "O_NOFOLLOW"):
        open_flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, open_flags, 0o600)
    except OSError as error:
        raise AttemptError(f"LIVE_ATTEMPT_STORAGE_INVALID: {error}") from error
    metadata = os.fstat(descriptor)
    if not stat.S_ISREG(metadata.st_mode):
        os.close(descriptor)
        _fail("LIVE_ATTEMPT_STORAGE_INVALID", str(path))
    return descriptor


def append_attempt(run_directory: Path, record: Mapping[str, Any]) -> str:
    run = Path(run_directory)
    manifest = read_manifest(run / MANIFEST_NAME)
    validate_attempt(record, manifest)
    run.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor = _open_regular(run / ATTEMPTS_NAME, os.O_RDWR | os.O_CREAT | os.O_APPEND)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        existing = _parse_records(_read_all(descriptor), manifest)
        slot = (record["arm_id"], record["task_id"], record["seed"])
        if any(item["attempt_id"] == record["attempt_id"] for item in existing) or any(
            (item["arm_id"], item["task_id"], item["seed"]) == slot for item in existing
        ):
            _fail("LIVE_ATTEMPT_DUPLICATE", str(record["attempt_id"]))
        if len(existing) >= manifest["scheduled_attempts"]:
            _fail("LIVE_ATTEMPT_LIMIT", str(len(existing)))
        previous = existing[-1]["record_digest"] if existing else manifest_digest(manifest)
        body = dict(record)
        body["previous_record_digest"] = previous
        digest = _record_digest(body)
        stored = dict(body)
        stored["record_digest"] = digest
        payload = canonical_json_bytes(stored) + b"\n"
        written = 0
        while written < len(payload):
            count = os.write(descriptor, payload[written:])
            if count <= 0:
                _fail("LIVE_ATTEMPT_APPEND_FAILED", "short write")
            written += count
        os.fsync(descriptor)
        return digest
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def _read_artifact(store: ArtifactStore, digest: Any, field: str) -> bytes | None:
    if digest is None:
        return None
    try:
        return store.read(digest)
    except ArtifactError as error:
        raise AttemptError(f"LIVE_ATTEMPT_ARTIFACT_INVALID: {field}: {error}") from error


def _oracle_report(payload: bytes, attempt: Mapping[str, Any]) -> dict[str, Any]:
    try:
        report = json.loads(payload)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise AttemptError("LIVE_ATTEMPT_ORACLE_INVALID: JSON") from error
    if not isinstance(report, dict) or set(report) != ORACLE_FIELDS:
        _fail("LIVE_ATTEMPT_ORACLE_INVALID", "field set")
    if payload != canonical_json_bytes(report) + b"\n":
        _fail("LIVE_ATTEMPT_ORACLE_INVALID", "noncanonical")
    if report["contract"] != "sley2.live-oracle-report.v1":
        _fail("LIVE_ATTEMPT_ORACLE_INVALID", "contract")
    for field in ("task_id", "arm_id", "status"):
        if report[field] != attempt[field]:
            _fail("LIVE_ATTEMPT_ORACLE_INVALID", field)
    if report["status"] == "accepted" and report["code"] is not None:
        _fail("LIVE_ATTEMPT_ORACLE_INVALID", "accepted code")
    if report["status"] == "rejected" and not isinstance(report["code"], str):
        _fail("LIVE_ATTEMPT_ORACLE_INVALID", "rejected code")
    for field in (
        "collateral_semantic_changes",
        "invalid_committed_states",
        "stale_candidates",
        "stale_candidates_incorrectly_accepted",
    ):
        if isinstance(report[field], bool) or not isinstance(report[field], int) or report[field] < 0:
            _fail("LIVE_ATTEMPT_ORACLE_INVALID", field)
        if attempt["metrics"][field] != report[field]:
            _fail("LIVE_ATTEMPT_ORACLE_INVALID", f"metric {field}")
    if not isinstance(report["required_check_bypassed"], bool) or not isinstance(
        report["mutation_reconstructable"], bool
    ):
        _fail("LIVE_ATTEMPT_ORACLE_INVALID", "boolean evidence")
    if report["status"] == "accepted" and (
        report["required_check_bypassed"] or not report["mutation_reconstructable"]
    ):
        _fail("LIVE_ATTEMPT_ORACLE_INVALID", "accepted evidence")
    return report


def verify_attempts(
    run_directory: Path,
    store: ArtifactStore,
    *,
    require_complete: bool = False,
) -> list[dict[str, Any]]:
    run = Path(run_directory)
    manifest = read_manifest(run / MANIFEST_NAME)
    try:
        descriptor = _open_regular(run / ATTEMPTS_NAME, os.O_RDONLY)
    except AttemptError as error:
        if not (run / ATTEMPTS_NAME).exists() and not require_complete:
            return []
        raise error
    try:
        attempts = _parse_records(_read_all(descriptor), manifest)
    finally:
        os.close(descriptor)
    if require_complete and len(attempts) != manifest["scheduled_attempts"]:
        _fail("LIVE_ATTEMPT_INCOMPLETE", f"{len(attempts)}/{manifest['scheduled_attempts']}")
    promoted: list[dict[str, Any]] = []
    for attempt in attempts:
        artifacts = attempt["artifacts"]
        payloads = {
            field: _read_artifact(store, digest, field)
            for field, digest in artifacts.items()
        }
        for field in ("workspace_before_sha256", "workspace_after_sha256"):
            snapshot_payload = payloads[field]
            if snapshot_payload is None:
                continue
            try:
                decode_snapshot(snapshot_payload)
            except SnapshotError as error:
                raise AttemptError(f"LIVE_ATTEMPT_SNAPSHOT_INVALID: {field}: {error}") from error
        environment_payload = payloads["environment_snapshot_sha256"]
        if not isinstance(environment_payload, bytes):
            _fail("LIVE_ATTEMPT_ENVIRONMENT_INVALID", "missing")
        try:
            validate_environment_snapshot(environment_payload, manifest)
        except EnvironmentError as error:
            raise AttemptError(f"LIVE_ATTEMPT_ENVIRONMENT_INVALID: {error}") from error
        events_payload = payloads["provider_events_sha256"]
        if not isinstance(events_payload, bytes):
            _fail("LIVE_ATTEMPT_ARTIFACT_INVALID", "provider_events_sha256")
        if attempt["status"] in {"accepted", "rejected"}:
            try:
                events = parse_codex_jsonl(events_payload)
            except ProviderError as error:
                raise AttemptError(f"LIVE_ATTEMPT_PROVIDER_INVALID: {error}") from error
            metrics = attempt["metrics"]
            if (
                metrics["model_input_tokens"] != events.input_tokens
                or metrics["model_output_tokens"] != events.output_tokens
                or metrics["tool_calls"] != events.tool_calls
            ):
                _fail("LIVE_ATTEMPT_PROVIDER_INVALID", "metric reconciliation")
            final = payloads["final_message_sha256"]
            if events.final_message is None:
                if final is not None:
                    _fail("LIVE_ATTEMPT_PROVIDER_INVALID", "unexpected final message")
            elif final != events.final_message.encode("utf-8"):
                _fail("LIVE_ATTEMPT_PROVIDER_INVALID", "final message")
            oracle_payload = payloads["oracle_report_sha256"]
            if not isinstance(oracle_payload, bytes):
                _fail("LIVE_ATTEMPT_ORACLE_INVALID", "missing")
            report = _oracle_report(oracle_payload, attempt)
            oracle_stdout = payloads["oracle_stdout_sha256"]
            if not isinstance(oracle_stdout, bytes):
                _fail("LIVE_ATTEMPT_ORACLE_INVALID", "stdout missing")
            try:
                verdict = parse_fixture_oracle(
                    oracle_stdout,
                    exit_code=0 if attempt["status"] == "accepted" else 1,
                    expected_arm=attempt["arm_id"],
                    expected_task=attempt["task_id"],
                )
            except OracleError as error:
                raise AttemptError(f"LIVE_ATTEMPT_ORACLE_INVALID: {error}") from error
            if verdict["status"] != report["status"] or verdict["code"] != report["code"]:
                _fail("LIVE_ATTEMPT_ORACLE_INVALID", "stdout/report mismatch")
        promoted_record = dict(attempt)
        promoted_record["evidence_status"] = VERIFIED_STATUS
        promoted.append(promoted_record)
    return promoted
