#!/usr/bin/env python3
"""S20-620 Sley 2 trial runner: endpoint-only, trace-complete, offline claims.

The runner drives the S20-430 `sley` endpoint in JSON mode over one
disposable repository, records every frame in a digest-chained trace before
the next request is written, derives ten metrics only from that trace, and
appends explicitly unverified trial claims under the `sley_2_0` arm with
the S20-610 manifest and chain primitives. The endpoint binary is the only
process it starts. Models, providers, and oracles remain injected Protocols
and no implementation of them is supplied.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import select
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from enum import IntEnum
from pathlib import Path
from typing import Any, Callable, Mapping, Protocol, runtime_checkable

from bench.sley2.handle import (
    HANDLE_SURFACE,
    EndpointHandle,
    handle_surface,
    reflected_privileged_names,
)
from bench.raw.runner import (
    EVIDENCE_STATUS,
    HEX_64,
    RUN_ID,
    RawRunnerError,
    VERIFICATION_STATUS,
    _expected_trial_pairs,
    _open_regular_at,
    _open_run_directory,
    _plan_and_corpus,
    _read_all,
    _read_manifest_exact,
    _read_regular_at,
    _require_utc_second,
    _task_ids,
    _validate_metric_controls,
    _validate_metrics,
    canonical_json_bytes,
    manifest_digest,
    task_statement_digest,
    validate_run_manifest,
    write_run_manifest,
)

ROOT = Path(__file__).resolve().parents[2]
ARM = "sley_2_0"
ARM_DIRECTORY = "sley2"
CLAIMS_NAME = "claims.jsonl"
TRACE_CONTRACT = "sley2.sley2-trial-trace.v1"
CLAIM_CONTRACT = "sley2.sley2-trial-digest-claim.v1"
TRACE_DOMAIN = TRACE_CONTRACT.encode("utf-8") + b"\0"
CLAIM_DOMAIN = CLAIM_CONTRACT.encode("utf-8") + b"\0"
EXCHANGE_FIXTURE = ROOT / "conformance/repository-exchange/v1/accepted.json"
METHOD_TABLE = ROOT / "conformance/smp1-json-bridge/v2/methods.json"
PLAN_PATH = ROOT / "bench/benchmark-plan.json"
CORPUS_PATH = ROOT / "bench/corpus/v1/tasks.json"
DEFAULT_SLEY = ROOT / "target/debug/sley"
# Only the adapter knows its own token usage, so it is the sole source for
# these and for nothing else.
ADAPTER_OWNED_METRICS = ("model_input_tokens", "model_output_tokens")
# Everything that grades the arm, or measures what the arm cost, comes from the
# oracle's judgement. An arm never reports its own correctness or resource use.
ORACLE_OWNED_METRICS = (
    "invalid_candidates",
    "repair_loops",
    "peak_memory",
    "canonical_storage_bytes",
    "pack_bytes",
    "execution_latency",
    "stale_candidates",
    "stale_candidates_incorrectly_accepted",
    "collateral_semantic_changes",
    "invalid_committed_states",
)
# The methods this arm's agent may name, frozen here rather than taken from
# whatever the endpoint happens to offer. The endpoint's own hello lists all 43
# SMP1 methods, including exchange.export, which is an entire-store dump that
# master goal 20.10 forbids an arm from having. The allowlist is a run control:
# its digest is recorded in the claim, so a run that widened it is visible.
ARM_AFFORDANCES = (
    "candidate.append",
    "candidate.create",
    "candidate.discard",
    "candidate.inspect",
    "candidate.validate",
    "capsule",
    "compare",
    "entity.signature",
    "entity.version",
    "handle.expand",
    "query.continue",
    "query.restricted",
    "query.root",
    "refs.list",
    "refs.resolve",
    "revision.read",
    "session.budgets",
    "session.capabilities",
    "workspace.open",
)
# Named so the reason for each exclusion survives: bulk export and import move
# whole stores; the rest mutate the repository or the run, or are harness,
# oracle, or run-control surfaces (session management, execution, reports,
# receipts, diagnostics, test selection, recovery). None belongs to an agent
# that is being measured on reading context and proposing a change. Exclusion
# is by risk, not by category: `workspace.open` (contract revision 5) takes no
# body, mutates nothing, and answers the accepted head, so it moved to the
# affordances as the arm's accepted-head opener.
ARM_DENIED_METHODS = (
    "branch.advance",
    "branch.create",
    "cancel",
    "checkout",
    "commit",
    "diagnostics",
    "exchange.export",
    "exchange.import",
    "execute",
    "gc.collect",
    "gc.dry_run",
    "merge.commit",
    "merge.judge",
    "receipt.read",
    "recovery",
    "ref.move.protected",
    "refs.recover",
    "report",
    "session.close",
    "session.open",
    "session.renew",
    "tests.affected",
    "tests.selected",
    "workspace.create",
)
CONTEXT_METHODS = frozenset({"capsule", "query.root", "query.continue", "query.restricted"})
TRIAL_STATUSES = frozenset({"accepted", "rejected", "timeout", "harness_failure"})
AGENT_REQUEST_FIELDS = frozenset({"method", "body", "cancel"})
# The capable CLI profile every live runner path uses: the nineteen-name
# allowlist needs the version 2 methods, so the offer, the serve process,
# and the handshake probe all run version-aware.
PROFILE_ARGS = ("--protocol-profile", "v2-capable")

ZERO_LIMITS = {
    "max_depth": 0,
    "max_edges": 0,
    "max_entities": 0,
    "max_frame_bytes": 0,
    "max_inflight": 0,
    "max_response_bytes": 0,
    "max_sessions": 0,
    "max_work": 0,
}
ZERO_BOUNDS = {
    "applied_limits": ZERO_LIMITS,
    "continuation": False,
    "omitted": 0,
    "reached_depth": 0,
    "returned_bytes": 0,
    "returned_edges": 0,
    "returned_entities": 0,
    "truncated": False,
}
CLAIM_FIELDS = frozenset(
    {
        "contract",
        "run_id",
        "trial_id",
        "arm_id",
        "task_id",
        "seed",
        "started_at_utc",
        "ended_at_utc",
        "status",
        "failure_code",
        "timeout",
        "endpoint_sha256",
        "handshake_id",
        "trace_head_digest",
        "trace_record_count",
        "report_digest",
        "fixture_digest",
        "exchange_digest",
        "prompt_digest",
        "model_output_digest",
        "oracle_report_digest",
        "evidence_status",
        "oracle_verification_status",
        "accounting_verification_status",
        "arm_affordances_digest",
        "metrics",
    }
)
CHAIN_FIELDS = frozenset({"previous_record_digest", "record_digest"})


class Sley2ErrorCode(IntEnum):
    MANIFEST_INVALID = 62_000
    ENDPOINT_UNAVAILABLE = 62_001
    HANDSHAKE_FAILED = 62_002
    FRAME_INVALID = 62_003
    TRACE_INVALID = 62_004
    CLAIM_INVALID = 62_005
    PRIVILEGED_CONTEXT = 62_006
    DUPLICATE = 62_007
    TIMEOUT = 62_008
    INTERNAL_INVARIANT = 62_009


SYMBOLS = {
    Sley2ErrorCode.MANIFEST_INVALID: "SLEY2_TRIAL_MANIFEST_INVALID",
    Sley2ErrorCode.ENDPOINT_UNAVAILABLE: "SLEY2_TRIAL_ENDPOINT_UNAVAILABLE",
    Sley2ErrorCode.HANDSHAKE_FAILED: "SLEY2_TRIAL_HANDSHAKE_FAILED",
    Sley2ErrorCode.FRAME_INVALID: "SLEY2_TRIAL_FRAME_INVALID",
    Sley2ErrorCode.TRACE_INVALID: "SLEY2_TRIAL_TRACE_INVALID",
    Sley2ErrorCode.CLAIM_INVALID: "SLEY2_TRIAL_CLAIM_INVALID",
    Sley2ErrorCode.PRIVILEGED_CONTEXT: "SLEY2_TRIAL_PRIVILEGED_CONTEXT",
    Sley2ErrorCode.DUPLICATE: "SLEY2_TRIAL_DUPLICATE",
    Sley2ErrorCode.TIMEOUT: "SLEY2_TRIAL_TIMEOUT",
    Sley2ErrorCode.INTERNAL_INVARIANT: "SLEY2_TRIAL_INTERNAL_INVARIANT",
}


class Sley2RunnerError(ValueError):
    def __init__(self, code: Sley2ErrorCode, detail: str = ""):
        super().__init__(f"{SYMBOLS[code]}:{detail}" if detail else SYMBOLS[code])
        self.code = code
        self.detail = detail

    @property
    def symbol(self) -> str:
        return SYMBOLS[self.code]


def _fail(code: Sley2ErrorCode, detail: str = "") -> None:
    raise Sley2RunnerError(code, detail)


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _canonical_sha256(value: Any) -> str:
    return _sha256(canonical_json_bytes(value))


# ---------------------------------------------------------------------------
# Injected interfaces (no implementation is supplied for real trials)
# ---------------------------------------------------------------------------


@runtime_checkable
class AgentAdapter(Protocol):
    """The agent arm: sees the endpoint handle and nothing else."""

    def run(self, handle: "EndpointHandle") -> Mapping[str, Any]:
        """Drive the endpoint; return an adapter-owned observation."""


@runtime_checkable
class OracleAdapter(Protocol):
    """Injected strict oracle; judges only from the task and the trace."""

    def evaluate(self, task: Mapping[str, Any], records: list[dict[str, Any]]) -> Mapping[str, Any]:
        """Return an adapter-owned observation with a `status` of accepted or rejected."""


@runtime_checkable
class AccountingClock(Protocol):
    """Injected accounting clock for claim timestamps."""

    def now_utc(self) -> str:
        """Return a UTC-second timestamp."""


# EndpointHandle lives in its own module so that a reflecting adapter reaching
# `exchange.__func__.__globals__` finds that module's names rather than this
# one's endpoint, subprocess, filesystem, and trace machinery.


# ---------------------------------------------------------------------------
# Frames
# ---------------------------------------------------------------------------


def request_frame(
    method: str,
    body_hex: str,
    session: str | None,
    request_id: int,
    cancel: bool = False,
    *,
    protocol_version: int,
) -> dict[str, Any]:
    """One request frame object. The version is a required keyword: every
    trial stamps the selected version 2 (`run_scripted_trial` requires
    it), so no run silently stamps 1 and no caller inherits a default."""
    if isinstance(protocol_version, bool) or not isinstance(protocol_version, int):
        _fail(Sley2ErrorCode.FRAME_INVALID, "protocol version")
        raise AssertionError("unreachable")
    if not isinstance(cancel, bool):
        _fail(Sley2ErrorCode.FRAME_INVALID, "cancel")
        raise AssertionError("unreachable")
    return {
        "body": body_hex,
        "bounds": ZERO_BOUNDS,
        "flags": {"cancel": cancel, "failed": False, "stream": False},
        "kind": "request",
        "method": method,
        "protocol_version": protocol_version,
        "request_id": request_id,
        "session": session,
    }


def _check_frame_shape(frame: Any) -> dict[str, Any]:
    if not isinstance(frame, dict) or frame.get("kind") not in {"hello", "request", "response", "event"}:
        _fail(Sley2ErrorCode.FRAME_INVALID, "kind")
    for field in ("body", "bounds", "flags", "method", "protocol_version", "request_id", "session"):
        if field not in frame:
            _fail(Sley2ErrorCode.FRAME_INVALID, field)
    if not isinstance(frame["flags"], dict) or not isinstance(frame["bounds"], dict):
        _fail(Sley2ErrorCode.FRAME_INVALID, "flags/bounds")
    return frame


# ---------------------------------------------------------------------------
# Endpoint process
# ---------------------------------------------------------------------------


class Endpoint:
    """One `sley serve --json` process; the runner's only external command."""

    def __init__(
        self,
        sley: Path,
        repository: Path,
        report: Path,
        timeout_seconds: int,
        extra_args: tuple[str, ...] = (),
    ):
        self._timeout = timeout_seconds
        try:
            self._process = subprocess.Popen(
                [str(sley), "serve", "--repository", str(repository), "--json", "--report", str(report), *extra_args],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        except OSError as error:
            raise Sley2RunnerError(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, str(error)) from error
        self._buffer = b""
        self._closed = False

    def send(self, frame: Mapping[str, Any]) -> list[dict[str, Any]]:
        assert self._process.stdin is not None
        try:
            self._process.stdin.write(canonical_json_bytes(dict(frame)) + b"\n")
            self._process.stdin.flush()
        except (OSError, ValueError) as error:
            raise Sley2RunnerError(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, str(error)) from error
        frames: list[dict[str, Any]] = []
        while True:
            line = self._read_line()
            try:
                frame_object = json.loads(line)
            except ValueError as error:
                raise Sley2RunnerError(Sley2ErrorCode.FRAME_INVALID, "endpoint line") from error
            frames.append(_check_frame_shape(frame_object))
            if frame_object["kind"] in {"hello", "response"}:
                return frames

    def _read_line(self) -> bytes:
        assert self._process.stdout is not None
        descriptor = self._process.stdout.fileno()
        deadline = time.monotonic() + self._timeout
        while b"\n" not in self._buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                _fail(Sley2ErrorCode.TIMEOUT, "endpoint response")
            ready, _, _ = select.select([descriptor], [], [], remaining)
            if not ready:
                _fail(Sley2ErrorCode.TIMEOUT, "endpoint response")
            chunk = os.read(descriptor, 65_536)
            if not chunk:
                _fail(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, "endpoint closed its output")
            self._buffer += chunk
        line, _, self._buffer = self._buffer.partition(b"\n")
        return line

    def close(self) -> tuple[int, str]:
        if self._closed:
            return self._process.returncode, ""
        self._closed = True
        try:
            # `communicate` closes standard input, drains the remaining
            # output, and waits; the endpoint exits at end of input.
            _, stderr = self._process.communicate(timeout=self._timeout)
        except subprocess.TimeoutExpired:
            self._process.kill()
            self._process.communicate()
            return 124, ""
        except (OSError, ValueError) as error:
            self._process.kill()
            self._process.wait()
            return 125, str(error)
        return self._process.returncode, stderr.decode("utf-8", errors="replace")


def endpoint_sha256(sley: Path) -> str:
    try:
        return _sha256(sley.read_bytes())
    except OSError as error:
        raise Sley2RunnerError(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, str(error)) from error


def _run_sley(sley: Path, arguments: list[str], stdin: bytes, timeout_seconds: int) -> bytes:
    try:
        completed = subprocess.run(
            [str(sley), *arguments],
            input=stdin,
            capture_output=True,
            timeout=timeout_seconds,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Sley2RunnerError(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, str(error)) from error
    if completed.returncode != 0:
        _fail(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, f"{arguments[0]} exit {completed.returncode}")
    return completed.stdout


def endpoint_offer(sley: Path, timeout_seconds: int = 30) -> tuple[dict[str, Any], list[str], dict[str, Any]]:
    """The client hello frame, the affordances, and the endpoint version, all from the binary.

    The offer is the capable `[1,2]` hello: the nineteen-name allowlist
    requires the version 2 methods, so a version 1 offer fails the
    missing-check below by design (drift fails closed)."""

    hello_bytes = _run_sley(sley, ["hello", *PROFILE_ARGS], b"", timeout_seconds)
    decoded = _run_sley(sley, ["frame", "decode"], hello_bytes, timeout_seconds)
    hello_object = _run_sley(sley, ["hello", "--json", *PROFILE_ARGS], b"", timeout_seconds)
    version = _run_sley(sley, ["version", *PROFILE_ARGS], b"", timeout_seconds)
    try:
        frame = _check_frame_shape(json.loads(decoded.decode("utf-8").strip()))
        offered = list(json.loads(hello_object)["methods"])
        version_object = json.loads(version)
    except (ValueError, KeyError, TypeError) as error:
        raise Sley2RunnerError(Sley2ErrorCode.FRAME_INVALID, "endpoint offer") from error
    if frame["kind"] != "hello":
        _fail(Sley2ErrorCode.HANDSHAKE_FAILED, "offer is not a hello")
    # The arm gets its frozen allowlist, not the endpoint's whole offer. A name
    # the allowlist claims but the endpoint does not offer is drift in the other
    # direction and fails rather than silently shrinking the arm.
    missing = [name for name in ARM_AFFORDANCES if name not in offered]
    if missing:
        _fail(Sley2ErrorCode.HANDSHAKE_FAILED, f"endpoint does not offer {missing}")
    denied = [name for name in ARM_DENIED_METHODS if name in ARM_AFFORDANCES]
    if denied:
        _fail(Sley2ErrorCode.INTERNAL_INVARIANT, f"allowlist names denied methods {denied}")
    return frame, list(ARM_AFFORDANCES), version_object


def arm_affordances_digest() -> str:
    """The run control: the digest of the frozen per-arm allowlist."""
    return _canonical_sha256(list(ARM_AFFORDANCES))


def probe_handshake(
    sley: Path,
    hello: Mapping[str, Any],
    scratch: Path,
    timeout_seconds: int = 30,
    serve_args: tuple[str, ...] = (),
) -> str:
    """The deterministic handshake identity, read from a hello-only invocation's report."""

    report = scratch / "probe-report.json"
    repository = scratch / "probe-repo"
    _run_sley(
        sley,
        ["serve", "--repository", str(repository), "--json", "--report", str(report), *serve_args],
        canonical_json_bytes(dict(hello)) + b"\n",
        timeout_seconds,
    )
    try:
        handshake = json.loads(report.read_text(encoding="utf-8"))["handshake_id"]
    except (OSError, ValueError, KeyError) as error:
        raise Sley2RunnerError(Sley2ErrorCode.HANDSHAKE_FAILED, "probe report") from error
    if not isinstance(handshake, str) or HEX_64.fullmatch(handshake) is None:
        _fail(Sley2ErrorCode.HANDSHAKE_FAILED, "no common profile")
    return handshake


# ---------------------------------------------------------------------------
# Trace
# ---------------------------------------------------------------------------


class Trace:
    """One create-once, append-only, digest-chained trace file."""

    def __init__(self, directory: Path, trial_id: str, previous_digest: str):
        self.path = directory / f"{trial_id}.trace.jsonl"
        self.previous = previous_digest
        self.count = 0
        try:
            self._descriptor = os.open(self.path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_APPEND, 0o600)
        except FileExistsError as error:
            raise Sley2RunnerError(Sley2ErrorCode.DUPLICATE, trial_id) from error
        except OSError as error:
            raise Sley2RunnerError(Sley2ErrorCode.TRACE_INVALID, str(error)) from error

    def close(self) -> None:
        if self._descriptor >= 0:
            os.fsync(self._descriptor)
            os.close(self._descriptor)
            self._descriptor = -1


def append_trace_record(trace: Trace, record: Mapping[str, Any]) -> str:
    """Chain one record; the trace is written before the runner proceeds."""

    body = dict(record)
    body["previous_record_digest"] = trace.previous
    digest = _sha256(TRACE_DOMAIN + canonical_json_bytes(body))
    stored = dict(body)
    stored["record_digest"] = digest
    payload = canonical_json_bytes(stored) + b"\n"
    written = 0
    while written < len(payload):
        count = os.write(trace._descriptor, payload[written:])
        if count <= 0:
            _fail(Sley2ErrorCode.TRACE_INVALID, "short write")
        written += count
    os.fsync(trace._descriptor)
    trace.previous = digest
    trace.count += 1
    return digest


def verify_trace(path: Path, manifest_digest_value: str) -> list[dict[str, Any]]:
    """Verify canonical bytes, chain, and record order; never artifact truth."""

    try:
        raw = path.read_bytes()
    except OSError as error:
        raise Sley2RunnerError(Sley2ErrorCode.TRACE_INVALID, str(error)) from error
    if not raw.endswith(b"\n"):
        _fail(Sley2ErrorCode.TRACE_INVALID, "truncated")
    records: list[dict[str, Any]] = []
    previous = manifest_digest_value
    next_seq = 0
    for index, line in enumerate(raw[:-1].split(b"\n")):
        try:
            stored = json.loads(line)
        except ValueError as error:
            raise Sley2RunnerError(Sley2ErrorCode.TRACE_INVALID, f"line {index}") from error
        if not isinstance(stored, dict) or canonical_json_bytes(stored) != line:
            _fail(Sley2ErrorCode.TRACE_INVALID, f"noncanonical line {index}")
        if stored.get("previous_record_digest") != previous:
            _fail(Sley2ErrorCode.TRACE_INVALID, f"chain break at {index}")
        body = {key: value for key, value in stored.items() if key != "record_digest"}
        if stored.get("record_digest") != _sha256(TRACE_DOMAIN + canonical_json_bytes(body)):
            _fail(Sley2ErrorCode.TRACE_INVALID, f"digest at {index}")
        kind = stored.get("kind")
        if index == 0 and kind != "header":
            _fail(Sley2ErrorCode.TRACE_INVALID, "missing header")
        if index > 0 and kind == "header":
            _fail(Sley2ErrorCode.TRACE_INVALID, "second header")
        if records and records[-1].get("kind") == "footer":
            _fail(Sley2ErrorCode.TRACE_INVALID, "record after footer")
        if kind == "frame":
            frame = stored.get("frame")
            if stored.get("seq") != next_seq or stored.get("direction") not in {"request", "response", "event"}:
                _fail(Sley2ErrorCode.TRACE_INVALID, f"sequence at {index}")
            if not isinstance(frame, dict) or stored.get("frame_sha256") != _canonical_sha256(frame):
                _fail(Sley2ErrorCode.TRACE_INVALID, f"frame digest at {index}")
            next_seq += 1
        elif kind == "guard_refusal":
            # A refusal the guard recorded before raising. It occupies a
            # sequence number like a frame, so a swallowed refusal cannot be
            # hidden by the counts agreeing without it.
            if stored.get("seq") != next_seq or not isinstance(stored.get("code"), str):
                _fail(Sley2ErrorCode.TRACE_INVALID, f"sequence at {index}")
            next_seq += 1
        elif kind == "footer":
            if stored.get("frames_recorded") != next_seq:
                _fail(Sley2ErrorCode.TRACE_INVALID, "footer count")
        elif kind != "header":
            _fail(Sley2ErrorCode.TRACE_INVALID, f"kind at {index}")
        records.append(stored)
        previous = stored["record_digest"]
    if not records or records[-1].get("kind") != "footer":
        _fail(Sley2ErrorCode.TRACE_INVALID, "missing footer")
    return records


def _whole_number(value: Any) -> int:
    """One non-negative integer, or zero for anything else."""
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        return 0
    return value


def derive_trace_metrics(records: list[dict[str, Any]]) -> dict[str, int]:
    """The ten metrics that come only from frame records (contract section 3)."""

    tool_calls = 0
    context_bytes = 0
    entities = 0
    relationships = 0
    validate_requests = 0
    for record in records:
        if record.get("kind") != "frame":
            continue
        frame = record["frame"]
        direction = record["direction"]
        if direction == "request":
            if frame.get("session") is not None and frame.get("method") != "session.close":
                tool_calls += 1
            if frame.get("method") == "candidate.validate":
                validate_requests += 1
            continue
        bounds = frame.get("bounds", {})
        entities += int(bounds.get("returned_entities", 0))
        relationships += int(bounds.get("returned_edges", 0))
        # Every body the agent received is context it read, whatever the method:
        # refs.list, handle.expand, candidate.inspect, compare, and
        # revision.read are context exactly as capsules are; denied methods
        # never reach the agent, so their bodies are never counted.
        # Counting only capsule and query.* understated the arm under test,
        # which master goal 21.6 forbids. capsule_bytes keeps the narrower
        # measure so the two are comparable rather than conflated.
        context_bytes += len(frame.get("body", "")) // 2
        if direction == "response" and frame.get("method") == "candidate.validate" and frame["flags"].get("failed"):
            # SMP1's `failed` flag marks a ProtocolFailure envelope: the request
            # itself did not complete. It is not an S20-360 candidate verdict,
            # which arrives as a successful response whose body says the
            # candidate is invalid. Counting it as `invalid_candidates` read
            # zero whenever candidates were genuinely invalid, always in the
            # arm's favour. It is counted in derive_context_breakdown as
            # protocol_failures, which is what it actually measures.
            pass
    return {
        "attempted_tasks": 1,
        "compile_or_check_attempts": validate_requests,
        "context_bytes": context_bytes,
        "entities_inspected": entities,
        "files_inspected": 0,
        "human_interventions": 0,
        "relationships_inspected": relationships,
        "tool_calls": tool_calls,
    }


def derive_context_breakdown(records: list[dict[str, Any]]) -> dict[str, int]:
    """How `context_bytes` divides, recorded by S20-620 rather than reported as a metric.

    `context_bytes` counts every body the agent received. The narrower capsule
    and `query.*` total is the number the earlier implementation reported as
    the whole, so keeping it visible is what makes the two comparable. It is
    not added to the shared benchmark plan's metric field set: that set is an
    S20-610 cross-arm fairness control, and widening it from inside one arm is
    the drift the S20-620 review objects to elsewhere.
    """

    context_bytes = 0
    capsule_bytes = 0
    protocol_failures = 0
    for record in records:
        if record.get("kind") != "frame" or record.get("direction") == "request":
            continue
        frame = record["frame"]
        body_bytes = len(frame.get("body", "")) // 2
        context_bytes += body_bytes
        if frame.get("method") in CONTEXT_METHODS:
            capsule_bytes += body_bytes
        if frame.get("flags", {}).get("failed"):
            protocol_failures += 1
    return {
        "context_bytes": context_bytes,
        "capsule_bytes": capsule_bytes,
        "non_capsule_context_bytes": context_bytes - capsule_bytes,
        "protocol_failures": protocol_failures,
    }


# ---------------------------------------------------------------------------
# Claims
# ---------------------------------------------------------------------------


def _arm_directory(run_directory: Path) -> Path:
    directory = run_directory / ARM_DIRECTORY
    try:
        directory.mkdir(mode=0o700, exist_ok=True)
    except OSError as error:
        raise Sley2RunnerError(Sley2ErrorCode.CLAIM_INVALID, str(error)) from error
    return directory


def _require_hex64(value: Any, field: str) -> None:
    if not isinstance(value, str) or HEX_64.fullmatch(value) is None:
        _fail(Sley2ErrorCode.CLAIM_INVALID, field)


def _validate_claim(record: Mapping[str, Any], manifest: Mapping[str, Any]) -> None:
    if set(record) != CLAIM_FIELDS:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "field set")
    try:
        canonical_json_bytes(dict(record))
    except RawRunnerError as error:
        raise Sley2RunnerError(Sley2ErrorCode.CLAIM_INVALID, "noncanonical") from error
    if record["contract"] != CLAIM_CONTRACT or record["run_id"] != manifest["run_id"]:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "contract/run")
    if not isinstance(record["trial_id"], str) or RUN_ID.fullmatch(record["trial_id"]) is None:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "trial_id")
    if record["arm_id"] != ARM:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "arm")
    _, corpus = _plan_and_corpus()
    if record["task_id"] not in _task_ids(corpus):
        _fail(Sley2ErrorCode.CLAIM_INVALID, "task")
    if record["seed"] not in manifest["random_seeds"]:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "seed")
    try:
        started = _require_utc_second(record["started_at_utc"], "started_at_utc")
        ended = _require_utc_second(record["ended_at_utc"], "ended_at_utc")
    except RawRunnerError as error:
        raise Sley2RunnerError(Sley2ErrorCode.CLAIM_INVALID, error.detail) from error
    if ended < started:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "time ordering")
    status = record["status"]
    if status not in TRIAL_STATUSES:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "status")
    if not isinstance(record["timeout"], bool) or record["timeout"] is not (status == "timeout"):
        _fail(Sley2ErrorCode.CLAIM_INVALID, "timeout")
    failure = record["failure_code"]
    if status == "accepted":
        if failure is not None:
            _fail(Sley2ErrorCode.CLAIM_INVALID, "accepted failure_code")
    elif not isinstance(failure, str) or not failure:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "missing failure_code")
    if status == "timeout" and failure != SYMBOLS[Sley2ErrorCode.TIMEOUT]:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "timeout failure_code")
    for field, expected in (
        ("evidence_status", EVIDENCE_STATUS),
        ("oracle_verification_status", VERIFICATION_STATUS),
        ("accounting_verification_status", VERIFICATION_STATUS),
    ):
        if record[field] != expected:
            _fail(Sley2ErrorCode.CLAIM_INVALID, field)
    for field in ("endpoint_sha256", "trace_head_digest", "fixture_digest", "exchange_digest", "prompt_digest"):
        _require_hex64(record[field], field)
    # The allowlist digest is a run control, not a decoration: it must be
    # a well-formed digest of exactly the frozen allowlist (contract
    # section 9), so a claim widened past the arm's surface never verifies.
    _require_hex64(record["arm_affordances_digest"], "arm_affordances_digest")
    if record["arm_affordances_digest"] != arm_affordances_digest():
        _fail(Sley2ErrorCode.CLAIM_INVALID, "arm_affordances_digest")
    if isinstance(record["trace_record_count"], bool) or not isinstance(record["trace_record_count"], int) or record["trace_record_count"] < 2:
        _fail(Sley2ErrorCode.CLAIM_INVALID, "trace_record_count")
    for field in ("handshake_id", "report_digest", "model_output_digest", "oracle_report_digest"):
        if record[field] is not None:
            _require_hex64(record[field], field)
    if status in {"accepted", "rejected"}:
        for field in ("handshake_id", "report_digest", "model_output_digest", "oracle_report_digest"):
            if record[field] is None:
                _fail(Sley2ErrorCode.CLAIM_INVALID, f"missing {field}")
    plan, _ = _plan_and_corpus()
    try:
        _validate_metrics(record["metrics"], plan, status)
        _validate_metric_controls(record["metrics"], manifest, status)
    except RawRunnerError as error:
        raise Sley2RunnerError(Sley2ErrorCode.CLAIM_INVALID, error.detail) from error


