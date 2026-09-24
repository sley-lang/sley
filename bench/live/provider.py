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
    # Provider-normalized tool observations for metric derivation; None
    # means "derive from Codex command_execution items" (the Codex parser).
    # Each item: command (shell text or None), aggregated_output (text the
    # agent received), exit_code (0 ok, 1 failed), read_paths (explicit
    # file-read operands of a dedicated read tool).
    commands: tuple[dict[str, Any], ...] | None = None


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
    model_provider = "openai-chatgpt-oauth"
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


CLAUDE_TOOLS = "Bash,Read,Edit,Write,Glob,Grep"
CLAUDE_EFFORTS = frozenset({"low", "medium", "high", "xhigh", "max"})


def _tool_result_text(content: Any) -> str:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "".join(part.get("text", "") for part in content
                       if isinstance(part, dict) and isinstance(part.get("text"), str))
    return ""


def parse_claude_stream_json(payload: bytes) -> CodexEvents:
    """Parse one complete `claude -p --output-format stream-json --verbose`
    stream into the provider-neutral observation.

    Usage is the terminal ``result`` record's session totals: input tokens
    are every input token the model processed (uncached input plus cache
    creation plus cache reads, as Codex's input count includes its cached
    tokens); cached input is the cache reads. Tool calls are the
    ``tool_use`` blocks the model issued; each is paired with the
    ``tool_result`` it received.
    """

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
    # The session may continue past a result (for example a queued
    # notification starts another query); the stream must still end on a
    # result record, whose session-cumulative ``modelUsage`` covers every
    # query of the session (``usage`` covers only the last query).
    if not events or events[-1]["type"] != "result":
        _fail("LIVE_PROVIDER_EVENT_INVALID", "terminal")
    result = events[-1]
    if result.get("is_error") is not False or result.get("subtype") != "success":
        _fail("LIVE_PROVIDER_TURN_FAILED", str(result.get("result", result.get("subtype", "")))[:200])
    usage = result.get("usage")
    fields = ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens", "output_tokens")
    if not isinstance(usage, dict) or not set(fields) <= set(usage):
        _fail("LIVE_PROVIDER_EVENT_INVALID", "usage")
    values = {field: _nonnegative_int(usage[field], field) for field in fields}
    by_model = result.get("modelUsage")
    if by_model is not None:
        if not isinstance(by_model, dict) or not by_model:
            _fail("LIVE_PROVIDER_EVENT_INVALID", "modelUsage")
        totals = dict.fromkeys(fields, 0)
        for entry in by_model.values():
            if not isinstance(entry, dict):
                _fail("LIVE_PROVIDER_EVENT_INVALID", "modelUsage entry")
            for field, key in (("input_tokens", "inputTokens"),
                               ("cache_creation_input_tokens", "cacheCreationInputTokens"),
                               ("cache_read_input_tokens", "cacheReadInputTokens"),
                               ("output_tokens", "outputTokens")):
                totals[field] += _nonnegative_int(entry.get(key, 0), key)
        if any(totals[field] < values[field] for field in fields):
            _fail("LIVE_PROVIDER_EVENT_INVALID", "modelUsage below last-query usage")
        values = totals
    details = usage.get("output_tokens_details")
    reasoning = details.get("thinking_tokens", 0) if isinstance(details, dict) else 0
    reasoning = _nonnegative_int(reasoning, "thinking_tokens")
    uses: dict[str, dict[str, Any]] = {}
    order: list[str] = []
    outputs: dict[str, tuple[str, bool]] = {}
    for event in events:
        message = event.get("message")
        if not isinstance(message, dict) or not isinstance(message.get("content"), list):
            continue
        for block in message["content"]:
            if not isinstance(block, dict):
                continue
            if event["type"] == "assistant" and block.get("type") == "tool_use":
                identifier = block.get("id")
                if not isinstance(identifier, str) or identifier in uses:
                    _fail("LIVE_PROVIDER_EVENT_INVALID", "tool_use id")
                uses[identifier] = block
                order.append(identifier)
            elif event["type"] == "user" and block.get("type") == "tool_result":
                identifier = block.get("tool_use_id")
                if isinstance(identifier, str):
                    outputs[identifier] = (_tool_result_text(block.get("content")),
                                           bool(block.get("is_error")))
    commands = []
    for identifier in order:
        block = uses[identifier]
        arguments = block.get("input") if isinstance(block.get("input"), dict) else {}
        text, failed = outputs.get(identifier, ("", False))
        name = block.get("name")
        command = arguments.get("command") if name == "Bash" and isinstance(arguments.get("command"), str) else None
        read_path = arguments.get("file_path") if name == "Read" and isinstance(arguments.get("file_path"), str) else None
        commands.append({"aggregated_output": text, "command": command,
                         "exit_code": 1 if failed else 0, "name": name,
                         "read_paths": [read_path] if read_path else []})
    session = result.get("session_id")
    final = result.get("result")
    return CodexEvents(
        events=tuple(events),
        thread_id=session if isinstance(session, str) else None,
        input_tokens=values["input_tokens"] + values["cache_creation_input_tokens"]
        + values["cache_read_input_tokens"],
        cached_input_tokens=values["cache_read_input_tokens"],
        output_tokens=values["output_tokens"],
        reasoning_output_tokens=reasoning,
        tool_calls=len(order),
        final_message=final if isinstance(final, str) else None,
        commands=tuple(commands),
    )


PARSERS = {
    "openai-chatgpt-oauth": parse_codex_jsonl,
    "anthropic-claude-code-oauth": parse_claude_stream_json,
}


def parse_provider_events(model_provider: str, payload: bytes) -> CodexEvents:
    parser = PARSERS.get(model_provider)
    if parser is None:
        _fail("LIVE_PROVIDER_CONFIG_INVALID", "model_provider")
    return parser(payload)


@dataclass(frozen=True)
class ClaudeCodeAdapter:
    """``claude -p`` with a pinned model, run inside the outer provider
    sandbox (the security boundary): permission prompts are bypassed so the
    agent can use its tools there, the tool set is fixed to the six
    workspace tools (no web, no subagents, no MCP, no skills), and no
    settings source is loaded."""

    executable: str
    model: str
    reasoning_effort: str
    model_provider = "anthropic-claude-code-oauth"

    def __post_init__(self) -> None:
        if not self.executable or "\x00" in self.executable:
            _fail("LIVE_PROVIDER_CONFIG_INVALID", "executable")
        if (not isinstance(self.model, str) or MODEL.fullmatch(self.model) is None
                or "latest" in self.model.lower()):
            _fail("LIVE_PROVIDER_CONFIG_INVALID", "model")
        if not isinstance(self.reasoning_effort, str) or self.reasoning_effort not in CLAUDE_EFFORTS:
            _fail("LIVE_PROVIDER_CONFIG_INVALID", "reasoning_effort")

    def command(self, workspace: str | Path) -> list[str]:
        # The working directory is the sandbox chdir (claude has no --cd);
        # the argument is accepted for adapter symmetry.
        location = str(workspace)
        if not location or "\x00" in location:
            _fail("LIVE_PROVIDER_CONFIG_INVALID", "workspace")
        return [
            self.executable,
            "-p",
            "--output-format", "stream-json",
            "--verbose",
            "--model", self.model,
            "--effort", self.reasoning_effort,
            "--tools", CLAUDE_TOOLS,
            "--permission-mode", "bypassPermissions",
            "--no-session-persistence",
            "--setting-sources=",
            "--strict-mcp-config",
            "--disable-slash-commands",
        ]
