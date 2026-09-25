#!/usr/bin/env python3
"""Model-facing thin CLI over the exact frozen Sley 1.2 task runner."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

from bench.legacy.task_runner import TaskRunnerError, run_session


RESULT_FIELDS = (
    "command",
    "outcome",
    "return_code",
    "report",
    "stderr_text",
    "stdout_bytes",
    "stderr_bytes",
    "truncated",
)


class LegacyToolError(ValueError):
    """A live legacy-tool request exceeded its exact command boundary."""


def _fail(detail: str) -> None:
    raise LegacyToolError(f"LIVE_LEGACY_TOOL_INVALID: {detail}")


def _target(root: Path, value: str) -> Path:
    if not isinstance(value, str) or not value or "\x00" in value:
        _fail("path")
    relative = Path(value)
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        _fail("path")
    candidate = root.joinpath(*relative.parts)
    try:
        if candidate.is_symlink() or not candidate.is_file() or candidate.resolve(strict=True).parent != root.resolve():
            _fail("path")
    except OSError as error:
        raise LegacyToolError(f"LIVE_LEGACY_TOOL_INVALID: {error}") from error
    return candidate


def _safe_result(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict) or any(field not in result for field in RESULT_FIELDS):
        _fail("runner result")
    safe = {field: result[field] for field in RESULT_FIELDS}
    try:
        json.dumps(safe, ensure_ascii=False, allow_nan=False)
    except (TypeError, ValueError) as error:
        raise LegacyToolError("LIVE_LEGACY_TOOL_INVALID: non-JSON result") from error
    return safe


def dispatch(session: Any, argv: list[str], workspace: Path) -> dict[str, Any]:
    """Dispatch one documented read/check operation against candidate files."""

    if not isinstance(argv, list) or not argv or any(not isinstance(arg, str) or "\x00" in arg for arg in argv):
        _fail("arguments")
    root = Path(workspace).resolve(strict=True)
    command = argv[0]
    if command in {"check", "test", "lint", "plan"} and len(argv) == 2:
        result = getattr(session, command)(_target(root, argv[1]))
    elif command == "run" and len(argv) >= 2:
        result = session.run(_target(root, argv[1]), *argv[2:])
    elif command == "verify" and len(argv) >= 2:
        result = session.verify(_target(root, argv[1]), *argv[2:])
    elif command == "query" and 2 <= len(argv) <= 4:
        kind = argv[2] if len(argv) >= 3 else "all"
        module = argv[3] if len(argv) == 4 else None
        result = session.query(_target(root, argv[1]), kind=kind, module=module)
    elif command == "graph" and len(argv) == 3:
        result = session.graph_slice(_target(root, argv[1]), argv[2])
    elif command == "graph-diff" and len(argv) == 4:
        result = session.graph_diff(
            _target(root, argv[1]),
            _target(root, argv[2]),
            _target(root, argv[3]),
        )
    elif command == "machine" and len(argv) == 4:
        try:
            request = json.loads(argv[3])
        except json.JSONDecodeError as error:
            raise LegacyToolError("LIVE_LEGACY_TOOL_INVALID: machine JSON") from error
        if not isinstance(request, dict):
            _fail("machine JSON")
        result = session.machine_invoke(_target(root, argv[1]), argv[2], request)
    else:
        _fail("command")
    return _safe_result(result)


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    try:
        result = run_session(lambda session: dispatch(session, arguments, Path.cwd()))
        print(json.dumps(result, ensure_ascii=False, allow_nan=False, sort_keys=True))
        return 0 if result["outcome"] == "completed" and result["return_code"] == 0 else 1
    except (LegacyToolError, TaskRunnerError, OSError, ValueError) as error:
        print(
            json.dumps(
                {"code": "LIVE_LEGACY_TOOL_INVALID", "detail": str(error)[:500]},
                sort_keys=True,
            )
        )
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