def _claim_digest(body: Mapping[str, Any]) -> str:
    return _sha256(CLAIM_DOMAIN + canonical_json_bytes(dict(body)))


def _parse_claims(raw: bytes, manifest: Mapping[str, Any]) -> list[dict[str, Any]]:
    if not raw:
        return []
    if not raw.endswith(b"\n"):
        _fail(Sley2ErrorCode.CLAIM_INVALID, "truncated chain")
    records: list[dict[str, Any]] = []
    previous = manifest_digest(manifest)
    for index, line in enumerate(raw[:-1].split(b"\n")):
        try:
            stored = json.loads(line)
        except ValueError as error:
            raise Sley2RunnerError(Sley2ErrorCode.CLAIM_INVALID, f"line {index}") from error
        if not isinstance(stored, dict) or canonical_json_bytes(stored) != line:
            _fail(Sley2ErrorCode.CLAIM_INVALID, f"noncanonical line {index}")
        if set(stored) != CLAIM_FIELDS | CHAIN_FIELDS:
            _fail(Sley2ErrorCode.CLAIM_INVALID, f"stored field set {index}")
        if stored["previous_record_digest"] != previous:
            _fail(Sley2ErrorCode.CLAIM_INVALID, f"chain break at {index}")
        body = {key: value for key, value in stored.items() if key != "record_digest"}
        if stored["record_digest"] != _claim_digest(body):
            _fail(Sley2ErrorCode.CLAIM_INVALID, f"digest at {index}")
        _validate_claim({key: value for key, value in stored.items() if key not in CHAIN_FIELDS}, manifest)
        records.append(stored)
        previous = stored["record_digest"]
    return records


