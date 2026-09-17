#!/usr/bin/env python3
"""Bounded legacy task runner for the S3 legacy arm (shared helper).

Runs the frozen Sley 1.2.0 artifact out-of-process through
``bench.legacy.runner.staged_frozen_artifact`` (which verifies the pinned
outer hash, archive safety, release identity, and every payload byte on
every staging) with the same containment posture as the S20-600 runner:

- scrubbed environment (no inheritance), private HOME/TMPDIR/source cache
- cwd is always the private stage root (fixture targets are absolute paths)
- bounded wall-clock timeout per child (30 s <= timeout <= 300 s)
- bounded retained output (combined stdout+stderr prefix cap)
- stdin is always DEVNULL, no shell, new process session + process-group
  kill on timeout/output-limit, short post-kill pipe-drain grace
- the archive is never copied into the repo tree; stages live in temp dirs

Subcommand coverage: run / check / verify / test (required) plus query /
lint / plan / graph / graph-diff / machine / doctor / ast, all bounded the
same way. Every helper returns a plain dict; harness errors raise
TaskRunnerError (oracle maps those to exit 2, never a verdict).
"""

from __future__ import annotations

import json
import os
import selectors
import signal
import subprocess
import time
from pathlib import Path

from bench.legacy.runner import staged_frozen_artifact

DEFAULT_TIMEOUT_SECONDS = 240.0
MIN_TIMEOUT_SECONDS = 30.0
MAX_TIMEOUT_SECONDS = 300.0
DEFAULT_OUTPUT_LIMIT_BYTES = 1024 * 1024
DRAIN_GRACE_SECONDS = 5.0


class TaskRunnerError(ValueError):
    """Harness-level failure (never an oracle verdict)."""


def _scrubbed_env(scratch_home: Path, scratch_tmp: Path, source_cache: Path) -> dict[str, str]:
    # PATH keeps the S20-600 system directories plus the pinned user node
    # toolchain directory. Rationale (engine fact): `sley machine` shells out
    # to `node` (NodeTextRuntime spawns `node -e ...`; without it every
    # machine invocation dies with FileNotFoundError). No system node exists
    # (/usr/bin/node absent); the pinned direct binary answers v26.8.1 under
    # a scrubbed environment (the mise shim does not: it needs trust/config).
    # check/run/verify/test/query/lint/plan/graph paths do not use node.
    # The pinned directory is appended only when it exists on the executing
    # host; elsewhere PATH stays at the system directories and machine
    # invocations fail closed with FileNotFoundError (harness error, never a
    # verdict), so a missing node toolchain can never silently pass.
    node_dir = "/home/gfarch/.local/share/mise/installs/node/26.8.1/bin"
    path = "/usr/bin:/bin"
    if Path(node_dir).is_dir():
        path += ":" + node_dir
    return {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "HOME": str(scratch_home),
        "LANG": "C",
        "LC_ALL": "C",
        "NO_COLOR": "1",
        "PATH": path,
        "PYTHONDONTWRITEBYTECODE": "1",
        "SLEY_DISABLE_SOURCE_CACHE": "0",
        "SLEY_SOURCE_CACHE_DIR": str(source_cache),
        "TMPDIR": str(scratch_tmp),
        "TZ": "UTC",
    }


def _check_timeout(timeout_seconds: float) -> None:
    if (
        isinstance(timeout_seconds, bool)
        or not isinstance(timeout_seconds, (int, float))
        or not (MIN_TIMEOUT_SECONDS <= float(timeout_seconds) <= MAX_TIMEOUT_SECONDS)
    ):
        raise TaskRunnerError(
            f"timeout {timeout_seconds!r} outside [{MIN_TIMEOUT_SECONDS},{MAX_TIMEOUT_SECONDS}]"
        )


