"""Bounded, shell-free provider subprocess execution."""

from __future__ import annotations

import os
import signal
import subprocess
import time
from dataclasses import dataclass
from typing import Mapping, Sequence


MAX_TIMEOUT_MS = 86_400_000
MAX_CAPTURE_BYTES = 512 * 1024 * 1024


class ProcessError(ValueError):
    """Provider process configuration, spawn, or capture failed."""


def _fail(symbol: str, detail: str = "") -> None:
    raise ProcessError(symbol if not detail else f"{symbol}: {detail}")


@dataclass(frozen=True)
class ProcessCapture:
    stdout: bytes
    stderr: bytes
    exit_code: int
    timed_out: bool
    wall_time_ms: int


def _kill_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def run_provider_process(
    argv: Sequence[str],
    prompt: bytes,
    *,
    timeout_ms: int,
    max_output_bytes: int,
    environment: Mapping[str, str] | None = None,
) -> ProcessCapture:
    """Execute one frozen provider command and retain its exact byte streams."""

    if (
        not isinstance(argv, (list, tuple))
        or not argv
        or any(not isinstance(item, str) or not item or "\x00" in item for item in argv)
        or not isinstance(prompt, bytes)
        or isinstance(timeout_ms, bool)
        or not isinstance(timeout_ms, int)
        or not 1 <= timeout_ms <= MAX_TIMEOUT_MS
        or isinstance(max_output_bytes, bool)
        or not isinstance(max_output_bytes, int)
        or not 1 <= max_output_bytes <= MAX_CAPTURE_BYTES
    ):
        _fail("LIVE_PROVIDER_PROCESS_INVALID")
    if environment is not None and any(
        not isinstance(key, str)
        or not isinstance(value, str)
        or "\x00" in key
        or "\x00" in value
        for key, value in environment.items()
    ):
        _fail("LIVE_PROVIDER_PROCESS_INVALID", "environment")
    started = time.monotonic_ns()
    try:
        process = subprocess.Popen(
            list(argv),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=None if environment is None else dict(environment),
            start_new_session=True,
        )
    except OSError as error:
        raise ProcessError(f"LIVE_PROVIDER_SPAWN_FAILED: {error}") from error
    timed_out = False
    try:
        stdout, stderr = process.communicate(prompt, timeout=timeout_ms / 1000)
    except subprocess.TimeoutExpired:
        timed_out = True
        _kill_group(process)
        stdout, stderr = process.communicate()
    wall_time_ms = max(0, (time.monotonic_ns() - started) // 1_000_000)
    if len(stdout) + len(stderr) > max_output_bytes:
        _fail("LIVE_PROVIDER_OUTPUT_LIMIT", str(len(stdout) + len(stderr)))
    return ProcessCapture(
        stdout=stdout,
        stderr=stderr,
        exit_code=124 if timed_out else process.returncode,
        timed_out=timed_out,
        wall_time_ms=wall_time_ms,
    )