def append_trial_claim(run_directory: Path, record: Mapping[str, Any]) -> str:
    """Append one unverified, denominator-preserving claim under the arm."""

    manifest = _read_manifest_exact(run_directory)
    _validate_claim(record, manifest)
    arm_directory = _arm_directory(run_directory)
    directory_descriptor = _open_run_directory(arm_directory)
    try:
        descriptor = _open_regular_at(directory_descriptor, CLAIMS_NAME, os.O_RDWR | os.O_CREAT | os.O_APPEND)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX)
            records = _parse_claims(_read_all(descriptor), manifest)
            if len(records) >= len(_expected_trial_pairs(manifest)):
                _fail(Sley2ErrorCode.DUPLICATE, "trial limit")
            if any(existing["trial_id"] == record["trial_id"] for existing in records):
                _fail(Sley2ErrorCode.DUPLICATE, str(record["trial_id"]))
            if any((existing["task_id"], existing["seed"]) == (record["task_id"], record["seed"]) for existing in records):
                _fail(Sley2ErrorCode.DUPLICATE, f"{record['task_id']}:{record['seed']}")
            previous = records[-1]["record_digest"] if records else manifest_digest(manifest)
            body = dict(record)
            body["previous_record_digest"] = previous
            digest = _claim_digest(body)
            stored = dict(body)
            stored["record_digest"] = digest
            payload = canonical_json_bytes(stored) + b"\n"
            written = 0
            while written < len(payload):
                count = os.write(descriptor, payload[written:])
                if count <= 0:
                    _fail(Sley2ErrorCode.CLAIM_INVALID, "short append")
                written += count
            os.fsync(descriptor)
            os.fsync(directory_descriptor)
            return digest
        finally:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
            os.close(descriptor)
    except RawRunnerError as error:
        raise Sley2RunnerError(Sley2ErrorCode.CLAIM_INVALID, error.detail) from error
    except OSError as error:
        raise Sley2RunnerError(Sley2ErrorCode.CLAIM_INVALID, str(error)) from error
    finally:
        os.close(directory_descriptor)


