"""Frozen run manifest for one small- or large-model live campaign."""

from __future__ import annotations

import hashlib
import json
import os
import re
from datetime import datetime
from pathlib import Path
from typing import Any, Mapping

from bench.raw.runner import task_statement_digest


ROOT = Path(__file__).resolve().parents[2]
PLAN = ROOT / "bench" / "benchmark-plan.json"
CORPUS = ROOT / "bench" / "corpus" / "v1" / "tasks.json"
CONTRACT = "sley2.live-campaign-manifest.v1"
DOMAIN = b"sley2.live-campaign-manifest.v1\0"
ARMS = frozenset({"raw_files", "sley_1_2_0", "sley_2_0"})
MODEL_TIERS = frozenset({"small", "large"})
REASONING_EFFORTS = frozenset({"low", "medium", "high", "xhigh", "max", "ultra"})
HEX_40 = re.compile(r"[0-9a-f]{40}\Z")
HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
RUN_ID = re.compile(r"[a-z0-9][a-z0-9._-]{0,127}\Z")
UTC_SECOND = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\Z")
EXACT_FIELDS = frozenset(
    {
        "action_budget",
        "arm_fixture_digests",
        "benchmark_plan_digest",
        "cache_state",
        "context_budget",
        "contract",
        "corpus_digest",
        "created_at_utc",
        "environment_manifest",
        "hardware_manifest",
        "model_configuration",
        "model_exact_version",
        "model_provider",
        "model_tier",
        "oracle_digest",
        "prompt_template_digest",
        "provider_executable_sha256",
        "provider_version",
        "random_seeds",
        "repo_commit",
        "retry_policy",
        "run_id",
        "scheduled_attempts",
        "task_statement_digest",
        "tool_description_digests",
        "trial_count",
        "wall_time_budget",
    }
)


class ManifestError(ValueError):
    """A live manifest is absent, malformed, mutable, or inconsistent."""


def _fail(symbol: str, detail: str = "") -> None:
    raise ManifestError(symbol if not detail else f"{symbol}: {detail}")


def _canonical_value(value: Any, path: str = "$") -> None:
    if value is None or isinstance(value, (str, bool)):
        return
    if isinstance(value, int) and not isinstance(value, bool):
        if -(1 << 63) <= value <= (1 << 64) - 1:
            return
        _fail("LIVE_MANIFEST_INVALID", f"integer range {path}")
    if isinstance(value, list):
        for index, item in enumerate(value):
            _canonical_value(item, f"{path}[{index}]")
        return
    if isinstance(value, dict) and all(isinstance(key, str) for key in value):
        for key in sorted(value):
            _canonical_value(value[key], f"{path}.{key}")
        return
    _fail("LIVE_MANIFEST_INVALID", f"unsupported value {path}")


def canonical_json_bytes(value: Any) -> bytes:
    _canonical_value(value)
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def _sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def manifest_digest(manifest: Mapping[str, Any]) -> str:
    validate_manifest(manifest)
    return hashlib.sha256(DOMAIN + canonical_json_bytes(dict(manifest))).hexdigest()


def _digest_map(value: Any, field: str) -> None:
    if not isinstance(value, dict) or set(value) != ARMS:
        _fail("LIVE_MANIFEST_INVALID", field)
    if any(not isinstance(item, str) or HEX_64.fullmatch(item) is None for item in value.values()):
        _fail("LIVE_MANIFEST_INVALID", field)


