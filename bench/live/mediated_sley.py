#!/usr/bin/env python3
"""Mediated sley_2_0 trial gateway: the agent's only route to trial state.

The confined agent process holds no protected filesystem: no repository,
no packs, no oracle inputs, no capture directory. Its sole channel is a
pipe of JSON frames to the trusted runner. Each frame names a phase, a
client session id, and one documented tool command; the runner executes
it against the protected workspace through the real `sley2_tool`
Session/dispatch machinery, inside `TrustedCapture.exchange` (request
recorded before dispatch, response recorded before release, budgets
accumulated trial-wide).

The model-facing surface is unchanged (the frozen documented commands).
Authoritative evidence is the runner-owned capture, never
candidate-side files: anything the agent writes in its scratch
(transcripts, hex dumps, diagnostics) is not consulted by
reconciliation or adjudication.

Refusals (unknown commands, denied methods, invalid bodies, server
failures) are recorded responses like any other and counted in the
cumulative ledger. Continuation accounting (omitted/truncated/continued)
flows from the same per-invocation session summary as the unmediated
tool, so legitimate bounded query and continuation behavior is
preserved, not prohibited.
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
import time
from pathlib import Path
from typing import Any, Callable, Mapping

from bench.live import sley2_tool
from bench.live.trusted_capture import (
    CaptureError,
    TrustedCapture,
    _canonical,
    reconcile,
)

GATEWAY_CONTRACT = "sley2.mediated-gateway.v1"

# The frozen documented surface (same commands as sley2_tool.dispatch).
ALLOWED_COMMANDS = frozenset({
    "inventory", "side", "read", "sig", "open", "revision", "caps", "budgets",
    "raw", "inspect", "validate", "append", "compose", "propose", "finish",
})

# Gateway-local mechanical helper (NOT a server method): `resolve`
# derives created-entity identities for a client-supplied base record
# under its own nonce. Derivation is payload-independent deterministic
# computation the unmediated tool already performs agent-side
# (sley2_tool._assemble via the codec service); the server re-verifies
# every derived identity at create time, so `resolve` grants no new
# authority. It exists so multi-phase composition needs no client-side
# codec stack inside the sandbox; every call is a captured exchange
# like any other.
GATEWAY_COMMANDS = ALLOWED_COMMANDS | {"resolve"}

FINAL_NAME = "final_candidate.hex"


class GatewayError(ValueError):
    """A mediated-gateway framing or dispatch error."""


def _fail(symbol: str, detail: str = "") -> None:
    raise GatewayError(symbol if not detail else f"{symbol}: {detail}")


class MediatedSleyEndpoint:
    """Runner-side executor on the protected workspace."""

    def __init__(self, sley_binary: Path, protected_ws: Path,
                 capture: TrustedCapture) -> None:
        binary = Path(sley_binary)
        workspace = Path(protected_ws)
        try:
            if (not binary.is_file() or binary.is_symlink()
                    or not os.access(binary, os.X_OK)):
                _fail("GATEWAY_BINARY_UNBOUND")
            if not workspace.is_dir() or workspace.is_symlink():
                _fail("GATEWAY_WORKSPACE_INVALID")
        except OSError as error:
            raise GatewayError(f"GATEWAY_INVALID: {error}") from error
        self._sley = binary
        self._workspace = workspace
        self._capture = capture

    @property
    def workspace(self) -> Path:
        return self._workspace

    @property
    def capture(self) -> TrustedCapture:
        return self._capture

    def _resolve(self, args: list[str]
                 ) -> tuple[dict[str, Any], dict[str, Any]]:
        """Gateway-local identity derivation (see GATEWAY_COMMANDS).

        Args: [base_record_hex, kinds_json] where kinds_json is the
        create-kind list whose identities the client needs under the
        base record's nonce. Returns {nonce, ids} derived through the
        pinned codecs against the live head workspace. Pure function
        of client-supplied bytes + public head state; the server
        re-checks every identity at create/validate time.
        """

        from bench.live import sley2_codecs

        if len(args) != 2:
            _fail("GATEWAY_ARGS_INVALID", "resolve arity")
        try:
            record_hex = bytes.fromhex(args[0]).hex()
        except ValueError:
            _fail("GATEWAY_ARGS_INVALID", "resolve record hex")
        try:
            kinds = json.loads(args[1])
        except json.JSONDecodeError:
            _fail("GATEWAY_ARGS_INVALID", "resolve kinds JSON")
        if (not isinstance(kinds, list) or not kinds or len(kinds) > 64
                or any(isinstance(k, bool) or not isinstance(k, int)
                       or k < 0 for k in kinds)):
            _fail("GATEWAY_ARGS_INVALID", "resolve kinds")
        transcript: list[dict[str, Any]] = []
        session = sley2_tool.Session(self._sley, self._workspace, transcript)
        try:
            try:
                [described] = sley2_codecs.run_batch(
                    [{"op": "describe_record", "record": record_hex}])
                nonce = described["nonce"]
                workspace_id = session.head["workspace"]
                batch = [{"op": "derive_entity", "workspace": workspace_id,
                          "nonce": nonce, "kind": kind, "ordinal": index}
                         for index, kind in enumerate(kinds)]
                derived = sley2_codecs.run_batch(batch)
                ids = [item["entity"] for item in derived]
                envelope: dict[str, Any] = {
                    "ok": True, "report": {"nonce": nonce, "ids": ids}}
                failed = False
            except (sley2_codecs.CodecError, ValueError, OSError,
                    KeyError) as error:
                envelope = {"ok": False, "error": type(error).__name__,
                            "detail": str(error)[:300]}
                failed = True
            summary = sley2_tool._session_summary(transcript)
            usage = {
                "failed": failed or summary.get("failed", 0) > 0,
                "omitted": int(summary.get("omitted", 0) or 0),
                "truncated": bool(summary.get("truncated", 0)),
                "continued": int(summary.get("continuations", 0) or 0) > 0,
            }
            return envelope, usage
        finally:
            session.close()

    def _run_command(self, command: str,
                     args: list[str]) -> tuple[dict[str, Any], dict[str, Any]]:
        """Execute one documented command on protected state.

        Returns (result_envelope, usage). Raises GatewayError for
        framing violations; Sley2ToolError/codec failures are returned
        as failed envelopes (recorded responses, not gateway faults).
        Denied commands are likewise ordinary failed responses: the
        denial is captured and counted, never an unrecorded path.
        """

        if command not in GATEWAY_COMMANDS:
            return ({"ok": False, "error": "GATEWAY_COMMAND_DENIED",
                     "detail": command[:80]},
                    {"failed": True, "omitted": 0, "truncated": False,
                     "continued": False})
        if (not isinstance(args, list)
                or any(not isinstance(a, str) or "\x00" in a for a in args)):
            _fail("GATEWAY_ARGS_INVALID")
        if command == "resolve":
            return self._resolve(args)
        transcript: list[dict[str, Any]] = []
        session = sley2_tool.Session(self._sley, self._workspace, transcript)
        try:
            try:
                result = sley2_tool.dispatch(session, self._workspace,
                                             [command, *args])
                failed = False
                envelope: dict[str, Any] = {"ok": True, "report": result}
            except (sley2_tool.Sley2ToolError, ValueError, OSError) as error:
                failed = True
                envelope = {"ok": False, "error": type(error).__name__,
                            "detail": str(error)[:500]}
            summary = sley2_tool._session_summary(transcript)
            usage = {
                "failed": failed or summary.get("failed", 0) > 0,
                "omitted": int(summary.get("omitted", 0) or 0),
                "truncated": bool(summary.get("truncated", 0)),
                "continued": int(summary.get("continuations", 0) or 0) > 0,
            }
            return envelope, usage
        finally:
            session.close()

    def handle(self, phase: str, session_id: str, command: str,
               args: list[str]) -> bytes:
        """Mediate one frame through trusted capture. Returns the exact
        response bytes released to the agent (also durably captured)."""

        # Audit method: the access audit distinguishes bounded-query
        # routes by method name, but every query travels as a `raw`
        # frame (the inner method hides in the request args, of which
        # capture keeps only the digest). Record the inner method
        # explicitly (`raw:<method>` for allowlisted server methods)
        # so the judge can audit bounded-query discipline from
        # captured requests/responses alone. Schema-stable: the
        # capture record keeps the same fields; only the method
        # vocabulary gains the `raw:` prefix for raw frames.
        # Vocabulary note (contract revision 5): admitting
        # `workspace.open` to TOOL_METHODS moves a raw frame naming it
        # from `raw:denied` to `raw:workspace.open`, and the dedicated
        # `open` command records under its own label `open` (the else
        # branch). Neither is a bounded paging route, so any omitted or
        # truncated signal on them is hidden truncation to the judge.
        if command == "raw" and args and args[0] in sley2_tool.TOOL_METHODS:
            audit_method = f"raw:{args[0]}"
        elif command == "raw":
            audit_method = "raw:denied"
        else:
            audit_method = command

        def dispatch() -> tuple[bytes, dict[str, Any]]:
            envelope, usage = self._run_command(command, args)
            return _canonical(envelope), usage

        return self._capture.exchange(
            phase=phase, session_id=session_id, method=audit_method,
            request=_canonical({"command": command, "args": args}),
            handler=dispatch)

    def protected_final(self) -> bytes | None:
        """Runner-held final artifact (protected workspace only)."""

        target = self._workspace / FINAL_NAME
        try:
            if target.is_symlink() or not target.is_file():
                return None
            return target.read_bytes()
        except OSError:
            return None


def gateway_loop(endpoint: MediatedSleyEndpoint, capture: TrustedCapture,
                 stdin: Any = sys.stdin.buffer,
                 stdout: Any = sys.stdout.buffer) -> int:
    """Serve frames until EOF or a fail-closed capture error.

    On CaptureError the attempt is dead: an out-of-band refusal is
    emitted (marked uncaptured) and the loop ends. That refusal cannot
    rehabilitate the attempt; reconciliation will fail closed.
    """

    while True:
        line = stdin.readline()
        if not line:
            return 0
        if len(line) > 8 * 1024 * 1024:
            _fail("GATEWAY_FRAME_LIMIT")
        try:
            frame = json.loads(line)
        except (UnicodeError, json.JSONDecodeError):
            reply = {"ok": False, "error": "GATEWAY_FRAME_INVALID",
                     "uncaptured": True}
            stdout.write((_canonical(reply) + b"\n"))
            stdout.flush()
            continue
        if (not isinstance(frame, dict) or not isinstance(frame.get("phase"), str)
                or not isinstance(frame.get("session_id"), str)
                or not isinstance(frame.get("command"), str)
                or not isinstance(frame.get("args"), list)):
            reply = {"ok": False, "error": "GATEWAY_FRAME_INVALID",
                     "uncaptured": True}
            stdout.write((_canonical(reply) + b"\n"))
            stdout.flush()
            continue
        try:
            response = endpoint.handle(frame["phase"], frame["session_id"],
                                       frame["command"], frame["args"])
        except CaptureError as error:
            reply = {"ok": False, "error": "GATEWAY_CAPTURE_FAILED",
                     "detail": str(error)[:300], "uncaptured": True}
            stdout.write((_canonical(reply) + b"\n"))
            stdout.flush()
            return 3
        except GatewayError as error:
            # Framing/deadline faults (unknown commands are already
            # ordinary failed responses from handle()): record the
            # fault as a failed exchange so it is captured too.
            def denied() -> tuple[bytes, dict[str, Any]]:
                return (_canonical({"ok": False, "error": "GATEWAY_COMMAND_DENIED",
                                    "detail": str(error)[:300]}),
                        {"failed": True})
            try:
                response = capture.exchange(
                    phase=frame["phase"], session_id=frame["session_id"],
                    method=frame["command"],
                    request=_canonical({"command": frame["command"],
                                        "args": frame["args"]}),
                    handler=denied)
            except CaptureError as capture_error:
                reply = {"ok": False, "error": "GATEWAY_CAPTURE_FAILED",
                         "detail": str(capture_error)[:300],
                         "uncaptured": True}
                stdout.write((_canonical(reply) + b"\n"))
                stdout.flush()
                return 3
        stdout.write(response + b"\n")
        stdout.flush()


def adjudicate(capture_dir: Path, protected_final: bytes | None,
               oracle_verdict: Mapping[str, Any]) -> tuple[str, str | None]:
    """Campaign-side acceptance gate: reconciled capture AND accepted
    oracle, else a fail-closed non-acceptance.

    ``protected_final`` is the runner-held final artifact bytes (None
    when the protected workspace holds no finish). The oracle verdict
    is the trusted oracle's own ``{status, code}`` mapping. Returns
    ``(status, failure_code)`` with status in
    {accepted, rejected, harness_failure}.
    """

    if not isinstance(oracle_verdict, Mapping):
        return ("harness_failure", "CAPTURE_GATE_ORACLE_INVALID")
    if not isinstance(protected_final, bytes):
        # No runner-held final: nothing is releasable. The capture
        # directory itself stays preserved failure evidence; callers
        # reconcile it separately when auditing the attempt.
        return ("harness_failure", "CAPTURE_GATE_NO_FINAL")
    try:
        result = reconcile(capture_dir, protected_final)
    except CaptureError as error:
        return ("harness_failure", f"CAPTURE_GATE_STORAGE: {error}"[:160])
    if not result["reconciled"]:
        # Failure evidence (complete prefix, totals, sessions) is
        # preserved in the capture directory; the attempt slot records
        # the capture code, never an acceptance.
        return ("harness_failure", result["code"])
    status = oracle_verdict.get("status")
    code = oracle_verdict.get("code")
    if status == "accepted":
        return ("accepted", None)
    if status == "rejected" and isinstance(code, str) and code:
        return ("rejected", code)
    return ("harness_failure", "CAPTURE_GATE_ORACLE_INVALID")


def digest_file(path: Path) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def pump_client(process: Any, endpoint: MediatedSleyEndpoint,
                *, frame_limit_bytes: int = 8 * 1024 * 1024,
                deadline_s: float = 3600) -> dict[str, Any]:
    """Route frames between a confined client process and the gateway.

    ``process`` is a Popen with piped stdin/stdout/stderr (child
    stdout = request frames, child stdin = responses, child stderr =
    diagnostics). Every stdout line must be a well-formed frame;
    anything else fails the attempt closed (frame-discipline
    violation). Returns {frames, diagnostics, returncode}.
    """

    import subprocess as _subprocess

    started = time.monotonic()
    frames = 0
    assert process.stdin is not None and process.stdout is not None
    while True:
        if time.monotonic() - started > deadline_s:
            try:
                process.kill()
            except OSError:
                pass
            _fail("GATEWAY_PUMP_DEADLINE")
        line = process.stdout.readline()
        if not line:
            break
        if len(line) > frame_limit_bytes:
            _fail("GATEWAY_FRAME_LIMIT")
        try:
            frame = json.loads(line)
        except (UnicodeError, json.JSONDecodeError):
            _fail("GATEWAY_FRAME_INVALID", "not JSON")
        if (not isinstance(frame, dict)
                or not isinstance(frame.get("phase"), str)
                or not isinstance(frame.get("session_id"), str)
                or not isinstance(frame.get("command"), str)
                or not isinstance(frame.get("args"), list)):
            _fail("GATEWAY_FRAME_INVALID", "shape")
        try:
            response = endpoint.handle(frame["phase"], frame["session_id"],
                                       frame["command"], frame["args"])
        except CaptureError as error:
            # Attempt dead; notify out-of-band (uncaptured) and stop.
            # Reconciliation will fail closed.
            note = _canonical({"ok": False,
                               "error": "GATEWAY_CAPTURE_FAILED",
                               "detail": str(error)[:300],
                               "uncaptured": True}) + b"\n"
            try:
                process.stdin.write(note)
                process.stdin.flush()
            except (OSError, ValueError):
                pass
            raise
        except GatewayError as error:
            # Framing/deadline faults (unknown commands are already
            # ordinary failed responses from handle()): record the
            # fault as a failed exchange so it is captured too.
            def denied() -> tuple[bytes, dict[str, Any]]:
                return (_canonical({"ok": False,
                                    "error": "GATEWAY_COMMAND_DENIED",
                                    "detail": str(error)[:300]}),
                        {"failed": True})

            try:
                response = endpoint.capture.exchange(
                    phase=frame["phase"], session_id=frame["session_id"],
                    method=frame["command"],
                    request=_canonical({"command": frame["command"],
                                        "args": frame["args"]}),
                    handler=denied)
            except CaptureError:
                raise
        try:
            process.stdin.write(response + b"\n")
            process.stdin.flush()
        except (OSError, ValueError) as error:
            raise GatewayError(f"GATEWAY_CLIENT_WRITE: {error}") from error
        frames += 1
    try:
        process.stdin.close()
    except (OSError, ValueError):
        pass
    try:
        _, stderr = process.communicate(timeout=60)
    except _subprocess.TimeoutExpired as error:
        try:
            process.kill()
        except OSError:
            pass
        raise GatewayError(f"GATEWAY_CLIENT_DRAIN: {error}") from error
    return {"frames": frames,
            "diagnostics": (stderr or b"").decode("utf-8", "replace"),
            "returncode": process.returncode}