def verify_trial_claims(run_directory: Path, *, require_complete: bool = False) -> list[dict[str, Any]]:
    manifest = _read_manifest_exact(run_directory)
    arm_directory = run_directory / ARM_DIRECTORY
    if not arm_directory.is_dir():
        return []
    directory_descriptor = _open_run_directory(arm_directory)
    try:
        raw = _read_regular_at(directory_descriptor, CLAIMS_NAME, missing_ok=True)
    finally:
        os.close(directory_descriptor)
    records = _parse_claims(raw, manifest)
    if require_complete:
        expected = _expected_trial_pairs(manifest)
        actual = {(record["task_id"], record["seed"]) for record in records}
        if actual != expected:
            _fail(Sley2ErrorCode.DUPLICATE, f"{len(actual)}/{len(expected)}")
    return records


# ---------------------------------------------------------------------------
# One trial
# ---------------------------------------------------------------------------


def _utc_millis(started: str, ended: str) -> int:
    begin = datetime.strptime(started, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    end = datetime.strptime(ended, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    return max(0, int((end - begin).total_seconds() * 1000))


def _task(task_id: str) -> dict[str, Any]:
    _, corpus = _plan_and_corpus()
    for task in corpus["tasks"]:
        if task["id"] == task_id:
            return task
    _fail(Sley2ErrorCode.CLAIM_INVALID, "task")
    raise AssertionError("unreachable")


def run_scripted_trial(
    *,
    run_directory: Path,
    trial_id: str,
    task_id: str,
    seed: int,
    endpoint_factory: Callable[[Path, Path], Any],
    endpoint_digest: str,
    hello: Mapping[str, Any],
    affordances: list[str],
    endpoint_version: Mapping[str, Any],
    handshake_id: str,
    protocol_version: int,
    exchange_hex: str,
    fixture_digest: str,
    prompt_digest: str,
    agent: AgentAdapter,
    oracle: OracleAdapter,
    clock: AccountingClock,
) -> dict[str, Any]:
    """Run one trial: seed, open, hand the agent its handle, trace, claim."""

    # One immutable snapshot binds the whole trial: the guard, the handle,
    # and the claim digest below all use this tuple, so a caller that
    # mutates its list mid-trial cannot split exercised admission from the
    # claimed digest.
    admitted = tuple(affordances)
    # The snapshot is bound to the frozen allowlist, not merely carried:
    # a trial that does not run the nineteen-name surface stops here, so
    # drift in either direction fails closed (contract section 9).
    if _canonical_sha256(list(admitted)) != arm_affordances_digest():
        _fail(Sley2ErrorCode.HANDSHAKE_FAILED, "allowlist digest")
    # The frozen allowlist requires a version 2 offer, so every trial
    # stamps 2: a legacy stamp under the nineteen-name digest cannot
    # complete, and the endpoint's own selection record checks the stamp
    # below (contract sections 1 and 9).
    if protocol_version != 2:
        _fail(Sley2ErrorCode.HANDSHAKE_FAILED, "protocol version")
    manifest = _read_manifest_exact(run_directory)
    run_manifest_digest = manifest_digest(manifest)
    arm_directory = _arm_directory(run_directory)
    task = _task(task_id)
    started = clock.now_utc()
    workspace = Path(tempfile.mkdtemp(prefix=f"{trial_id}-", dir=arm_directory))
    repository = workspace / "repo"
    report_path = workspace / "report.json"
    trace = Trace(arm_directory, trial_id, run_manifest_digest)
    append_trace_record(
        trace,
        {
            "endpoint_sha256": endpoint_digest,
            "endpoint_version": dict(endpoint_version),
            "fixture_digest": fixture_digest,
            "handshake_id": handshake_id,
            "kind": "header",
            "protocol_version": protocol_version,
            "run_manifest_digest": run_manifest_digest,
            "seed": seed,
            "task_id": task_id,
            "trial_id": trial_id,
        },
    )
    state: dict[str, Any] = {"seq": 0, "next_request": 1, "session": None, "violations": []}
    outcome = "completed"
    failure_code: str | None = None
    observation: Mapping[str, Any] = {}
    endpoint = None
    exit_status: int | None = None

    def record(direction: str, frame: Mapping[str, Any]) -> None:
        append_trace_record(
            trace,
            {
                "direction": direction,
                "frame": dict(frame),
                "frame_sha256": _canonical_sha256(dict(frame)),
                "kind": "frame",
                "seq": state["seq"],
            },
        )
        state["seq"] += 1

    def transact(frame: Mapping[str, Any]) -> list[dict[str, Any]]:
        record("request", frame)
        replies = endpoint.send(frame)
        for reply in replies:
            record("event" if reply["kind"] == "event" else "response", reply)
        return replies

    def runner_request(method: str, body_hex: str, request_id: int | None = None) -> dict[str, Any]:
        # The pre-session space carries identifier 0 only and shares no
        # counter (SMP1 section 3): an explicit identifier bypasses the
        # session counter, which starts at 1 for the first bound request.
        if request_id is None:
            request_id = state["next_request"]
            state["next_request"] += 1
        frame = request_frame(
            method, body_hex, state["session"], request_id,
            protocol_version=protocol_version,
        )
        return transact(frame)[-1]

    def refuse(code: Sley2ErrorCode, detail: str) -> None:
        """Record the refusal, then raise it.

        The guard raises into the agent's frame, so an adapter can catch it and
        carry on. Recording first, and remembering that it happened, is what
        makes swallowing useless: the attempt is in the trace and the trial
        cannot be reported as completed.
        """
        state["violations"].append({"code": code.name, "detail": detail})
        append_trace_record(
            trace,
            {
                "code": code.name,
                "detail": detail,
                "kind": "guard_refusal",
                "seq": state["seq"],
            },
        )
        state["seq"] += 1
        _fail(code, detail)

    def guarded_exchange(request: Mapping[str, Any]) -> list[dict[str, Any]]:
        if not isinstance(request, Mapping) or not set(request) <= AGENT_REQUEST_FIELDS:
            refuse(Sley2ErrorCode.PRIVILEGED_CONTEXT, "agent request names a runner field")
        method = request.get("method")
        body = request.get("body", "")
        cancel = request.get("cancel", False)
        if not isinstance(cancel, bool):
            refuse(Sley2ErrorCode.FRAME_INVALID, "cancel")
        if method not in admitted:
            refuse(Sley2ErrorCode.FRAME_INVALID, f"method {method!r}")
        if not isinstance(body, str) or len(body) % 2 or any(ch not in "0123456789abcdef" for ch in body):
            refuse(Sley2ErrorCode.FRAME_INVALID, "body")
        if state["session"] is None:
            refuse(Sley2ErrorCode.PRIVILEGED_CONTEXT, "no session")
        frame = request_frame(
            method, body, state["session"], state["next_request"], cancel,
            protocol_version=protocol_version,
        )
        state["next_request"] += 1
        return [dict(reply) for reply in transact(frame)]

    try:
        endpoint = endpoint_factory(repository, report_path)
        greeting = transact(hello)
        if greeting[-1]["kind"] != "hello":
            _fail(Sley2ErrorCode.HANDSHAKE_FAILED, "negotiation refused")
        seeded = runner_request("exchange.import", exchange_hex, 0)
        if seeded["flags"].get("failed"):
            _fail(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, "exchange.import refused")
        opened = runner_request("session.open", handshake_id, 0)
        if opened["flags"].get("failed") or not isinstance(opened.get("body"), str) or HEX_64.fullmatch(opened["body"]) is None:
            _fail(Sley2ErrorCode.HANDSHAKE_FAILED, "session.open refused")
        state["session"] = opened["body"]
        handle = EndpointHandle(guarded_exchange, admitted)
        if handle_surface(handle) != HANDLE_SURFACE:
            _fail(Sley2ErrorCode.PRIVILEGED_CONTEXT, "handle surface")
        # The declared surface is not the whole reachable surface: a bound
        # method carries its defining module's globals. This is the part that
        # can be held to zero and checked, so it is checked rather than assumed.
        if reflected_privileged_names(handle):
            _fail(Sley2ErrorCode.PRIVILEGED_CONTEXT, "handle reflection")
        try:
            observation = agent.run(handle)
        except Sley2RunnerError:
            raise
        except Exception as error:  # noqa: BLE001 - adapter failures are harness failures
            raise Sley2RunnerError(Sley2ErrorCode.INTERNAL_INVARIANT, type(error).__name__) from error
        # A refusal the adapter swallowed still ends the trial: the guard fired,
        # so the agent attempted a privileged context or an invalid frame, and
        # returning normally afterwards does not make the attempt go away.
        if state["violations"]:
            first = state["violations"][0]
            _fail(Sley2ErrorCode[first["code"]], f"swallowed: {first['detail']}")
        runner_request("session.close", "")
        state["session"] = None
    except Sley2RunnerError as error:
        outcome = "harness_failure"
        failure_code = error.symbol
    finally:
        if endpoint is not None:
            exit_status, _ = endpoint.close()
    report_object: Any = None
    try:
        report_object = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        report_object = None
    # The stamp is bound to the negotiated selection through the
    # endpoint's own record: a trial whose frames were stamped anything
    # but the reported selection fails closed here, not somewhere
    # downstream under a mislabeled code (contract section 1).
    selected = report_object.get("selected_protocol_version") if isinstance(report_object, dict) else None
    if selected != protocol_version and outcome == "completed":
        outcome = "harness_failure"
        failure_code = SYMBOLS[Sley2ErrorCode.FRAME_INVALID]
    append_trace_record(
        trace,
        {
            "endpoint_exit_status": exit_status,
            "failure_code": failure_code,
            "frames_recorded": state["seq"],
            "kind": "footer",
            "outcome": outcome,
            "report": report_object,
        },
    )
    head = trace.previous
    trace.close()
    records = verify_trace(trace.path, run_manifest_digest)
    derived = derive_trace_metrics(records)
    if outcome == "completed":
        judgement = oracle.evaluate(task, records)
        status = str(judgement.get("status"))
        if status not in {"accepted", "rejected"}:
            status = "harness_failure"
            failure_code = SYMBOLS[Sley2ErrorCode.INTERNAL_INVARIANT]
        elif status == "rejected":
            failure_code = str(judgement.get("failure_code") or "ORACLE_REJECTED")
    else:
        judgement = {}
        status = "harness_failure"
    ended = clock.now_utc()
    plan, _ = _plan_and_corpus()
    metrics: dict[str, Any] = {name: 0 for name in plan["metrics"]}
    metrics.update(derived)
    # Every injected metric has exactly one source. The adapter's observation
    # used to take precedence over the oracle's judgement for the correctness
    # metrics, which let the arm under test grade itself and report zero
    # against its own interest.
    for name in ADAPTER_OWNED_METRICS:
        metrics[name] = _whole_number(observation.get(name))
    for name in ORACLE_OWNED_METRICS:
        metrics[name] = _whole_number(judgement.get(name))
    metrics["total_observable_tokens"] = metrics["model_input_tokens"] + metrics["model_output_tokens"]
    metrics["wall_time"] = _utc_millis(started, ended)
    metrics["strict_accepted_correctness"] = status == "accepted"
    metrics["accepted_correct_changes"] = 1 if status == "accepted" else 0
    metrics["accepted_change_tokens"] = None
    claim = {
        "accounting_verification_status": VERIFICATION_STATUS,
        "arm_affordances_digest": _canonical_sha256(list(admitted)),
        "arm_id": ARM,
        "contract": CLAIM_CONTRACT,
        "endpoint_sha256": endpoint_digest,
        "ended_at_utc": ended,
        "evidence_status": EVIDENCE_STATUS,
        "exchange_digest": _sha256(bytes.fromhex(exchange_hex)),
        "failure_code": failure_code,
        "fixture_digest": fixture_digest,
        "handshake_id": handshake_id,
        "metrics": metrics,
        "model_output_digest": _canonical_sha256(dict(observation)) if observation else None,
        "oracle_report_digest": _canonical_sha256(dict(judgement)) if judgement else None,
        "oracle_verification_status": VERIFICATION_STATUS,
        "prompt_digest": prompt_digest,
        "report_digest": _canonical_sha256(report_object) if isinstance(report_object, dict) else None,
        "run_id": manifest["run_id"],
        "seed": seed,
        "started_at_utc": started,
        "status": status,
        "task_id": task_id,
        "timeout": status == "timeout",
        "trace_head_digest": head,
        "trace_record_count": len(records),
        "trial_id": trial_id,
    }
    claim_digest = append_trial_claim(run_directory, claim)
    return {
        "claim_digest": claim_digest,
        "endpoint_exit_status": exit_status,
        "failure_code": failure_code,
        "metrics": metrics,
        "outcome": outcome,
        "report": report_object,
        "status": status,
        "trace_head_digest": head,
        "trace_path": str(trace.path),
        "trace_records": len(records),
        "trial_id": trial_id,
    }


# ---------------------------------------------------------------------------
# Scripted smoke arm (no model, no task attempt)
# ---------------------------------------------------------------------------


class ScriptedAgent:
    """Asks the repository four bounded questions through the handle; attempts no task."""

    def __init__(self, expand_body: str = "00") -> None:
        # The expand body names handle 0 under the head root the smoke
        # discovers through a throwaway serve (contract section 5); the
        # "00" default is for offline tests whose fake endpoint answers
        # regardless of the body.
        self._script = (("session.capabilities", ""), ("refs.list", "10"), ("handle.expand", expand_body), ("session.budgets", ""))

    @property
    def script(self) -> tuple:
        return self._script

    def run(self, handle: EndpointHandle) -> Mapping[str, Any]:
        transcript = []
        offered = handle.affordances()
        for method, body in self._script:
            if method not in offered:
                _fail(Sley2ErrorCode.FRAME_INVALID, method)
            transcript.append(handle.exchange({"method": method, "body": body}))
        return {"model_input_tokens": 0, "model_output_tokens": 0, "transcript_sha256": _canonical_sha256(transcript)}


class ScriptedOracle:
    """Rejects every scripted trial: no task was attempted."""

    def evaluate(self, task: Mapping[str, Any], records: list[dict[str, Any]]) -> Mapping[str, Any]:
        return {"failure_code": "SLEY2_SMOKE_NO_TASK_ATTEMPTED", "frames": sum(1 for r in records if r.get("kind") == "frame"), "status": "rejected", "task_id": task["id"]}


class SystemClock:
    def now_utc(self) -> str:
        return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def smoke_manifest(run_id: str, created_at: str, repo_commit: str, exchange_digest: str) -> dict[str, Any]:
    plan = json.loads(PLAN_PATH.read_text(encoding="utf-8"))
    frozen = {arm["id"]: arm.get("artifact_sha256") for arm in plan["arms"] if arm.get("required")}
    unfrozen = "0" * 64
    return {
        "action_budget": 100,
        "arm_fixture_digests": {
            "raw_files": unfrozen,
            "sley_1_2_0": frozen["sley_1_2_0"] or unfrozen,
            "sley_2_0": exchange_digest,
        },
        "benchmark_plan_digest": _sha256(PLAN_PATH.read_bytes()),
        "cache_state": {"mode": "cold"},
        "context_budget": 1_000_000,
        "contract": "sley2.raw-run-manifest.v1",
        "corpus_digest": _sha256(CORPUS_PATH.read_bytes()),
        "corpus_version": 1,
        "created_at_utc": created_at,
        "environment_manifest": {"network": "not-isolated", "scope": "scripted-smoke"},
        "execution_mode": "offline_injected",
        "external_command_policy": "forbidden",
        "hardware_manifest": {"machine": "local-smoke"},
        "model_configuration": {"scripted": True},
        "model_exact_version": "scripted-agent-v1",
        "model_provider": "none-scripted",
        "oracle_digest": unfrozen,
        "random_seeds": [1],
        "repo_commit": repo_commit,
        "retry_policy": {"maximum_retries": 0},
        "run_id": run_id,
        "task_statement_digest": task_statement_digest(),
        "tool_description_digests": {
            "raw_files": unfrozen,
            "sley_1_2_0": unfrozen,
            "sley_2_0": _sha256(METHOD_TABLE.read_bytes()),
        },
        "trial_count": 1,
        "wall_time_budget": 600_000,
    }


def _git_head() -> str:
    head = ROOT / ".git" / "HEAD"
    try:
        reference = head.read_text(encoding="utf-8").strip()
        if reference.startswith("ref: "):
            return (ROOT / ".git" / reference[5:]).read_text(encoding="utf-8").strip()
        return reference
    except OSError:
        return "0" * 40


def _failure_code(body_hex: str) -> int:
    """The failure code carried by a response body: field 1 of the SCB
    failure record (record of 6, tag 1, sized uvarint). Fails closed on
    any shape deviation rather than guessing."""
    try:
        body = bytes.fromhex(body_hex)
    except ValueError as error:
        raise Sley2RunnerError(Sley2ErrorCode.FRAME_INVALID, "failure body hex") from error
    values = list(body)
    if len(values) < 4 or values[0] != 6 or values[1] != 1:
        raise Sley2RunnerError(Sley2ErrorCode.FRAME_INVALID, "failure record shape")
    length = values[2]
    if length >= 0x80 or len(values) < 3 + length:
        raise Sley2RunnerError(Sley2ErrorCode.FRAME_INVALID, "failure code length")
    code, shift, index = 0, 0, 3
    for _ in range(length):
        byte = values[index]
        index += 1
        code |= (byte & 0x7F) << shift
        shift += 7
        if not byte & 0x80:
            break
    else:
        raise Sley2RunnerError(Sley2ErrorCode.FRAME_INVALID, "failure code uvarint")
    return code

def v1_rejection_shape(answer: Mapping[str, Any]) -> bool:
    """Control shape predicate: the frozen v1 surface answers 306 with the
    endpoint's own rejection, which names no method, session, or request
    identifier and sets the failed bit (SMP1 section 6)."""
    return (
        answer.get("method") == ""
        and answer.get("session") is None
        and answer.get("request_id") == 0
        and bool(answer.get("flags", {}).get("failed"))
    )


def v2_dispatched(v2: Mapping[str, Any]) -> bool:
    """Allowlist predicate for the live dispatch proof (contract section 5):
    the version 2 arm must answer the empty-body entity.version with exactly
    the body-layer refusal (40008), proving the request dispatched past the
    method layer. Any other code (method-unsupported, downgrade, the
    bridge's own mapping, a generic shape code) or a nonzero exit proves
    nothing and answers False: the predicate fails closed."""
    return v2.get("failure_code") == 40008 and v2.get("endpoint_exit_status") == 0


def _scb_uvar(data: bytes, offset: int) -> tuple[int, int]:
    """One LEB128 unsigned varint with its end offset; fails closed."""
    value = shift = 0
    while True:
        if offset >= len(data) or shift >= 64:
            _fail(Sley2ErrorCode.FRAME_INVALID, "uvarint")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, offset
        shift += 7


def _scb_record_fields(data: bytes, expected: int) -> list[bytes]:
    """The sized field payloads of one ordered SCB record in tag order;
    fails closed on any shape deviation."""
    count, offset = _scb_uvar(data, 0)
    if count != expected:
        _fail(Sley2ErrorCode.FRAME_INVALID, "record arity")
    fields: list[bytes] = []
    for tag in range(1, expected + 1):
        seen, offset = _scb_uvar(data, offset)
        if seen != tag:
            _fail(Sley2ErrorCode.FRAME_INVALID, "record tag")
        length, offset = _scb_uvar(data, offset)
        if offset + length > len(data):
            _fail(Sley2ErrorCode.FRAME_INVALID, "record length")
        fields.append(data[offset : offset + length])
        offset += length
    if offset != len(data):
        _fail(Sley2ErrorCode.FRAME_INVALID, "record trailing")
    return fields


def discover_expand_body(
    sley: Path,
    scratch: Path,
    timeout_seconds: int,
    exchange_hex: str,
    accepted_head_tx: str,
) -> str:
    """The handle.expand body the scripted trial uses: handle 0 under the
    head root, discovered through a throwaway serve rather than hardcoded
    or read from repository files. Seeds a disposable repository, opens a
    session, reads the fixture-pinned accepted head's revision summary for
    its state root, and trial-expands handle 0 under it; any step failing
    stops the smoke loudly instead of scripting a malformed body. Returns
    the hex body (uvar handle 0 followed by the 32-byte root)."""
    try:
        head_tx = bytes.fromhex(accepted_head_tx)
    except ValueError as error:
        raise Sley2RunnerError(Sley2ErrorCode.MANIFEST_INVALID, "accepted head") from error
    if len(head_tx) != 32:
        _fail(Sley2ErrorCode.MANIFEST_INVALID, "accepted head")
    hello, _, _ = endpoint_offer(sley, timeout_seconds)
    handshake = probe_handshake(sley, hello, scratch, timeout_seconds, PROFILE_ARGS)
    repository = scratch / "expand-discovery-repo"
    endpoint = Endpoint(
        sley, repository, scratch / "expand-discovery-report.json",
        timeout_seconds, PROFILE_ARGS,
    )
    try:
        greeting = endpoint.send(hello)
        if greeting[-1]["kind"] != "hello":
            raise Sley2RunnerError(Sley2ErrorCode.HANDSHAKE_FAILED, "discovery offer refused")
        seeded = endpoint.send(request_frame("exchange.import", exchange_hex, None, 0, protocol_version=2))[-1]
        if seeded["kind"] != "response" or seeded["flags"].get("failed"):
            raise Sley2RunnerError(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, "discovery seed refused")
        opened = endpoint.send(request_frame("session.open", handshake, None, 0, protocol_version=2))[-1]
        if opened["kind"] != "response" or opened["flags"].get("failed"):
            raise Sley2RunnerError(Sley2ErrorCode.HANDSHAKE_FAILED, "discovery open refused")
        session = opened.get("body")
        if not isinstance(session, str):
            raise Sley2RunnerError(Sley2ErrorCode.HANDSHAKE_FAILED, "discovery open body")
        summary = endpoint.send(request_frame("revision.read", head_tx.hex(), session, 1, protocol_version=2))[-1]
        if summary["kind"] != "response" or summary["flags"].get("failed"):
            raise Sley2RunnerError(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, "discovery revision refused")
        try:
            fields = _scb_record_fields(bytes.fromhex(summary.get("body", "")), 8)
        except ValueError as error:
            raise Sley2RunnerError(Sley2ErrorCode.FRAME_INVALID, "discovery summary hex") from error
        if len(fields[1]) != 32:
            raise Sley2RunnerError(Sley2ErrorCode.FRAME_INVALID, "discovery state root")
        body = "00" + fields[1].hex()
        trial = endpoint.send(request_frame("handle.expand", body, session, 2, protocol_version=2))[-1]
        if trial["kind"] != "response" or trial["flags"].get("failed"):
            raise Sley2RunnerError(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, "discovery expand refused")
        return body
    finally:
        endpoint.close()


def entity_read_round_trip(sley: Path, scratch: Path, timeout_seconds: int) -> dict[str, Any]:
    """Live serving proof for the version 2 entity reads, plus the version 1
    negative control. Seeds the repository, opens a session, and sends
    `entity.version` with an empty body: under a version 2 selection the
    method dispatches past the method layer, proving it with the body-layer
    refusal (40008); under a version 1 selection the frozen surface cannot
    even name the method, so the endpoint answers its own rejection
    (`JSON_BRIDGE_METHOD_UNKNOWN`, 42003) and nothing reaches dispatch.
    Pre-session frames carry identifier 0 (SMP1 section 3); the session-bound
    read carries 1. Not a demonstration (that is AT-MW-02 I4b): the empty
    body cannot read anything."""
    vector = json.loads(EXCHANGE_FIXTURE.read_text(encoding="utf-8"))["vectors"][0]
    exchange_hex = vector["exchange_hex"]
    evidence: dict[str, Any] = {}
    for name, offer_args, serve_args, version in (
        ("v2", ("--protocol-profile", "v2-capable"), PROFILE_ARGS, 2),
        ("v1", (), (), 1),
    ):
        evidence[name] = {}
        hello_bytes = _run_sley(sley, ["hello", *offer_args], b"", timeout_seconds)
        decoded = _run_sley(sley, ["frame", "decode"], hello_bytes, timeout_seconds)
        hello_object = json.loads(decoded.decode("utf-8").strip())
        repository = scratch / f"entity-read-{name}-repo"
        endpoint = Endpoint(
            sley, repository, scratch / f"entity-read-{name}-report.json",
            timeout_seconds, serve_args,
        )
        try:
            greeting = endpoint.send(hello_object)
            if greeting[-1]["kind"] != "hello":
                raise Sley2RunnerError(Sley2ErrorCode.HANDSHAKE_FAILED, f"{name} offer refused")
            handshake = probe_handshake(sley, hello_object, scratch, timeout_seconds, serve_args)
            seeded = endpoint.send(request_frame("exchange.import", exchange_hex, None, 0, protocol_version=version))[-1]
            if seeded["kind"] != "response" or seeded["flags"].get("failed"):
                raise Sley2RunnerError(Sley2ErrorCode.ENDPOINT_UNAVAILABLE, f"{name} seed refused")
            opened = endpoint.send(request_frame("session.open", handshake, None, 0, protocol_version=version))[-1]
            if opened["kind"] != "response" or opened["flags"].get("failed"):
                raise Sley2RunnerError(Sley2ErrorCode.HANDSHAKE_FAILED, f"{name} open refused")
            session = opened.get("body")
            if not isinstance(session, str):
                raise Sley2RunnerError(Sley2ErrorCode.HANDSHAKE_FAILED, f"{name} open body")
            answer = endpoint.send(request_frame("entity.version", "", session, 1, protocol_version=version))[-1]
            if answer["kind"] != "response":
                raise Sley2RunnerError(Sley2ErrorCode.INTERNAL_INVARIANT, f"{name} entity read not answered")
            if version == 1:
                # Frozen surface: the rejection names no method, session, or
                # request identifier, carries the bridge's unknown-method
                # code, and sets the failed bit (SMP1 section 6), proving
                # 306 never reached dispatch.
                if not v1_rejection_shape(answer):
                    raise Sley2RunnerError(Sley2ErrorCode.INTERNAL_INVARIANT, f"{name} entity read not rejected")
                code = _failure_code(answer.get("body", ""))
                evidence[name] = {"failure_code": code}
                continue
            if not answer["flags"].get("failed"):
                raise Sley2RunnerError(Sley2ErrorCode.INTERNAL_INVARIANT, f"{name} entity read unexpectedly succeeded")
            # The answer must echo the session-bound request it answers;
            # a rejection that names no method, session, or identifier is
            # the endpoint's own, not a dispatch past the method layer.
            if (
                answer.get("method") != "entity.version"
                or answer.get("session") != session
                or answer.get("request_id") != 1
            ):
                raise Sley2RunnerError(Sley2ErrorCode.INTERNAL_INVARIANT, f"{name} entity read not dispatched")
            code = _failure_code(answer.get("body", ""))
            evidence[name] = {"failure_code": code}
        finally:
            exit_status, _ = endpoint.close()
            evidence[name]["endpoint_exit_status"] = exit_status
    v2, v1 = evidence["v2"], evidence["v1"]
    evidence["v2_dispatched"] = v2_dispatched(v2)
    evidence["v1_still_gates"] = (
        v1.get("failure_code") == 42003
        and v1.get("endpoint_exit_status") == 0
    )
    return evidence


def smoke(sley: Path, evidence_directory: Path, timeout_seconds: int) -> int:
    clock = SystemClock()
    started = clock.now_utc()
    evidence_directory.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_id = f"sley2-smoke-{stamp.lower()}"
    run_directory = evidence_directory / f"run-{stamp}"
    evidence: dict[str, Any] = {
        "contract": "s20-620-sley2-runner-smoke-v1",
        "work_package": "S20-620",
        "scope": "SCRIPTED_AGENT_OVER_EXCHANGE_FIXTURE_NO_MODEL_NO_TASK",
        "full_s20_620_complete": False,
        "provider_or_model_execution": False,
        "actual_trials": 0,
        "started_at_utc": started,
        "endpoint": str(sley.relative_to(ROOT)) if sley.is_relative_to(ROOT) else str(sley.name),
        "run_directory": str(run_directory.relative_to(ROOT)) if run_directory.is_relative_to(ROOT) else run_directory.name,
        "problems": [],
    }
    try:
        vector = json.loads(EXCHANGE_FIXTURE.read_text(encoding="utf-8"))["vectors"][0]
        exchange_hex = vector["exchange_hex"]
        exchange_digest = _sha256(bytes.fromhex(exchange_hex))
        manifest = smoke_manifest(run_id, started, _git_head(), exchange_digest)
        validate_run_manifest(manifest)
        run_manifest_digest = write_run_manifest(run_directory, manifest)
        digest = endpoint_sha256(sley)
        hello, affordances, version = endpoint_offer(sley, timeout_seconds)
        scratch = Path(tempfile.mkdtemp(prefix="probe-", dir=run_directory))
        handshake = probe_handshake(sley, hello, scratch, timeout_seconds, PROFILE_ARGS)
        # The scripted expand body names handle 0 under the head root the
        # smoke discovers from the fixture-pinned accepted head: a fixed
        # historical body cannot satisfy the expected-root rule, so the
        # smoke learns a valid one instead of scripting a refusal.
        expand_body = discover_expand_body(
            sley, scratch, timeout_seconds, exchange_hex,
            vector.get("accepted_head_transaction_id_hex", ""),
        )
        agent = ScriptedAgent(expand_body)
        evidence.update(
            {
                "endpoint_sha256": digest,
                "endpoint_version": version,
                "affordances": len(affordances),
                "handshake_id": handshake,
                "run_manifest_digest": run_manifest_digest,
            }
        )
        # Guard: a handle with extra surface and a request naming a runner field are refused.
        class Leaky(EndpointHandle):
            __slots__ = ()

            def repository(self) -> str:
                return "leak"

        evidence["guard"] = {
            "handle_surface": sorted(handle_surface(EndpointHandle(lambda r: [], affordances))),
            "leaky_handle_refused": handle_surface(Leaky(lambda r: [], affordances)) != HANDLE_SURFACE,
            "reflected_privileged_names": sorted(
                reflected_privileged_names(EndpointHandle(lambda r: [], affordances))
            ),
            "reflection_residual": "exchange closure cells remain reachable; section 2 states this",
        }
        summary = run_scripted_trial(
            run_directory=run_directory,
            trial_id="smoke-trial-001",
            task_id=sorted(_task_ids(json.loads(CORPUS_PATH.read_text(encoding="utf-8"))))[0],
            seed=1,
            endpoint_factory=lambda repository, report: Endpoint(sley, repository, report, timeout_seconds, PROFILE_ARGS),
            endpoint_digest=digest,
            hello=hello,
            affordances=affordances,
            endpoint_version=version,
            handshake_id=handshake,
            protocol_version=2,
            exchange_hex=exchange_hex,
            fixture_digest=exchange_digest,
            prompt_digest=_canonical_sha256({"scripted": [list(step) for step in agent.script]}),
            agent=agent,
            oracle=ScriptedOracle(),
            clock=clock,
        )
        evidence["trial"] = summary
        claims = verify_trial_claims(run_directory)
        evidence["claims_verified"] = len(claims)

        class Intruder:
            def run(self, handle: EndpointHandle) -> Mapping[str, Any]:
                handle.exchange({"method": "session.capabilities", "body": "", "session": "00" * 32})
                return {}

        intrusion = run_scripted_trial(
            run_directory=run_directory,
            trial_id="smoke-trial-guard",
            task_id=sorted(_task_ids(json.loads(CORPUS_PATH.read_text(encoding="utf-8"))))[1],
            seed=1,
            endpoint_factory=lambda repository, report: Endpoint(sley, repository, report, timeout_seconds, PROFILE_ARGS),
            endpoint_digest=digest,
            hello=hello,
            affordances=affordances,
            endpoint_version=version,
            handshake_id=handshake,
            protocol_version=2,
            exchange_hex=exchange_hex,
            fixture_digest=exchange_digest,
            prompt_digest=_canonical_sha256({"intruder": True}),
            agent=Intruder(),
            oracle=ScriptedOracle(),
            clock=clock,
        )
        evidence["guard"]["runner_field_refused"] = intrusion["failure_code"] == SYMBOLS[Sley2ErrorCode.PRIVILEGED_CONTEXT]
        evidence["guard_trial"] = {"status": intrusion["status"], "failure_code": intrusion["failure_code"], "trace_records": intrusion["trace_records"]}
        round_trip = entity_read_round_trip(
            sley, Path(tempfile.mkdtemp(prefix="entity-read-", dir=run_directory)), timeout_seconds
        )
        evidence["v2_entity_read_round_trip"] = round_trip
        checks = {
            "trial_completed": summary["outcome"] == "completed" and summary["status"] == "rejected",
            "endpoint_exit_zero": summary["endpoint_exit_status"] == 0,
            "tool_calls_four": summary["metrics"]["tool_calls"] == 4,
            "no_failed_answers": isinstance(summary["report"], dict) and summary["report"].get("failed_answers") == 0,
            "claims_two": len(verify_trial_claims(run_directory)) == 2,
            "guard_refused": bool(evidence["guard"]["runner_field_refused"]) and evidence["guard"]["leaky_handle_refused"],
            "v2_entity_read_dispatched": bool(round_trip["v2_dispatched"]),
            "v1_entity_read_still_gated": bool(round_trip["v1_still_gates"]),
        }
        evidence["checks"] = checks
        for name, passed in checks.items():
            if not passed:
                evidence["problems"].append(name)
    except (Sley2RunnerError, RawRunnerError) as error:
        evidence["problems"].append(f"{type(error).__name__}:{error}")
    except Exception as error:  # noqa: BLE001 - the evidence file must always record the failure
        evidence["problems"].append(f"UNEXPECTED:{type(error).__name__}:{error}")
    evidence["ended_at_utc"] = clock.now_utc()
    evidence["result"] = "PASS" if not evidence["problems"] else "FAIL"
    (evidence_directory / "evidence.json").write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({key: evidence[key] for key in ("contract", "result", "problems", "run_directory")}, indent=2, sort_keys=True))
    return 0 if evidence["result"] == "PASS" else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    smoke_parser = commands.add_parser("smoke", help="run the scripted smoke trial over the exchange fixture")
    smoke_parser.add_argument("--sley", type=Path, default=DEFAULT_SLEY)
    smoke_parser.add_argument("--evidence-dir", type=Path, required=True)
    smoke_parser.add_argument("--timeout-seconds", type=int, default=60)
    verify_parser = commands.add_parser("verify", help="verify a run directory's claim chain and traces")
    verify_parser.add_argument("--run", type=Path, required=True)
    verify_parser.add_argument("--complete", action="store_true")
    arguments = parser.parse_args(argv)
    if arguments.command == "smoke":
        return smoke(arguments.sley.resolve(), arguments.evidence_dir.resolve(), arguments.timeout_seconds)
    try:
        records = verify_trial_claims(arguments.run, require_complete=arguments.complete)
        manifest = _read_manifest_exact(arguments.run)
        traces = 0
        for record in records:
            path = arguments.run / ARM_DIRECTORY / f"{record['trial_id']}.trace.jsonl"
            head = verify_trace(path, manifest_digest(manifest))[-1]["record_digest"]
            if head != record["trace_head_digest"]:
                _fail(Sley2ErrorCode.TRACE_INVALID, record["trial_id"])
            traces += 1
    except (Sley2RunnerError, RawRunnerError) as error:
        print(json.dumps({"result": "FAIL", "problem": str(error)}, sort_keys=True))
        return 1
    print(json.dumps({"claims": len(records), "result": "PASS", "traces": traces}, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