def _positive_int(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        _fail("LIVE_MANIFEST_INVALID", field)
    return value


def validate_manifest(manifest: Mapping[str, Any]) -> None:
    if not isinstance(manifest, Mapping) or set(manifest) != EXACT_FIELDS:
        _fail("LIVE_MANIFEST_INVALID", "field set")
    _canonical_value(dict(manifest))
    if manifest["contract"] != CONTRACT:
        _fail("LIVE_MANIFEST_INVALID", "contract")
    if not isinstance(manifest["run_id"], str) or RUN_ID.fullmatch(manifest["run_id"]) is None:
        _fail("LIVE_MANIFEST_INVALID", "run_id")
    created = manifest["created_at_utc"]
    if not isinstance(created, str) or UTC_SECOND.fullmatch(created) is None:
        _fail("LIVE_MANIFEST_INVALID", "created_at_utc")
    try:
        datetime.strptime(created, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ManifestError("LIVE_MANIFEST_INVALID: created_at_utc") from error
    if not isinstance(manifest["repo_commit"], str) or HEX_40.fullmatch(manifest["repo_commit"]) is None:
        _fail("LIVE_MANIFEST_INVALID", "repo_commit")
    expected_digests = {
        "benchmark_plan_digest": _sha256_file(PLAN),
        "corpus_digest": _sha256_file(CORPUS),
        "task_statement_digest": task_statement_digest(),
    }
    for field, expected in expected_digests.items():
        if manifest[field] != expected:
            _fail("LIVE_MANIFEST_INVALID", field)
    if manifest["model_provider"] != "openai-chatgpt-oauth":
        _fail("LIVE_MANIFEST_INVALID", "model_provider")
    version = manifest["model_exact_version"]
    if not isinstance(version, str) or not version or "latest" in version.lower():
        _fail("LIVE_MANIFEST_INVALID", "model_exact_version")
    if not isinstance(manifest["model_tier"], str) or manifest["model_tier"] not in MODEL_TIERS:
        _fail("LIVE_MANIFEST_INVALID", "model_tier")
    configuration = manifest["model_configuration"]
    if not isinstance(configuration, dict) or set(configuration) != {"reasoning_effort"}:
        _fail("LIVE_MANIFEST_INVALID", "model_configuration")
    if (
        not isinstance(configuration["reasoning_effort"], str)
        or configuration["reasoning_effort"] not in REASONING_EFFORTS
    ):
        _fail("LIVE_MANIFEST_INVALID", "reasoning_effort")
    _digest_map(manifest["arm_fixture_digests"], "arm_fixture_digests")
    _digest_map(manifest["tool_description_digests"], "tool_description_digests")
    for field in ("oracle_digest", "prompt_template_digest", "provider_executable_sha256"):
        value = manifest[field]
        if not isinstance(value, str) or HEX_64.fullmatch(value) is None:
            _fail("LIVE_MANIFEST_INVALID", field)
    for field in ("context_budget", "action_budget", "wall_time_budget", "trial_count"):
        _positive_int(manifest[field], field)
    seeds = manifest["random_seeds"]
    if (
        not isinstance(seeds, list)
        or len(seeds) != manifest["trial_count"]
        or len(set(seeds)) != len(seeds)
        or any(isinstance(seed, bool) or not isinstance(seed, int) or seed < 0 for seed in seeds)
    ):
        _fail("LIVE_MANIFEST_INVALID", "random_seeds")
    expected_attempts = 15 * len(ARMS) * manifest["trial_count"]
    if manifest["scheduled_attempts"] != expected_attempts:
        _fail("LIVE_MANIFEST_INVALID", "scheduled_attempts")
    retry = manifest["retry_policy"]
    if not isinstance(retry, dict) or set(retry) != {"provider_attempts", "retryable_failures"}:
        _fail("LIVE_MANIFEST_INVALID", "retry_policy")
    if retry["provider_attempts"] != 1 or retry["retryable_failures"] != []:
        _fail("LIVE_MANIFEST_INVALID", "retry_policy")
    for field in ("hardware_manifest", "environment_manifest"):
        if not isinstance(manifest[field], dict) or not manifest[field]:
            _fail("LIVE_MANIFEST_INVALID", field)
    for field in ("cache_state", "provider_version"):
        if not isinstance(manifest[field], str) or not manifest[field]:
            _fail("LIVE_MANIFEST_INVALID", field)


def build_manifest(
    *,
    run_id: str,
    created_at_utc: str,
    repo_commit: str,
    model_exact_version: str,
    model_tier: str,
    reasoning_effort: str,
    trial_count: int,
    random_seeds: list[int],
    context_budget: int,
    action_budget: int,
    wall_time_budget: int,
    retry_policy: dict[str, Any],
    hardware_manifest: dict[str, Any],
    cache_state: str,
    environment_manifest: dict[str, Any],
    arm_fixture_digests: dict[str, str],
    tool_description_digests: dict[str, str],
    oracle_digest: str,
    prompt_template_digest: str,
    provider_executable_sha256: str,
    provider_version: str,
) -> dict[str, Any]:
    manifest = {
        "action_budget": action_budget,
        "arm_fixture_digests": dict(arm_fixture_digests),
        "benchmark_plan_digest": _sha256_file(PLAN),
        "cache_state": cache_state,
        "context_budget": context_budget,
        "contract": CONTRACT,
        "corpus_digest": _sha256_file(CORPUS),
        "created_at_utc": created_at_utc,
        "environment_manifest": dict(environment_manifest),
        "hardware_manifest": dict(hardware_manifest),
        "model_configuration": {"reasoning_effort": reasoning_effort},
        "model_exact_version": model_exact_version,
        "model_provider": "openai-chatgpt-oauth",
        "model_tier": model_tier,
        "oracle_digest": oracle_digest,
        "prompt_template_digest": prompt_template_digest,
        "provider_executable_sha256": provider_executable_sha256,
        "provider_version": provider_version,
        "random_seeds": list(random_seeds),
        "repo_commit": repo_commit,
        "retry_policy": dict(retry_policy),
        "run_id": run_id,
        "scheduled_attempts": 15 * len(ARMS) * trial_count,
        "task_statement_digest": task_statement_digest(),
        "tool_description_digests": dict(tool_description_digests),
        "trial_count": trial_count,
        "wall_time_budget": wall_time_budget,
    }
    validate_manifest(manifest)
    return manifest


def write_manifest_once(path: Path, manifest: Mapping[str, Any]) -> None:
    validate_manifest(manifest)
    path = Path(path)
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, 0o600)
    except FileExistsError as error:
        raise ManifestError("LIVE_MANIFEST_EXISTS") from error
    try:
        payload = canonical_json_bytes(dict(manifest)) + b"\n"
        written = 0
        while written < len(payload):
            count = os.write(descriptor, payload[written:])
            if count <= 0:
                _fail("LIVE_MANIFEST_WRITE_FAILED", "short write")
            written += count
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def read_manifest(path: Path) -> dict[str, Any]:
    try:
        payload = Path(path).read_bytes()
        value = json.loads(payload)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ManifestError(f"LIVE_MANIFEST_INVALID: {error}") from error
    if not isinstance(value, dict):
        _fail("LIVE_MANIFEST_INVALID", "not an object")
    validate_manifest(value)
    if payload != canonical_json_bytes(value) + b"\n":
        _fail("LIVE_MANIFEST_INVALID", "noncanonical bytes")
    return value
