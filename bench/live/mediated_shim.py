#!/usr/bin/env python3
"""Confined one-shot `sley-tool` replacement for mediated sley_2_0 trials.

Stdlib only. Staged by the runner into the agent scratch; runs inside
the bwrap sandbox with no protected filesystem. The CLI contract
mirrors the documented tool surface (`sley-tool <command> [args...]`);
each invocation forwards exactly one frame over the runner-provided
unix socket ($SLEY2_GATEWAY_SOCK) and prints the gateway's reply
envelope as JSON. Transport failures, uncaptured replies, and
server-side refusals are tool errors (exit 2), never successes.

Phase mapping mirrors the deterministic adapter: reads run in the
read phase, validating composition in compose, finish in finish.
"""

from __future__ import annotations

import json
import os
import socket
import sys

READ_COMMANDS = frozenset({
    "inventory", "side", "read", "sig", "open", "revision", "caps", "budgets",
    "raw", "inspect",
})
COMPOSE_COMMANDS = frozenset({
    "validate", "append", "compose", "propose", "resolve",
})

SOCK_ENV = "SLEY2_GATEWAY_SOCK"
FRAME_LIMIT = 8 * 1024 * 1024


def _phase(command: str) -> str:
    if command in READ_COMMANDS:
        return "read"
    if command in COMPOSE_COMMANDS:
        return "compose"
    if command == "finish":
        return "finish"
    return "read"


def _reply(payload: dict) -> int:
    sys.stdout.write(json.dumps(payload, sort_keys=True) + "\n")
    sys.stdout.flush()
    return 0 if payload.get("ok") is True else 2


def main() -> int:
    if len(sys.argv) < 2 or not sys.argv[1]:
        return _reply({"ok": False, "error": "SLEY2_SHIM_ARGV",
                       "detail": "usage: sley-tool <command> [args...]"})
    command = sys.argv[1]
    args = sys.argv[2:]
    if any(not isinstance(a, str) or "\x00" in a for a in args):
        return _reply({"ok": False, "error": "SLEY2_SHIM_ARGV",
                       "detail": "argument encoding"})
    sock_path = os.environ.get(SOCK_ENV, "")
    if not sock_path or "\x00" in sock_path:
        return _reply({"ok": False, "error": "SLEY2_SHIM_TRANSPORT",
                       "detail": "gateway socket unbound"})
    frame = {"phase": _phase(command),
             "session_id": f"shim-{os.getpid()}",
             "command": command, "args": args}
    try:
        raw = (json.dumps(frame, sort_keys=True) + "\n").encode("utf-8")
    except (UnicodeError, ValueError):
        return _reply({"ok": False, "error": "SLEY2_SHIM_ARGV",
                       "detail": "frame encoding"})
    if len(raw) > FRAME_LIMIT:
        return _reply({"ok": False, "error": "SLEY2_SHIM_FRAME_LIMIT",
                       "detail": str(len(raw))})
    try:
        client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            client.connect(sock_path)
            client.sendall(raw)
            reader = client.makefile("rb")
            line = reader.readline(FRAME_LIMIT + 2)
        finally:
            client.close()
    except OSError as error:
        return _reply({"ok": False, "error": "SLEY2_SHIM_TRANSPORT",
                       "detail": f"{type(error).__name__}"[:120]})
    if not line or len(line) > FRAME_LIMIT + 1:
        return _reply({"ok": False, "error": "SLEY2_SHIM_TRANSPORT",
                       "detail": "reply framing"})
    try:
        reply = json.loads(line.decode("utf-8"))
    except (UnicodeError, ValueError):
        return _reply({"ok": False, "error": "SLEY2_SHIM_TRANSPORT",
                       "detail": "reply JSON"})
    if not isinstance(reply, dict):
        return _reply({"ok": False, "error": "SLEY2_SHIM_TRANSPORT",
                       "detail": "reply shape"})
    if reply.get("uncaptured"):
        return _reply({"ok": False, "error": "SLEY2_SHIM_UNCAPTURED",
                       "detail": str(reply.get("error", ""))[:120]})
    return _reply(reply)


if __name__ == "__main__":
    raise SystemExit(main())
