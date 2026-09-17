"""Bounded, shell-free provider subprocess execution."""

from __future__ import annotations

import os
import signal
import subprocess
import threading
import time
from dataclasses import dataclass
from pathlib import Path
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
    peak_memory_bytes: int = 0


def _resident_bytes(process_id: int) -> int:
    """Sample resident bytes for one Linux process tree; races read as zero."""

    pending = [process_id]
    seen: set[int] = set()
    total = 0
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        try:
            status = (Path(f"/proc/{current}/status")).read_text(encoding="ascii")
            children = (Path(f"/proc/{current}/task/{current}/children")).read_text(
                encoding="ascii"
            )
        except (OSError, UnicodeError):
            continue
        for line in status.splitlines():
            if line.startswith("VmRSS:"):
                fields = line.split()
                if len(fields) >= 2 and fields[1].isdigit():
                    total += int(fields[1]) * 1024
                break
        pending.extend(int(child) for child in children.split() if child.isdigit())
    return total


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
    peak_memory_bytes = 0
    sampling_done = threading.Event()

    def sample_memory() -> None:
        nonlocal peak_memory_bytes
        while not sampling_done.is_set():
            peak_memory_bytes = max(peak_memory_bytes, _resident_bytes(process.pid))
            sampling_done.wait(0.01)
        peak_memory_bytes = max(peak_memory_bytes, _resident_bytes(process.pid))

    sampler = threading.Thread(target=sample_memory, name="sley2-live-rss", daemon=True)
    sampler.start()
    timed_out = False
    try:
        stdout, stderr = process.communicate(prompt, timeout=timeout_ms / 1000)
    except subprocess.TimeoutExpired:
        timed_out = True
        _kill_group(process)
        stdout, stderr = process.communicate()
    finally:
        sampling_done.set()
        sampler.join(timeout=1)
    wall_time_ms = max(0, (time.monotonic_ns() - started) // 1_000_000)
    if len(stdout) + len(stderr) > max_output_bytes:
        _fail("LIVE_PROVIDER_OUTPUT_LIMIT", str(len(stdout) + len(stderr)))
    return ProcessCapture(
        stdout=stdout,
        stderr=stderr,
        exit_code=124 if timed_out else process.returncode,
        timed_out=timed_out,
        wall_time_ms=wall_time_ms,
        peak_memory_bytes=peak_memory_bytes,
    )
