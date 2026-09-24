#!/usr/bin/env python3
"""Runner-owned authoritative capture for live succession trials.

The existing campaign archival machinery copies agent-writable evidence
after provider completion. It is retained as archival infrastructure.
Authoritative capture lives here instead: the trusted runner owns the
collector, the protected repository state, fixture packs, oracle inputs,
and every evidence byte from before the first agent interaction.

Protocol
--------

- The runner creates one capture directory per attempt (mode 0o700) and
  writes ``start.json`` binding the attempt id, frozen inputs (pack,
  task manifest, tool version, binary identity), and budget caps.
- Every agent request crosses the mediated interface as bytes. The
  runner appends a ``request`` record (fsync file + directory) BEFORE
  dispatch, performs the permitted operation on protected state, then
  appends the ``response`` record (fsync) BEFORE releasing any byte to
  the agent. Refusals, errors, and unsuccessful operations are recorded
  the same way: there is no unrecorded route to a response.
- Records hash-chain (each names the previous digest; sequence numbers
  are contiguous from zero). A crash can only truncate the suffix; the
  retained prefix stays verifiable failure evidence and never an
  acceptance.
- ``complete()`` writes ``completion.json`` binding the final artifact
  digest, exchange count, chain head, cumulative budgets, and the
  session/phase transitions the runner observed. The runner alone
  writes start and completion records.
- ``reconcile()`` fails closed on: missing start, any gap, broken
  chain, orphan request (crash between request dispatch and response
  capture), torn suffix, missing/broken budget ledger, missing
  completion, chain-head mismatch, or final-linkage mismatch. Missing
  capture, incomplete delivery, unavailable storage, or failed
  reconciliation can never produce an accepted attempt.
- Budgets accumulate trial-wide across subprocesses, sessions, phases,
  and retries in ``budgets.json``. A corrupted or missing ledger is a
  fail-closed capture error that preserves the raw bytes; accounting is
  never reset to zero.

Candidate-side diagnostic files are never read here: only the
runner-owned capture directory and the runner-held final artifact
participate in reconciliation.
"""

from __future__ import annotations

import hashlib
import json
import os
import stat
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Mapping

CAPTURE_CONTRACT = "sley2.trusted-capture.v1"
COMPLETION_CONTRACT = "sley2.trusted-capture-completion.v1"
GENESIS_HASH = "0" * 64

START_NAME = "start.json"
EXCHANGES_NAME = "exchanges.jsonl"
BUDGETS_NAME = "budgets.json"
COMPLETION_NAME = "completion.json"

# Cumulative trial-wide budget keys. ``refused`` counts runner-side
# cap refusals (per-response over-cap and trial-wide exhaustion);
# ``failed`` counts operation-level failures (server refusals, invalid
# commands). Both are responses like any other: recorded before
# release, counted under the same ledger.
BUDGET_KEYS = (
    "exchanges",
    "failed",
    "refused",
    "response_bytes",
    "wall_ms",
    "continuations",
    "omitted",
    "truncated",
)


class CaptureError(ValueError):
    """A fail-closed trusted-capture error (storage, chain, or budget)."""


def _fail(symbol: str, detail: str = "") -> None:
    raise CaptureError(symbol if not detail else f"{symbol}: {detail}")


def _canonical(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"),
                      ensure_ascii=True).encode("utf-8")