def _spawn(
    argv: list[str],
    cwd: Path,
    env: dict[str, str],
    timeout_seconds: float,
    output_limit_bytes: int,
) -> dict:
    """Run one child with timeout + output cap + process-group kill."""
    started_monotonic = time.monotonic()
    deadline = started_monotonic + float(timeout_seconds)
    stdout_parts: list[bytes] = []
    stderr_parts: list[bytes] = []
    stdout_count = 0
    stderr_count = 0
    retained_total = 0
    termination: str | None = None
    spawn_error: str | None = None
    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    streams: dict[int, tuple[str, object]] = {}
    drain_deadline: float | None = None
    try:
        process = subprocess.Popen(
            argv,
            cwd=str(cwd),
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            shell=False,
            close_fds=True,
            start_new_session=True,
        )
        assert process.stdout is not None and process.stderr is not None
        for name, stream in (("stdout", process.stdout), ("stderr", process.stderr)):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
            streams[stream.fileno()] = (name, stream)
        import contextlib as _ctx

        while streams:
            now = time.monotonic()
            if termination is None and now >= deadline:
                termination = "timeout"
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            if termination is not None and drain_deadline is None:
                drain_deadline = now + DRAIN_GRACE_SECONDS
            if drain_deadline is not None and now >= drain_deadline:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                for _, stream in streams.values():
                    with _ctx.suppress(Exception):
                        stream.close()  # type: ignore[union-attr]
                    with _ctx.suppress(Exception):
                        selector.unregister(stream)  # type: ignore[arg-type]
                streams.clear()
                break
            pace = deadline - now
            if drain_deadline is not None:
                pace = min(pace, drain_deadline - now)
            events = selector.select(timeout=max(0.0, min(0.05, pace)))
            if not events and process.poll() is not None:
                events = [
                    (type("ReadyKey", (), {"fileobj": stream, "data": name})(), None)
                    for name, stream in streams.values()
                ]
            for key, _ in events:
                stream = key.fileobj
                name = key.data
                try:
                    chunk = os.read(stream.fileno(), 64 * 1024)
                except BlockingIOError:
                    continue
                if not chunk:
                    with _ctx.suppress(Exception):
                        selector.unregister(stream)
                    streams.pop(stream.fileno(), None)
                    stream.close()
                    continue
                if name == "stdout":
                    stdout_count += len(chunk)
                else:
                    stderr_count += len(chunk)
                remaining = max(0, output_limit_bytes - retained_total)
                kept = chunk[:remaining]
                retained_total += len(kept)
                (stdout_parts if name == "stdout" else stderr_parts).append(kept)
                if (
                    termination is None
                    and stdout_count + stderr_count > output_limit_bytes
                ):
                    termination = "output_limit"
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
        return_code: int | None = None
        remaining = max(0.0, deadline - time.monotonic()) if termination is None else 1.0
        try:
            return_code = process.wait(timeout=remaining)
        except subprocess.TimeoutExpired:
            if termination is None:
                termination = "timeout"
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                return_code = process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                return_code = process.returncode
    except (OSError, subprocess.SubprocessError) as error:
        spawn_error = str(error)
        if process is not None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=1)
            except Exception:
                pass
        return_code = process.returncode if process is not None else None
    finally:
        selector.close()
        for _, stream in streams.values():
            try:
                stream.close()  # type: ignore[union-attr]
            except Exception:
                pass
    stdout_bytes = b"".join(stdout_parts)
    stderr_bytes = b"".join(stderr_parts)
    outcome = "completed"
    if spawn_error is not None:
        outcome = "spawn_failure"
    elif termination == "timeout":
        outcome = "timeout"
    elif termination == "output_limit":
        outcome = "output_limit"
    return {
        "argv_tail": argv[1:],
        "return_code": return_code,
        "outcome": outcome,
        "spawn_error": spawn_error,
        "stdout": stdout_bytes,
        "stderr": stderr_bytes,
        "stdout_bytes": stdout_count,
        "stderr_bytes": stderr_count,
        "truncated": retained_total != stdout_count + stderr_count,
    }


def _try_json(payload: bytes):
    try:
        return json.loads(payload.decode("utf-8"))
    except Exception:
        return None


