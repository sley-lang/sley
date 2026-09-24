"""Strict subprocess boundary for the frozen per-task fixture oracles."""

from __future__ import annotations

import json
import os
import re
import signal
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "bench" / "fixtures"
FIELDS = frozenset({"arm", "code", "detail", "status", "task_id"})
ARM_NAMES = {"raw_files": "raw", "sley_1_2_0": "legacy", "sley_2_0": "sley2"}
CODE = re.compile(r"[A-Z][A-Z0-9_]{0,127}\Z")
MAX_OUTPUT_BYTES = 1_048_576


class OracleError(ValueError):
    """An oracle process or verdict failed the live evidence boundary."""


def _fail(detail: str) -> None:
    raise OracleError(f"LIVE_ORACLE_INVALID: {detail}")


def parse_fixture_oracle(
    stdout: bytes,
    *,
    exit_code: int,
    expected_arm: str,
    expected_task: str,
) -> dict[str, Any]:
    if expected_arm not in ARM_NAMES:
        _fail("arm")
    if not isinstance(stdout, bytes) or len(stdout) > MAX_OUTPUT_BYTES:
        _fail("stdout")
    lines = stdout.splitlines()
    if len(lines) != 1 or not lines[0]:
        _fail("one output line required")
    try:
        verdict = json.loads(lines[0])
    except (UnicodeError, json.JSONDecodeError) as error:
        raise OracleError("LIVE_ORACLE_INVALID: JSON") from error
    if not isinstance(verdict, dict) or set(verdict) != FIELDS:
        _fail("field set")
    if verdict["arm"] != ARM_NAMES[expected_arm] or verdict["task_id"] != expected_task:
        _fail("identity")
    detail = verdict["detail"]
    if not isinstance(detail, str) or not detail or "\n" in detail or len(detail.encode("utf-8")) > 4096:
        _fail("detail")
    status = verdict["status"]
    code = verdict["code"]
    if status == "accepted":
        if exit_code != 0 or code is not None:
            _fail("accepted mapping")
    elif status == "rejected":
        if exit_code != 1 or not isinstance(code, str) or CODE.fullmatch(code) is None:
            _fail("rejected mapping")
    else:
        _fail("status")
    return {
        "arm_id": expected_arm,
        "code": code,
        "detail": detail,
        "status": status,
        "task_id": expected_task,
    }


def run_fixture_oracle(
    *,
    arm_id: str,
    task_id: str,
    candidate: Path,
    timeout_seconds: int = 300,
) -> tuple[dict[str, Any], bytes, bytes]:
    """Run one frozen oracle against the exact disposable candidate directory."""

    if arm_id not in ARM_NAMES:
        _fail("fixture process arm")
    if isinstance(timeout_seconds, bool) or not isinstance(timeout_seconds, int) or not 1 <= timeout_seconds <= 300:
        _fail("timeout")
    if arm_id == "sley_2_0":
        # Live-trial judging lives beside (never inside) the frozen S3
        # conformance oracles: per-task entry, shared judge module.
        oracle = FIXTURES / ARM_NAMES[arm_id] / task_id / "live_oracle.py"
    else:
        oracle = FIXTURES / ARM_NAMES[arm_id] / task_id / "oracle.py"
    candidate = Path(candidate)
    try:
        oracle = oracle.resolve(strict=True)
        candidate = candidate.resolve(strict=True)
    except OSError as error:
        raise OracleError(f"LIVE_ORACLE_INVALID: {error}") from error
    if not oracle.is_file() or not candidate.is_dir():
        _fail("path")
    environment = {
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "PATH": "/usr/bin:/bin",
        "PYTHONHASHSEED": "0",
        "SLEY2_LIVE_ORACLE_CANDIDATE": str(candidate),
        "TZ": "UTC",
    }
    if arm_id == "sley_2_0" and os.environ.get("SLEY2_SLEY_BINARY"):
        # The live judge drives scratch serve sessions with the bound
        # binary; raw and legacy oracles never see this variable.
        environment["SLEY2_SLEY_BINARY"] = os.environ["SLEY2_SLEY_BINARY"]
    if arm_id == "sley_2_0" and os.environ.get("SUCC_JUDGE_TEST_BINARY"):
        # Frozen Rust case-driver binary for value-level strict cases.
        environment["SUCC_JUDGE_TEST_BINARY"] = os.environ["SUCC_JUDGE_TEST_BINARY"]
    if arm_id == "sley_2_0" and os.environ.get("SLEY2_MEDIATED_CAPTURE_DIR"):
        # Runner-controlled reconciled capture for this attempt: the
        # judge derives mediated access/budget evidence from it.
        environment["SLEY2_MEDIATED_CAPTURE_DIR"] = os.environ[
            "SLEY2_MEDIATED_CAPTURE_DIR"]
    try:
        process = subprocess.Popen(
            [sys.executable, str(oracle), str(candidate)],
            cwd=ROOT,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
    except OSError as error:
        raise OracleError(f"LIVE_ORACLE_INVALID: spawn: {error}") from error
    try:
        stdout, stderr = process.communicate(timeout=timeout_seconds)
    except subprocess.TimeoutExpired as error:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        raise OracleError("LIVE_ORACLE_INVALID: timeout") from error
    if len(stdout) > MAX_OUTPUT_BYTES or len(stderr) > MAX_OUTPUT_BYTES:
        _fail("output limit")
    verdict = parse_fixture_oracle(
        stdout,
        exit_code=process.returncode,
        expected_arm=arm_id,
        expected_task=task_id,
    )
    return verdict, stdout, stderr
