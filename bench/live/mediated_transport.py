#!/usr/bin/env python3
"""Generic mediated transport for confined sley_2_0 trials (production).

Stdlib only. This is the only client-side Python staged into the
production agent scratch by ``stage_mediated_scratch`` alongside the
documented tool contract (``.sley-live/TOOLING.md``) and the
frame-forwarding ``.sley-live/sley-tool`` shim.

Contains no task content: no witness sequences, no role discovery,
no member literals, no expected outcomes, no synthetic provider-event
emission. Deterministic test adapters live in test-only
``mediated_client.py`` (never staged by the production path) and
import this transport for frame movement only.

The live model drives the documented ``sley-tool`` surface directly;
this module is the shared frame transport any in-sandbox helper may
use. Every call crosses the runner-side gateway and is captured and
counted like any agent action.
"""

from __future__ import annotations

import json
import os
import sys

SOCK_ENV = "SLEY2_GATEWAY_SOCK"
FRAME_LIMIT = 8 * 1024 * 1024

# Generic checked-scalar type descriptor used in envelope examples.
# Carries no task answer: callers supply their own payloads.
SINT = {"variant": "SInt", "value": 64}


def log(text: str) -> None:
    sys.stderr.write(text + "\n")
    sys.stderr.flush()


def _report(reply: dict) -> dict:
    """Gateway envelope report (raises on refusal)."""

    if not isinstance(reply, dict) or not reply.get("ok"):
        raise RuntimeError(f"gateway refused: {str(reply)[:200]}")
    report = reply.get("report")
    if not isinstance(report, dict):
        raise RuntimeError("gateway report shape")
    return report


def _decoded_body(report: dict) -> dict:
    """Decoded entity body from a read report (raises on failure)."""

    if report.get("failed"):
        raise RuntimeError(f"read failed: {str(report)[:200]}")
    decoded = report.get("decoded") or {}
    entries = decoded.get("entries") or []
    if len(entries) != 1 or not isinstance(entries[0].get("body"), dict):
        raise RuntimeError("read body shape")
    return entries[0]["body"]


def op_create(kind: int, payload: dict) -> dict:
    return {"class": "CreateEntity", "kind": kind, "target": None,
            "field_tag": None, "payload": payload}


def op_replace(kind: int, target: str, payload: dict) -> dict:
    return {"class": "ReplaceEntityVersion", "kind": kind,
            "target": target, "field_tag": None, "payload": payload}


class Gateway:
    """Frame transport to the runner-side mediated gateway.

    One frame per call (phase/session/command/args); the reply is the
    captured gateway envelope. Transport failures raise; denials come
    back as envelopes for the caller to handle.
    """

    def __init__(self, session_id: str) -> None:
        self.session_id = session_id
        self.frames = 0
        self.cmdlog: list[str] = []
        sock_path = os.environ.get(SOCK_ENV, "")
        self._sockfile = None
        if sock_path:
            import socket as _socket

            try:
                client = _socket.socket(_socket.AF_UNIX,
                                        _socket.SOCK_STREAM)
                client.connect(sock_path)
            except OSError as error:
                raise RuntimeError(f"gateway socket: {error}")
            self._sockfile = client.makefile("rwb")

    def call(self, phase: str, command: str,
             args: list[str]) -> dict:
        frame = {"phase": phase, "session_id": self.session_id,
                 "command": command, "args": args}
        if self._sockfile is not None:
            raw = (json.dumps(frame, sort_keys=True) + "\n").encode()
            try:
                self._sockfile.write(raw)
                self._sockfile.flush()
                line = self._sockfile.readline(FRAME_LIMIT + 2)
            except OSError as error:
                raise RuntimeError(f"gateway transport: {error}")
            if not line:
                raise RuntimeError("gateway EOF")
            reply = json.loads(line.decode("utf-8"))
        else:
            sys.stdout.write(json.dumps(frame, sort_keys=True) + "\n")
            sys.stdout.flush()
            line = sys.stdin.readline()
            if not line:
                raise RuntimeError("gateway EOF")
            reply = json.loads(line)
        self.frames += 1
        self.cmdlog.append(command)
        if reply.get("uncaptured"):
            raise RuntimeError(
                f"gateway capture failure: {reply.get('error')}")
        return reply
