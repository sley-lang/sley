"""Pinned Codex CLI event boundary for an explicitly launched live trial."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any


MODEL = re.compile(r"[a-z0-9][a-z0-9._-]{0,127}\Z")
EFFORTS = frozenset({"low", "medium", "high", "xhigh", "max", "ultra"})
TOOL_ITEM_TYPES = frozenset(
    {
        "command_execution",
        "file_change",
        "mcp_tool_call",
        "web_search",
    }
)


class ProviderError(ValueError):
    """Malformed provider configuration or captured event stream."""


def _fail(symbol: str, detail: str = "") -> None:
    raise ProviderError(symbol if not detail else f"{symbol}: {detail}")


@dataclass(frozen=True)
class CodexEvents:
    events: tuple[dict[str, Any], ...]
    thread_id: str | None
    input_tokens: int
    cached_input_tokens: int
    output_tokens: int
    reasoning_output_tokens: int
    tool_calls: int
    final_message: str | None


def _nonnegative_int(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        _fail("LIVE_PROVIDER_EVENT_INVALID", field)
    return value


def parse_codex_jsonl(payload: bytes) -> CodexEvents:
    """Parse one complete `codex exec --json` stream without losing events."""

    if not isinstance(payload, bytes) or not payload:
        _fail("LIVE_PROVIDER_EVENT_INVALID", "empty stream")
    events: list[dict[str, Any]] = []
    for number, raw_line in enumerate(payload.splitlines(), 1):
        if not raw_line:
            _fail("LIVE_PROVIDER_EVENT_INVALID", f"blank line {number}")
        try:
            event = json.loads(raw_line)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise ProviderError(f"LIVE_PROVIDER_EVENT_INVALID: line {number}") from error
        if not isinstance(event, dict) or not isinstance(event.get("type"), str):
            _fail("LIVE_PROVIDER_EVENT_INVALID", f"shape {number}")
        events.append(event)
    terminals = [event for event in events if event["type"] in {"turn.completed", "turn.failed"}]
    if len(terminals) != 1 or terminals[0] is not events[-1]:
        _fail("LIVE_PROVIDER_EVENT_INVALID", "terminal")
    terminal = terminals[0]
    if terminal["type"] != "turn.completed":
        _fail("LIVE_PROVIDER_TURN_FAILED", str(terminal.get("error", ""))[:200])
    usage = terminal.get("usage")
    required_usage = {
        "input_tokens",
        "cached_input_tokens",
        "output_tokens",
        "reasoning_output_tokens",
    }
    if not isinstance(usage, dict) or not required_usage <= set(usage):
        _fail("LIVE_PROVIDER_EVENT_INVALID", "usage")
    values = {field: _nonnegative_int(usage[field], field) for field in required_usage}
    if values["cached_input_tokens"] > values["input_tokens"]:
        _fail("LIVE_PROVIDER_EVENT_INVALID", "cached_input_tokens")
    threads = [event.get("thread_id") for event in events if event["type"] == "thread.started"]
    if len(threads) > 1 or (threads and not isinstance(threads[0], str)):
        _fail("LIVE_PROVIDER_EVENT_INVALID", "thread")
    messages = [
        event.get("item", {}).get("text")
        for event in events
        if event["type"] == "item.completed"
        and isinstance(event.get("item"), dict)
        and event["item"].get("type") == "agent_message"
    ]
    if any(not isinstance(message, str) for message in messages):
        _fail("LIVE_PROVIDER_EVENT_INVALID", "agent message")
    tool_calls = sum(
        1
        for event in events
        if event["type"] == "item.completed"
        and isinstance(event.get("item"), dict)
        and event["item"].get("type") in TOOL_ITEM_TYPES
    )
    return CodexEvents(
        events=tuple(events),
        thread_id=threads[0] if threads else None,
        input_tokens=values["input_tokens"],
        cached_input_tokens=values["cached_input_tokens"],
        output_tokens=values["output_tokens"],
        reasoning_output_tokens=values["reasoning_output_tokens"],
        tool_calls=tool_calls,
        final_message=messages[-1] if messages else None,
    )


@dataclass(frozen=True)
class CodexExecAdapter:
    executable: str
    model: str
    reasoning_effort: str
    # Codex's own workspace-write sandbox forbids every socket connect,
    # AF_UNIX included, unless network access is enabled for it; the
    # mediated sley_2_0 tool reaches its gateway over a unix socket, so
    # the live campaign enables it for every arm alike. It is safe only
    # inside the outer provider sandbox (bench/live/provider_sandbox.py),
    # whose network namespace has no route out except the allowlisting
    # egress proxy that model commands are never pointed at.
    command_network_access: bool = False

    def __post_init__(self) -> None:
        if not self.executable or "\x00" in self.executable:
            _fail("LIVE_PROVIDER_CONFIG_INVALID", "executable")
        if (
            not isinstance(self.model, str)
            or MODEL.fullmatch(self.model) is None
            or "latest" in self.model.lower()
        ):
            _fail("LIVE_PROVIDER_CONFIG_INVALID", "model")
        if not isinstance(self.reasoning_effort, str) or self.reasoning_effort not in EFFORTS:
            _fail("LIVE_PROVIDER_CONFIG_INVALID", "reasoning_effort")
        if not isinstance(self.command_network_access, bool):
            _fail("LIVE_PROVIDER_CONFIG_INVALID", "command_network_access")

    def command(self, workspace: str | Path) -> list[str]:
        location = str(workspace)
        if not location or "\x00" in location:
            _fail("LIVE_PROVIDER_CONFIG_INVALID", "workspace")
        return [
            self.executable,
            "exec",
            "--json",
            "--strict-config",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--sandbox",
            "workspace-write",
            "--skip-git-repo-check",
            "--model",
            self.model,
            "--config",
            f'model_reasoning_effort="{self.reasoning_effort}"',
            "--config",
            'shell_environment_policy.inherit="none"',
            *(
                ["--config", "sandbox_workspace_write.network_access=true"]
                if self.command_network_access
                else []
            ),
            "--cd",
            location,
            "-",
        ]