class LegacySession:
    """One staged artifact + warm source cache shared by many calls."""

    def __init__(self, stage) -> None:
        self._stage = stage
        self._sley = str(stage.root / "bin/sley")
        scratch = stage.scratch
        self._env = _scrubbed_env(scratch / "home", scratch / "tmp", scratch / "source-cache")
        self._cwd = stage.root

    def invoke(
        self,
        args: list[str],
        *,
        timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
        output_limit_bytes: int = DEFAULT_OUTPUT_LIMIT_BYTES,
        stdin_text: str | None = None,
    ) -> dict:
        _check_timeout(timeout_seconds)
        if not isinstance(output_limit_bytes, int) or not 1024 <= output_limit_bytes <= 4 * 1024 * 1024:
            raise TaskRunnerError(f"bad output limit {output_limit_bytes!r}")
        if stdin_text is not None:
            return self._invoke_with_stdin(args, stdin_text, timeout_seconds, output_limit_bytes)
        raw = _spawn([self._sley, *args], self._cwd, self._env, float(timeout_seconds), output_limit_bytes)
        return self._shape(args, raw)

    def _invoke_with_stdin(
        self, args: list[str], stdin_text: str, timeout_seconds: float, output_limit_bytes: int
    ) -> dict:
        # Bounded stdin variant for `sley machine` (single small request line).
        data = stdin_text.encode("utf-8")
        if len(data) > 65536:
            raise TaskRunnerError("stdin request too large")
        deadline = time.monotonic() + float(timeout_seconds)
        try:
            process = subprocess.Popen(
                [self._sley, *args],
                cwd=str(self._cwd),
                env=self._env,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                shell=False,
                close_fds=True,
                start_new_session=True,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise TaskRunnerError(f"spawn failed: {error}") from error
        try:
            remaining = max(1.0, deadline - time.monotonic())
            out, err = process.communicate(data, timeout=remaining)
        except subprocess.TimeoutExpired as error:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            raise TaskRunnerError("child timeout") from error
        raw = {
            "argv_tail": args,
            "return_code": process.returncode,
            "outcome": "completed",
            "spawn_error": None,
            "stdout": out[:output_limit_bytes],
            "stderr": err[:output_limit_bytes],
            "stdout_bytes": len(out),
            "stderr_bytes": len(err),
            "truncated": len(out) + len(err) > output_limit_bytes,
        }
        return self._shape(args, raw)

    def _shape(self, args: list[str], raw: dict) -> dict:
        result = dict(raw)
        result["command"] = args[0] if args else ""
        result["report"] = _try_json(raw["stdout"])
        result["stderr_text"] = raw["stderr"].decode("utf-8", errors="replace")
        return result

    # ---- convenience wrappers (all bounded, all inside this stage) ----
    def check(self, target: str | Path, **kw) -> dict:
        return self.invoke(["check", "--json", str(target)], **kw)

    def run(self, target: str | Path, *caps: str, **kw) -> dict:
        args: list[str] = ["run", "--json"]
        for cap in caps:
            args += ["--cap", cap]
        args.append(str(target))
        return self.invoke(args, **kw)

    def verify(self, target: str | Path, *extra: str, **kw) -> dict:
        return self.invoke(["verify", "--json", *extra, str(target)], **kw)

    def test(self, target: str | Path, **kw) -> dict:
        return self.invoke(["test", "--json", str(target)], **kw)

    def query(self, target: str | Path, kind: str = "all", module: str | None = None, **kw) -> dict:
        args = ["query", "--json", "--kind", kind]
        if module:
            args += ["--module", module]
        args.append(str(target))
        return self.invoke(args, **kw)

    def lint(self, target: str | Path, **kw) -> dict:
        return self.invoke(["lint", "--json", str(target)], **kw)

    def plan(self, target: str | Path, **kw) -> dict:
        return self.invoke(["plan", "--json", str(target)], **kw)

    def graph_slice(self, target: str | Path, surface: str, **kw) -> dict:
        return self.invoke(["graph", "--json", "--slice", surface, str(target)], **kw)

    def graph_diff(self, base: str | Path, ours: str | Path, theirs: str | Path, **kw) -> dict:
        return self.invoke(
            ["graph-diff", "--json", "--base", str(base), "--ours", str(ours), "--theirs", str(theirs)],
            **kw,
        )

    def machine_invoke(
        self,
        source: str | Path,
        task: str,
        request_input: dict,
        package_id: str = "legacy-fixture",
        package_version: str = "1.0.0",
        **kw,
    ) -> dict:
        """Two-step machine invoke: learn canonical pins, then execute."""
        source = str(source)
        probe = {
            "protocolVersion": "sley.machine.invoke.v0",
            "requestId": "pin-probe",
            "operation": "invoke",
            "packageId": package_id,
            "packageVersion": package_version,
            "task": task,
            "input": request_input,
            "pins": {
                "sourceDigest": "sha256:" + "0" * 64,
                "runtimeId": "sley.stage1.machine",
                "runtimeVersion": "sley 1.2.0",
                "runtimeDigest": "sha256:" + "0" * 64,
            },
        }
        first = self.invoke(
            ["machine", "--json", "--source", source, "--package-id", package_id,
             "--package-version", package_version],
            stdin_text=json.dumps(probe) + "\n",
            **kw,
        )
        report = first.get("report") or {}
        source_digest = (report.get("source") or {}).get("digest")
        runtime = report.get("runtime") or {}
        if not isinstance(source_digest, str) or not isinstance(runtime.get("digest"), str):
            raise TaskRunnerError(f"machine pin probe failed: rc={first['return_code']}")
        request = {
            "protocolVersion": "sley.machine.invoke.v0",
            "requestId": "invoke-1",
            "operation": "invoke",
            "packageId": package_id,
            "packageVersion": package_version,
            "task": task,
            "input": request_input,
            "pins": {
                "sourceDigest": source_digest,
                "runtimeId": runtime.get("id", "sley.stage1.machine"),
                "runtimeVersion": runtime.get("version", "sley 1.2.0"),
                "runtimeDigest": runtime["digest"],
            },
        }
        return self.invoke(
            ["machine", "--json", "--source", source, "--package-id", package_id,
             "--package-version", package_version],
            stdin_text=json.dumps(request) + "\n",
            **kw,
        )


def run_session(body, *, timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS):
    """Stage the frozen artifact (verified every run) and run body(session)."""
    _check_timeout(timeout_seconds)
    with staged_frozen_artifact() as stage:
        return body(LegacySession(stage))