def _sha256hex(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _fsync_dir(directory: Path) -> None:
    try:
        descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
    except OSError as error:
        raise CaptureError(f"CAPTURE_STORAGE_INVALID: dir: {error}") from error
    try:
        os.fsync(descriptor)
    except OSError as error:
        raise CaptureError(f"CAPTURE_STORAGE_INVALID: fsync dir: {error}") from error
    finally:
        os.close(descriptor)


def _write_synced(path: Path, payload: bytes) -> None:
    """Create-or-replace a small file with file + directory fsync."""

    tmp = path.parent / (path.name + ".tmp")
    flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(tmp, flags, 0o600)
    except OSError as error:
        raise CaptureError(f"CAPTURE_STORAGE_INVALID: open: {error}") from error
    try:
        view = memoryview(payload)
        written = 0
        while written < len(view):
            count = os.write(descriptor, view[written:])
            if count <= 0:
                _fail("CAPTURE_STORAGE_INVALID", "short write")
            written += count
        os.fsync(descriptor)
    except OSError as error:
        raise CaptureError(f"CAPTURE_STORAGE_INVALID: write: {error}") from error
    finally:
        os.close(descriptor)
    try:
        os.rename(tmp, path)
    except OSError as error:
        raise CaptureError(f"CAPTURE_STORAGE_INVALID: rename: {error}") from error
    _fsync_dir(path.parent)


def _append_synced(path: Path, line: bytes) -> None:
    """Append one ``\\n``-terminated record with file + directory fsync."""

    flags = os.O_WRONLY | os.O_CREAT | os.O_APPEND
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as error:
        raise CaptureError(f"CAPTURE_STORAGE_INVALID: open: {error}") from error
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode):
            _fail("CAPTURE_STORAGE_INVALID", "not a regular file")
        view = memoryview(line)
        written = 0
        while written < len(view):
            count = os.write(descriptor, view[written:])
            if count <= 0:
                _fail("CAPTURE_STORAGE_INVALID", "short write")
            written += count
        os.fsync(descriptor)
    except OSError as error:
        raise CaptureError(f"CAPTURE_STORAGE_INVALID: write: {error}") from error
    finally:
        os.close(descriptor)
    _fsync_dir(path.parent)


def _read_synced(path: Path) -> bytes:
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except FileNotFoundError:
        raise
    except OSError as error:
        raise CaptureError(f"CAPTURE_STORAGE_INVALID: open: {error}") from error
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode):
            _fail("CAPTURE_STORAGE_INVALID", "not a regular file")
        chunks: list[bytes] = []
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                return b"".join(chunks)
            chunks.append(chunk)
    except OSError as error:
        raise CaptureError(f"CAPTURE_STORAGE_INVALID: read: {error}") from error
    finally:
        os.close(descriptor)


class BudgetLedger:
    """Cumulative trial-wide accounting. Never resets to zero.

    The ledger file lives in the runner-owned capture directory. A
    missing or malformed ledger on a resumed capture is a fail-closed
    error: the raw bytes are preserved untouched and the attempt
    cannot proceed, let alone restart accounting from zero. The only
    zero state is the one written at capture creation.
    """

    def __init__(self, path: Path, totals: dict[str, int]) -> None:
        self._path = path
        self._totals = totals

    @staticmethod
    def _check_shape(totals: Any) -> dict[str, int]:
        if not isinstance(totals, dict) or set(totals) != set(BUDGET_KEYS):
            _fail("CAPTURE_BUDGET_CORRUPT", "key set")
        clean: dict[str, int] = {}
        for key in BUDGET_KEYS:
            value = totals[key]
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                _fail("CAPTURE_BUDGET_CORRUPT", key)
            clean[key] = value
        return clean

    @classmethod
    def create(cls, path: Path) -> "BudgetLedger":
        totals = {key: 0 for key in BUDGET_KEYS}
        _write_synced(path, _canonical({"totals": totals}) + b"\n")
        return cls(path, totals)

    @classmethod
    def resume(cls, path: Path) -> "BudgetLedger":
        """Reload cumulative totals. Corrupt/missing input fails closed
        with the raw bytes preserved; nothing is zeroed."""

        try:
            raw = _read_synced(path)
        except FileNotFoundError as error:
            raise CaptureError("CAPTURE_BUDGET_MISSING: ledger absent") from error
        try:
            loaded = json.loads(raw)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise CaptureError(
                f"CAPTURE_BUDGET_CORRUPT: not JSON ({len(raw)} raw bytes preserved)"
            ) from error
        if not isinstance(loaded, dict) or "totals" not in loaded:
            _fail("CAPTURE_BUDGET_CORRUPT", "shape")
        try:
            totals = cls._check_shape(loaded["totals"])
        except CaptureError as error:
            raise CaptureError(
                f"{error} ({len(raw)} raw bytes preserved)"
            ) from error
        return cls(path, totals)

    @property
    def totals(self) -> dict[str, int]:
        return dict(self._totals)

    def record(self, deltas: Mapping[str, int]) -> dict[str, int]:
        for key, delta in deltas.items():
            if key not in BUDGET_KEYS:
                _fail("CAPTURE_BUDGET_CORRUPT", f"unknown key {key}")
            if (isinstance(delta, bool) or not isinstance(delta, int)
                    or delta < 0):
                _fail("CAPTURE_BUDGET_CORRUPT", f"delta {key}")
            self._totals[key] += delta
        _write_synced(self._path, _canonical({"totals": self._totals}) + b"\n")
        return self.totals


