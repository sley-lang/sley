"""Manifest-bound environment receipts for each live attempt."""

from __future__ import annotations

import json
from typing import Any, Mapping

from bench.live.manifest import canonical_json_bytes, manifest_digest, validate_manifest


CONTRACT = "sley2.live-environment-snapshot.v1"
FIELDS = frozenset(
    {
        "cache_state",
        "contract",
        "environment_manifest",
        "hardware_manifest",
        "model_configuration",
        "model_exact_version",
        "model_provider",
        "provider_executable_sha256",
        "provider_version",
        "repo_commit",
        "run_id",
        "run_manifest_digest",
    }
)
PROVIDER_ENVIRONMENT_FIELDS = frozenset(
    {
        "BASH_DEFAULT_TIMEOUT_MS",
        "BASH_MAX_TIMEOUT_MS",
        "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
        "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS",
        "CLAUDE_CODE_DISABLE_CLAUDE_MDS",
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
        "CLAUDE_CONFIG_DIR",
        "CODEX_HOME",
        "DISABLE_AUTOUPDATER",
        "DISABLE_ERROR_REPORTING",
        "DISABLE_TELEMETRY",
        "HOME",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "LANG",
        "LC_ALL",
        "NO_PROXY",
        "PATH",
        "SSL_CERT_DIR",
        "SSL_CERT_FILE",
        "TZ",
        "http_proxy",
        "https_proxy",
        "no_proxy",
    }
)
REQUIRED_PROVIDER_ENVIRONMENT_FIELDS = frozenset({"HOME", "PATH"})


class EnvironmentError(ValueError):
    """An attempt environment receipt is malformed or disagrees with its run."""


def provider_environment(manifest: Mapping[str, Any]) -> dict[str, str]:
    """Return the exact allowlisted environment passed to the provider process."""

    validate_manifest(manifest)
    value = manifest["environment_manifest"].get("provider_environment")
    if (
        not isinstance(value, dict)
        or not REQUIRED_PROVIDER_ENVIRONMENT_FIELDS <= set(value)
        or not set(value) <= PROVIDER_ENVIRONMENT_FIELDS
        or any(
            not isinstance(key, str)
            or not isinstance(item, str)
            or not item
            or "\x00" in key
            or "\x00" in item
            for key, item in value.items()
        )
    ):
        raise EnvironmentError("LIVE_PROVIDER_ENVIRONMENT_INVALID")
    return dict(value)


def _expected(manifest: Mapping[str, Any]) -> dict[str, Any]:
    validate_manifest(manifest)
    return {
        "cache_state": manifest["cache_state"],
        "contract": CONTRACT,
        "environment_manifest": manifest["environment_manifest"],
        "hardware_manifest": manifest["hardware_manifest"],
        "model_configuration": manifest["model_configuration"],
        "model_exact_version": manifest["model_exact_version"],
        "model_provider": manifest["model_provider"],
        "provider_executable_sha256": manifest["provider_executable_sha256"],
        "provider_version": manifest["provider_version"],
        "repo_commit": manifest["repo_commit"],
        "run_id": manifest["run_id"],
        "run_manifest_digest": manifest_digest(manifest),
    }


def environment_snapshot_bytes(manifest: Mapping[str, Any]) -> bytes:
    return canonical_json_bytes(_expected(manifest)) + b"\n"


def validate_environment_snapshot(payload: bytes, manifest: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(payload, bytes) or not payload.endswith(b"\n"):
        raise EnvironmentError("LIVE_ENVIRONMENT_INVALID: bytes")
    try:
        value = json.loads(payload)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise EnvironmentError("LIVE_ENVIRONMENT_INVALID: JSON") from error
    if not isinstance(value, dict) or set(value) != FIELDS:
        raise EnvironmentError("LIVE_ENVIRONMENT_INVALID: field set")
    if canonical_json_bytes(value) + b"\n" != payload:
        raise EnvironmentError("LIVE_ENVIRONMENT_INVALID: noncanonical")
    if value != _expected(manifest):
        raise EnvironmentError("LIVE_ENVIRONMENT_INVALID: manifest mismatch")
    return value
