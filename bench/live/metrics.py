"""Evidence-derived provider metrics for live campaign attempts."""

from __future__ import annotations

import re
import shlex
from pathlib import PurePosixPath
from typing import Any

from bench.live.provider import CodexEvents


_CHECK = re.compile(
    r"(?:^|[;&|]\s*|\s)(?:"
    r"python(?:3)?\s+-m\s+(?:unittest|pytest)|"
    r"pytest|cargo\s+(?:check|test|clippy)|"
    r"(?:\.?/?[^\s;&|]*/)?sley-tool\s+(?:check|test|verify)|"
    r"make\s+(?:check|test|quick|gate|pre-candidate)"
    r")(?:\s|$)"
)
_READ_COMMANDS = frozenset({"cat", "head", "sed", "tail"})
_SHELL_SEPARATORS = frozenset({"&&", "||", ";", "|"})


def _commands(events: CodexEvents) -> list[dict[str, Any]]:
    return [
        event["item"]
        for event in events.events
        if event.get("type") == "item.completed"
        and isinstance(event.get("item"), dict)
        and event["item"].get("type") == "command_execution"
    ]


def _output_bytes(item: dict[str, Any]) -> int:
    # Current Codex JSONL calls this `aggregated_output`. The two aliases are
    # admitted for provider-version compatibility and each field is counted at
    # most once by selecting the first present spelling.
    for field in ("aggregated_output", "output", "result"):
        value = item.get(field)
        if isinstance(value, str):
            return len(value.encode("utf-8"))
    return 0


def _explicit_read_paths(command: str) -> set[str]:
    """Return path operands exposed by simple, observable read commands.

    This is deliberately a lower bound. It never infers reads from command
    output, globs, recursive tools, or shell side effects.
    """

    try:
        tokens = shlex.split(command, posix=True)
    except ValueError:
        return set()
    paths: set[str] = set()
    index = 0
    while index < len(tokens):
        executable = PurePosixPath(tokens[index]).name
        if executable not in _READ_COMMANDS:
            index += 1
            continue
        index += 1
        while index < len(tokens) and tokens[index] not in _SHELL_SEPARATORS:
            operand = tokens[index]
            if not operand.startswith("-") and operand not in {"/dev/null", "-"}:
                paths.add(operand)
            index += 1
    return paths


def derive_provider_observation(events: CodexEvents, prompt: bytes) -> dict[str, int]:
    """Derive the subset of benchmark metrics exposed by Codex JSONL.

    `files_inspected` is the exact count of unique literal operands to simple
    read commands in completed command events. It is an auditable lower bound,
    not an inferred filesystem-access count.
    """

    if not isinstance(prompt, bytes):
        raise ValueError("LIVE_METRIC_INVALID: prompt")
    commands = _commands(events)
    checks = [item for item in commands if isinstance(item.get("command"), str) and _CHECK.search(item["command"])]
    failed_checks = [
        item
        for item in checks
        if isinstance(item.get("exit_code"), int) and not isinstance(item.get("exit_code"), bool) and item["exit_code"] != 0
    ]
    read_paths: set[str] = set()
    for item in commands:
        command = item.get("command")
        if isinstance(command, str):
            read_paths.update(_explicit_read_paths(command))
    context_bytes = len(prompt) + sum(_output_bytes(item) for item in commands)
    return {
        "compile_or_check_attempts": len(checks),
        "context_bytes": context_bytes,
        "files_inspected": len(read_paths),
        "invalid_candidates": len(failed_checks),
        "model_input_tokens": events.input_tokens,
        "model_output_tokens": events.output_tokens,
        "repair_loops": len(failed_checks),
        "tool_calls": events.tool_calls,
        "total_observable_tokens": events.input_tokens + events.output_tokens,
    }