class TrustedCapture:
    """Runner-owned per-attempt collector (create and append only)."""

    def __init__(self, capture_dir: Path, attempt_id: str,
                 frozen: dict[str, Any], caps: dict[str, int],
                 ledger: BudgetLedger, seq: int, head: str) -> None:
        self._dir = capture_dir
        self._attempt = attempt_id
        self._frozen = frozen
        self._caps = caps
        self._ledger = ledger
        self._seq = seq
        self._head = head
        self._sessions: set[str] = set()
        self._phases: set[str] = set()

    @property
    def attempt_id(self) -> str:
        return self._attempt

    @property
    def directory(self) -> Path:
        return self._dir

    @property
    def totals(self) -> dict[str, int]:
        return self._ledger.totals

    @classmethod
    def create(cls, capture_dir: Path, *, attempt_id: str,
               frozen: Mapping[str, Any],
               caps: Mapping[str, int] | None = None,
               utc_now: Callable[[], str] = _utc_now) -> "TrustedCapture":
        """Create the runner-owned capture before any agent interaction."""

        root = Path(capture_dir)
        try:
            root.mkdir(mode=0o700, parents=True, exist_ok=False)
        except OSError as error:
            raise CaptureError(
                f"CAPTURE_STORAGE_INVALID: mkdir: {error}") from error
        if not isinstance(attempt_id, str) or not attempt_id:
            _fail("CAPTURE_START_INVALID", "attempt_id")
        frozen_clean = dict(frozen)
        for key in ("pack_sha256", "task_manifest_sha256", "tool_version",
                    "binary_sha256"):
            if not isinstance(frozen_clean.get(key), str) or not frozen_clean[key]:
                _fail("CAPTURE_START_INVALID", f"frozen {key}")
        limits = {"per_response_max_bytes": 1048576,
                  "trial_max_response_bytes": 8388608,
                  "trial_max_exchanges": 512,
                  "trial_max_wall_ms": 3600000}
        if caps:
            for key, value in caps.items():
                if key not in limits:
                    _fail("CAPTURE_START_INVALID", f"cap {key}")
                if (isinstance(value, bool) or not isinstance(value, int)
                        or value <= 0):
                    _fail("CAPTURE_START_INVALID", f"cap {key}")
                limits[key] = value
        start = {
            "contract": CAPTURE_CONTRACT,
            "attempt_id": attempt_id,
            "frozen": frozen_clean,
            "caps": limits,
            "created_utc": utc_now(),
            "creator_pid": os.getpid(),
        }
        _write_synced(root / START_NAME, _canonical(start) + b"\n")
        ledger = BudgetLedger.create(root / BUDGETS_NAME)
        return cls(root, attempt_id, frozen_clean, limits, ledger, 0,
                   GENESIS_HASH)

    def _seal(self, record: dict[str, Any]) -> dict[str, Any]:
        body = _canonical(record)
        digest = _sha256hex((self._head + body.decode("ascii")).encode("ascii"))
        sealed = dict(record)
        sealed["prev"] = self._head
        sealed["hash"] = digest
        return sealed

    def exchange(self, *, phase: str, session_id: str, method: str,
                 request: bytes,
                 handler: Callable[[], tuple[bytes, dict[str, Any]]],
                 utc_now: Callable[[], str] = _utc_now) -> bytes:
        """Mediate one agent request through protected state.

        ``handler`` runs the permitted operation on runner-owned state
        and returns ``(response_bytes, usage)`` where ``usage`` may
        carry ``failed`` (bool), ``omitted``/``truncated``/``continued``
        accounting. Handler exceptions are captured as failed
        responses: the failure is recorded before anything is
        released, and a CaptureError propagates to the runner (the
        agent receives a refusal frame from the gateway, never raw
        protected state).
        """

        if not isinstance(phase, str) or not phase:
            _fail("CAPTURE_EXCHANGE_INVALID", "phase")
        if not isinstance(session_id, str) or not session_id:
            _fail("CAPTURE_EXCHANGE_INVALID", "session_id")
        if not isinstance(method, str) or not method:
            _fail("CAPTURE_EXCHANGE_INVALID", "method")
        if not isinstance(request, bytes):
            _fail("CAPTURE_EXCHANGE_INVALID", "request bytes")
        seq = self._seq
        # 1. Request BEFORE dispatch (durable). A crash from here on
        #    leaves an orphan request: reconciliation rejects while
        #    preserving the prefix.
        req_record = self._seal({
            "contract": CAPTURE_CONTRACT,
            "attempt_id": self._attempt,
            "kind": "request",
            "seq": seq,
            "phase": phase,
            "session_id": session_id,
            "method": method,
            "request_sha256": _sha256hex(request),
            "request_bytes": len(request),
            "t_utc": utc_now(),
        })
        _append_synced(self._dir / EXCHANGES_NAME,
                       _canonical(req_record) + b"\n")
        self._head = req_record["hash"]
        # 2. Dispatch on protected state (trusted side only).
        started = time.monotonic_ns()
        failed = False
        omitted = 0
        truncated = False
        continued = False
        chain: dict[str, Any] | None = None
        response: bytes
        try:
            response, usage = handler()
            if not isinstance(response, bytes):
                _fail("CAPTURE_HANDLER_INVALID", "response bytes")
            if not isinstance(usage, Mapping):
                _fail("CAPTURE_HANDLER_INVALID", "usage")
            failed = bool(usage.get("failed", False))
            omitted = int(usage.get("omitted", 0) or 0)
            truncated = bool(usage.get("truncated", False))
            continued = bool(usage.get("continued", False))
            raw_chain = usage.get("chain")
            if isinstance(raw_chain, Mapping):
                chain = {"query": raw_chain.get("query"),
                         "after": raw_chain.get("after"),
                         "truncated": raw_chain.get("truncated"),
                         "next": raw_chain.get("next")}
        except CaptureError:
            raise
        except Exception as error:  # noqa: BLE001 - boundary records all failures
            response = _canonical({"ok": False, "error": type(error).__name__,
                                   "detail": str(error)[:300]})
            failed = True
            usage = {}
            chain = None
        wall_ms = max(0, (time.monotonic_ns() - started) // 1_000_000)
        # 3. Response BEFORE release (durable) + cumulative budgets.
        #    A root-query response also carries its continuation binding
        #    (`chain`: query key, request cursor, truncation, next cursor)
        #    so the judge binds each continue to the page it continues.
        body = {
            "contract": CAPTURE_CONTRACT,
            "attempt_id": self._attempt,
            "kind": "response",
            "seq": seq,
            "phase": phase,
            "session_id": session_id,
            "method": method,
            "response_sha256": _sha256hex(response),
            "response_bytes": len(response),
            "failed": failed,
            "omitted": omitted,
            "truncated": truncated,
            "continued": continued,
            "wall_ms": wall_ms,
            "t_utc": utc_now(),
        }
        if chain is not None:
            body["chain"] = chain
        resp_record = self._seal(body)
        _append_synced(self._dir / EXCHANGES_NAME,
                       _canonical(resp_record) + b"\n")
        self._head = resp_record["hash"]
        self._seq = seq + 1
        self._sessions.add(session_id)
        self._phases.add(phase)
        deltas: dict[str, int] = {
            "exchanges": 1,
            "failed": 1 if failed else 0,
            "refused": 0,
            "response_bytes": len(response),
            "wall_ms": wall_ms,
            "continuations": 1 if continued else 0,
            "omitted": omitted,
            "truncated": 1 if truncated else 0,
        }
        totals = self._ledger.record(deltas)
        # 4. Cap enforcement AFTER recording: the over-cap response is
        #    evidence (retained), and the attempt is refused (counted).
        if len(response) > self._caps["per_response_max_bytes"]:
            self._ledger.record({"exchanges": 0, "failed": 0, "refused": 1,
                                 "response_bytes": 0, "wall_ms": 0,
                                 "continuations": 0, "omitted": 0,
                                 "truncated": 0})
            raise CaptureError(
                f"CAPTURE_RESPONSE_OVER_CAP: seq {seq} "
                f"{len(response)} > {self._caps['per_response_max_bytes']}")
        if totals["response_bytes"] > self._caps["trial_max_response_bytes"]:
            self._ledger.record({"exchanges": 0, "failed": 0, "refused": 1,
                                 "response_bytes": 0, "wall_ms": 0,
                                 "continuations": 0, "omitted": 0,
                                 "truncated": 0})
            raise CaptureError(
                f"CAPTURE_TRIAL_BUDGET_EXCEEDED: response_bytes "
                f"{totals['response_bytes']}")
        if totals["exchanges"] > self._caps["trial_max_exchanges"]:
            self._ledger.record({"exchanges": 0, "failed": 0, "refused": 1,
                                 "response_bytes": 0, "wall_ms": 0,
                                 "continuations": 0, "omitted": 0,
                                 "truncated": 0})
            raise CaptureError("CAPTURE_TRIAL_BUDGET_EXCEEDED: exchanges")
        return response

    def complete(self, final: bytes, *,
                 utc_now: Callable[[], str] = _utc_now) -> dict[str, Any]:
        """Runner-controlled completion binding (runner writes only)."""

        if not isinstance(final, bytes):
            _fail("CAPTURE_COMPLETION_INVALID", "final bytes")
        completion = {
            "contract": COMPLETION_CONTRACT,
            "attempt_id": self._attempt,
            "final_sha256": _sha256hex(final),
            "final_bytes": len(final),
            "exchanges": self._seq,
            "chain_head": self._head,
            "totals": self._ledger.totals,
            "sessions": sorted(self._sessions),
            "phases": sorted(self._phases),
            "t_utc": utc_now(),
        }
        _write_synced(self._dir / COMPLETION_NAME,
                      _canonical(completion) + b"\n")
        return completion


def reconcile(capture_dir: Path, expected_final: bytes) -> dict[str, Any]:
    """Verify runner-owned capture against the runner-held final artifact.

    Returns a verdict dict ``{reconciled, code, detail, ...}``; never
    raises on content problems (storage unreadability raises
    CaptureError, itself a fail-closed non-acceptance). Only the
    capture directory and ``expected_final`` (runner-held bytes)
    participate: candidate-side files are not consulted.
    """

    root = Path(capture_dir)

    def verdict(ok: bool, code: str, detail: str = "",
                **extra: Any) -> dict[str, Any]:
        out: dict[str, Any] = {"reconciled": ok, "code": code,
                               "detail": detail}
        out.update(extra)
        return out
    try:
        start_raw = _read_synced(root / START_NAME)
    except FileNotFoundError:
        return verdict(False, "CAPTURE_MISSING_START", "no start.json")
    try:
        start = json.loads(start_raw)
    except (UnicodeError, json.JSONDecodeError):
        return verdict(False, "CAPTURE_START_INVALID", "start.json not JSON")
    if (not isinstance(start, dict) or start.get("contract") != CAPTURE_CONTRACT
            or not isinstance(start.get("attempt_id"), str)):
        return verdict(False, "CAPTURE_START_INVALID", "start shape")
    attempt_id = start["attempt_id"]
    try:
        ledger = BudgetLedger.resume(root / BUDGETS_NAME)
    except FileNotFoundError:
        return verdict(False, "CAPTURE_BUDGET_MISSING",
                       "ledger absent; accounting not zeroed")
    except CaptureError as error:
        text = str(error)
        if text.startswith("CAPTURE_BUDGET_MISSING"):
            return verdict(False, "CAPTURE_BUDGET_MISSING", text)
        return verdict(False, "CAPTURE_BUDGET_CORRUPT", text)
    try:
        raw = _read_synced(root / EXCHANGES_NAME)
    except FileNotFoundError:
        return verdict(False, "CAPTURE_NO_EXCHANGES", "no exchanges.jsonl")
    if not raw:
        return verdict(False, "CAPTURE_NO_EXCHANGES", "empty exchanges")
    lines = raw.split(b"\n")
    # A crash between write and newline leaves a torn suffix: the
    # prefix stays failure evidence; acceptance is impossible.
    torn = False
    if lines and lines[-1] != b"":
        torn = True
        suffix = lines[-1]
        lines = lines[:-1]
    else:
        suffix = b""
        if lines and lines[-1] == b"":
            lines = lines[:-1]
    head = GENESIS_HASH
    seq = 0
    pending_request: dict[str, Any] | None = None
    recomputed: dict[str, int] = {key: 0 for key in BUDGET_KEYS}
    sessions: set[str] = set()
    phases: set[str] = set()
    for number, line in enumerate(lines):
        try:
            record = json.loads(line)
        except (UnicodeError, json.JSONDecodeError):
            return verdict(False, "CAPTURE_CHAIN_BROKEN",
                           f"line {number} not JSON",
                           complete_exchanges=seq)
        if not isinstance(record, dict):
            return verdict(False, "CAPTURE_CHAIN_BROKEN",
                           f"line {number} shape", complete_exchanges=seq)
        if record.get("contract") != CAPTURE_CONTRACT:
            return verdict(False, "CAPTURE_CHAIN_BROKEN",
                           f"line {number} contract",
                           complete_exchanges=seq)
        if record.get("attempt_id") != attempt_id:
            return verdict(False, "CAPTURE_CHAIN_BROKEN",
                           f"line {number} attempt binding",
                           complete_exchanges=seq)
        if record.get("prev") != head:
            return verdict(False, "CAPTURE_CHAIN_BROKEN",
                           f"line {number} prev link",
                           complete_exchanges=seq)
        digest = record.get("hash")
        body = dict(record)
        body.pop("hash", None)
        body.pop("prev", None)
        if _sha256hex((head + _canonical(body).decode("ascii")).encode(
                "ascii")) != digest:
            return verdict(False, "CAPTURE_CHAIN_BROKEN",
                           f"line {number} hash", complete_exchanges=seq)
        head = digest
        kind = record.get("kind")
        if kind == "request":
            if pending_request is not None:
                return verdict(False, "CAPTURE_ORPHAN_REQUEST",
                               f"seq {pending_request.get('seq')} never answered",
                               complete_exchanges=seq)
            if record.get("seq") != seq:
                return verdict(False, "CAPTURE_GAP",
                               f"line {number} seq", complete_exchanges=seq)
            pending_request = record
        elif kind == "response":
            if pending_request is None:
                return verdict(False, "CAPTURE_GAP",
                               f"line {number} response without request",
                               complete_exchanges=seq)
            if record.get("seq") != seq:
                return verdict(False, "CAPTURE_GAP",
                               f"line {number} seq", complete_exchanges=seq)
            if (record.get("phase") != pending_request.get("phase")
                    or record.get("method") != pending_request.get("method")
                    or record.get("session_id")
                    != pending_request.get("session_id")):
                return verdict(False, "CAPTURE_CHAIN_BROKEN",
                               f"line {number} request/response binding",
                               complete_exchanges=seq)
            recomputed["exchanges"] += 1
            recomputed["failed"] += 1 if record.get("failed") else 0
            recomputed["response_bytes"] += int(record.get("response_bytes", 0))
            recomputed["wall_ms"] += int(record.get("wall_ms", 0))
            recomputed["continuations"] += 1 if record.get("continued") else 0
            recomputed["omitted"] += int(record.get("omitted", 0) or 0)
            recomputed["truncated"] += 1 if record.get("truncated") else 0
            sessions.add(str(record.get("session_id", "")))
            phases.add(str(record.get("phase", "")))
            pending_request = None
            seq += 1
        else:
            return verdict(False, "CAPTURE_CHAIN_BROKEN",
                           f"line {number} kind", complete_exchanges=seq)
    if pending_request is not None:
        # Interruption between request capture and response capture
        # (or torn suffix swallowing the response): failure evidence
        # preserved, acceptance impossible.
        return verdict(False, "CAPTURE_ORPHAN_REQUEST",
                       f"seq {pending_request.get('seq')} never answered",
                       complete_exchanges=seq)
    if torn:
        return verdict(False, "CAPTURE_TORN_SUFFIX",
                       f"{len(suffix)} torn trailing bytes after "
                       f"{seq} complete exchanges",
                       complete_exchanges=seq)
    totals = ledger.totals
    # Ledger refusal counts have no per-record counterpart (they are
    # recorded as ledger-only entries at enforcement time); compare
    # the record-derived keys.
    for key in ("exchanges", "failed", "response_bytes", "wall_ms",
                "continuations", "omitted", "truncated"):
        if totals.get(key) != recomputed.get(key):
            return verdict(False, "CAPTURE_TOTALS_MISMATCH",
                           f"{key}: ledger {totals.get(key)} != "
                           f"records {recomputed.get(key)}",
                           complete_exchanges=seq)
    try:
        completion_raw = _read_synced(root / COMPLETION_NAME)
    except FileNotFoundError:
        return verdict(False, "CAPTURE_COMPLETION_MISSING",
                       "no completion.json; runner never closed the attempt",
                       complete_exchanges=seq)
    try:
        completion = json.loads(completion_raw)
    except (UnicodeError, json.JSONDecodeError):
        return verdict(False, "CAPTURE_COMPLETION_INVALID",
                       "completion.json not JSON",
                       complete_exchanges=seq)
    if (not isinstance(completion, dict)
            or completion.get("contract") != COMPLETION_CONTRACT
            or completion.get("attempt_id") != attempt_id):
        return verdict(False, "CAPTURE_COMPLETION_INVALID",
                       "completion shape/binding", complete_exchanges=seq)
    if completion.get("chain_head") != head:
        return verdict(False, "CAPTURE_COMPLETION_INVALID",
                       "chain head mismatch", complete_exchanges=seq)
    if completion.get("exchanges") != seq:
        return verdict(False, "CAPTURE_COMPLETION_INVALID",
                       "exchange count mismatch", complete_exchanges=seq)
    if not isinstance(expected_final, bytes):
        return verdict(False, "CAPTURE_COMPLETION_INVALID",
                       "expected final not bytes", complete_exchanges=seq)
    if completion.get("final_sha256") != _sha256hex(expected_final):
        return verdict(False, "CAPTURE_FINAL_MISMATCH",
                       "completion names different bytes than the held final",
                       complete_exchanges=seq)
    return verdict(True, "CAPTURE_RECONCILED", "",
                   complete_exchanges=seq, totals=totals,
                   sessions=sorted(sessions), phases=sorted(phases))
