#!/usr/bin/env python3
"""Confined mediated sley_2_0 attempt path: campaign integration.

This wires the implemented protected-capture machinery (trusted
capture, bwrap confinement, mediated gateway, real ``sley2_tool``
Sessions on runner-owned state) into the actual campaign attempt
path for the sley_2_0 arm. It builds no second capture system: every
agent interaction crosses ``MediatedSleyEndpoint.handle`` inside
``TrustedCapture.exchange``, and acceptance routes through
``adjudicate`` (reconciled capture AND trusted oracle).

Attempt shape (sley_2_0 only; other arms keep their existing path):
- Runner-owned ``protected/`` holds the staged starting state; the
  gateway drives real Sessions on it. The agent process is confined
  (bwrap): protected prefixes (trial state, capture directory,
  artifact store) are masked by private tmpfs. Its sole channel is a
  unix socket to the runner-side gateway server; the staged
  ``.sley-live/sley-tool`` shim forwards the documented tool surface
  over that socket with the identical CLI contract.
- Frozen start bindings (pack, task manifest, tool version, binary
  identity) and budget caps are written to the capture BEFORE the
  trial agent can interact. Caps derive from the run manifest:
  action budget bounds trial exchanges, wall budget bounds trial
  wall time; response caps stay at the capture defaults.
- The final artifact is obtained from protected state only
  (``protected_final``). Scratch diagnostics (including any
  candidate-side transcript/usage/final files) are never consulted:
  the runner-owned evidence slots carry capture-derived bytes
  (exchanges transcript, cumulative usage ledger, protected final)
  plus the existing completion binding naming exactly those
  digests, so the record-verification path is unchanged.
- One fail-closed ingress: a malformed or oversized frame, or an
  uncaptured gateway reply, permanently invalidates the attempt
  (``MEDIATED_INGRESS_INVALID`` / ``MEDIATED_FRAME_OVERSIZED`` /
  ``MEDIATED_CAPTURE_FAILED``). The session never continues toward
  acceptance as though the missing exchange never happened.
- Failed, timed-out, interrupted, and incomplete attempts are
  retained under the existing accounting contract (timeout /
  harness_failure / rejected with the established codes); missing
  observations stay missing, never measured zeros.
- Provider/model configuration, prompt, frozen environment mapping,
  provider-event parsing, metric derivation, budget enforcement,
  oracle invocation, report shape, and record append/verification
  are the declared campaign machinery, reused unchanged. Mediation
  overhead is not hidden: every exchange is captured and counted in
  the cumulative trial ledger persisted with the run; provider
  budgets are enforced from provider events exactly as before. The
  confinement environment mapping (HOME=/scratch, host PATH/LANG
  carried through) is explicit here.
"""

from __future__ import annotations

import hashlib
import json
import os
import socket
import threading
import time
from pathlib import Path
from typing import Any, Callable, Mapping

from bench.live import tooling as _tooling
from bench.live.attempts import append_attempt, build_attempt
from bench.live.confined import (
    SandboxSpec,
    confinement_argv,
    require_bwrap,
)
from bench.live.manifest import canonical_json_bytes
from bench.live.mediated_sley import (
    GatewayError,
    MediatedSleyEndpoint,
    adjudicate,
    audit_label,
)
from bench.live.sley2_tool import TOOL_VERSION as _TOOL_VERSION
from bench.live.oracle import OracleError
from bench.live.process import ProcessCapture
from bench.live.provider import ProviderError, parse_codex_jsonl
from bench.live.snapshot import encode_snapshot, snapshot_directory
from bench.live.taskpacks import stage_initial
from bench.live.trusted_capture import (
    BUDGET_KEYS,
    CaptureError,
    TrustedCapture,
    _canonical,
    reconcile,
)

# One source of truth with the tool boundary the judge pins.
MEDIATED_TOOL_VERSION = _TOOL_VERSION
SOCK_NAME = "gateway.sock"
SHIM_ENV = "SLEY2_GATEWAY_SOCK"
FRAME_LIMIT_BYTES = 8 * 1024 * 1024

# No staged trial inputs: the confined agent discovers starting
# identities exclusively through the documented gateway surface
# (open/inventory/read/bounded raw queries/resolve, all captured and
# counted). A private
# manifest role map, 6c-prefix parameter selection, or conventional
# variant-member literals must never be staged into scratch: they
# are deterministic-test scaffolding, not campaign inputs. Agent
# authorship of new identities (e.g. typedef member ids) happens
# client-side through the same surface in tests and production.

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "bench" / "fixtures"

# Production agent-visible scratch set. The confined production agent
# sees exactly: documented tooling, the generic shim, and the generic
# frame transport. Scripted witness sequences, member literals,
# expected outcomes, and synthetic provider events are test-only and
# are never staged here (tests inject them at the provider boundary
# via the adapter hook below).
PRODUCTION_STAGING_FILES = (
    ".sley-live/TOOLING.md",
    ".sley-live/sley-tool",
    "mediated_transport.py",
)

# Test-only adapter assets that must never appear in production
# staging (regression-enforced). Filenames are enforced by the
# permitted-set check above; these are solver/witness/event code
# markers that must not occur in staged production bytes.
FORBIDDEN_STAGING_PATTERNS = (
    "seq_type",
    "seq_stale",
    "CLIENT_MEMBERS",
    "emit_provider_stream",
)


def stage_mediated_scratch(scratch: Path) -> None:
    """Stage the confined production agent view: tool contract docs,
    the frame-forwarding ``sley-tool`` shim, and the generic frame
    transport (stdlib-only, task-agnostic). No packs, oracle inputs,
    capture paths, scripted solvers, witness sequences, expected
    outcomes, or synthetic provider events are staged here."""

    root = Path(scratch)
    control = root / ".sley-live"
    control.mkdir(mode=0o700, parents=True, exist_ok=True)
    doc = _tooling._files("sley_2_0")["TOOLING.md"][0]
    (control / "TOOLING.md").write_bytes(doc)
    os.chmod(control / "TOOLING.md", 0o444)
    shim = (ROOT / "bench" / "live" / "mediated_shim.py").read_bytes()
    (control / "sley-tool").write_bytes(shim)
    os.chmod(control / "sley-tool", 0o555)
    transport = (ROOT / "bench" / "live" / "mediated_transport.py").read_bytes()
    (root / "mediated_transport.py").write_bytes(transport)
    os.chmod(root / "mediated_transport.py", 0o444)


def production_staging_digest(scratch: Path) -> dict[str, str]:
    """Digest every legitimate production-staged agent-visible input."""

    root = Path(scratch)
    digests: dict[str, str] = {}
    for rel in PRODUCTION_STAGING_FILES:
        digests[rel] = hashlib.sha256((root / rel).read_bytes()).hexdigest()
    return digests


def assert_production_staging_clean(scratch: Path) -> None:
    """Regression gate: permitted file set only, no solver assets."""

    from bench.live.campaign import CampaignError as _CampaignError

    root = Path(scratch)
    for rel in PRODUCTION_STAGING_FILES:
        if not (root / rel).is_file():
            raise _CampaignError(f"LIVE_MEDIATED_STAGING_MISSING: {rel}")
    # No extra agent-visible files at the scratch top level or in
    # .sley-live beyond the permitted set.
    allowed = set(PRODUCTION_STAGING_FILES)
    observed: list[str] = []
    for path in list((root).iterdir()) + list((root / ".sley-live").iterdir()):
        if path.is_dir():
            continue
        rel = str(path.relative_to(root))
        observed.append(rel)
        if rel not in allowed and rel != "gateway.sock":
            raise _CampaignError(
                f"LIVE_MEDIATED_STAGING_EXTRA: {rel}")
    for rel in PRODUCTION_STAGING_FILES:
        try:
            text = (root / rel).read_text(encoding="utf-8", errors="strict")
        except OSError as error:
            raise _CampaignError(
                f"LIVE_MEDIATED_STAGING_UNREADABLE: {rel}: {error}") from error
        for pattern in FORBIDDEN_STAGING_PATTERNS:
            if pattern in text:
                raise _CampaignError(
                    f"LIVE_MEDIATED_STAGING_FORBIDDEN: {pattern} in {rel}")


def stage_test_adapter(scratch: Path, source: Path | None = None) -> Path:
    """Test-only injection: copy the deterministic stand-in client into
    a scratch that already carries the real production staging. Never
    called by the production path; tests call it via the adapter hook
    so the stand-in exercises the real confinement, mediation,
    capture, oracle, append, and verification machinery."""

    src = Path(source) if source is not None else (
        ROOT / "bench" / "live" / "mediated_client.py")
    dest = Path(scratch) / "mediated_client.py"
    dest.write_bytes(src.read_bytes())
    os.chmod(dest, 0o444)
    return dest


def resolve_sley_binary() -> Path:
    """The bound sley binary: explicit environment binding only."""

    from bench.live.campaign import CampaignError as _CampaignError

    raw = os.environ.get("SLEY2_SLEY_BINARY", "")
    path = Path(raw)
    try:
        if not raw or "\x00" in raw or not path.is_file() \
                or path.is_symlink() or not os.access(path, os.X_OK):
            raise _CampaignError("LIVE_MEDIATED_BINARY_UNBOUND")
    except OSError as error:
        raise _CampaignError(
            f"LIVE_MEDIATED_BINARY_UNBOUND: {error}") from error
    return path


def frozen_bindings(protected_ws: Path, task_id: str,
                    sley_binary: Path) -> dict[str, str]:
    """Frozen start bindings for the capture (pack, task manifest,
    tool version, binary identity)."""

    from bench.live.campaign import CampaignError as _CampaignError

    try:
        pack = (Path(protected_ws) / "base.pack").read_bytes()
        task_manifest = (
            FIXTURES / "sley2" / task_id / "task_manifest.json"
        ).read_bytes()
        binary = Path(sley_binary).read_bytes()
    except OSError as error:
        raise _CampaignError(
            f"LIVE_MEDIATED_FROZEN_INVALID: {error}") from error
    return {
        "pack_sha256": hashlib.sha256(pack).hexdigest(),
        "task_manifest_sha256": hashlib.sha256(task_manifest).hexdigest(),
        "tool_version": MEDIATED_TOOL_VERSION,
        "binary_sha256": hashlib.sha256(binary).hexdigest(),
    }


class GatewayServer:
    """Runner-side unix-socket ingress: the single route from the
    confined trial to the mediated endpoint. Every well-formed frame
    is served through ``endpoint.handle`` (captured before release).
    Malformed/oversized frames and uncaptured replies permanently
    invalidate the attempt; serving stops when capture dies."""

    def __init__(self, sock_path: Path, endpoint: MediatedSleyEndpoint,
                 *, frame_limit: int = FRAME_LIMIT_BYTES) -> None:
        self._sock = Path(sock_path)
        self._endpoint = endpoint
        self._limit = frame_limit
        self._lock = threading.Lock()
        self._frames = 0
        self._invalidated: str | None = None
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._listener: socket.socket | None = None

    @property
    def frames(self) -> int:
        with self._lock:
            return self._frames

    @property
    def invalidated(self) -> str | None:
        with self._lock:
            return self._invalidated

    def _invalidate(self, code: str) -> None:
        with self._lock:
            if self._invalidated is None:
                self._invalidated = code
        self._stop.set()

    def start(self) -> None:
        try:
            if self._sock.exists():
                os.unlink(self._sock)
        except OSError as error:
            from bench.live.campaign import CampaignError as _CampaignError

            raise _CampaignError(
                f"LIVE_MEDIATED_SOCKET_INVALID: {error}") from error
        listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        listener.bind(str(self._sock))
        try:
            os.chmod(self._sock, 0o600)
        except OSError:
            pass
        listener.listen(16)
        listener.settimeout(1.0)
        self._listener = listener
        thread = threading.Thread(target=self._serve, daemon=True,
                                  name="mediated-gateway")
        self._thread = thread
        thread.start()

    def stop(self) -> None:
        self._stop.set()
        listener, self._listener = self._listener, None
        if listener is not None:
            try:
                listener.close()
            except OSError:
                pass
        thread, self._thread = self._thread, None
        if thread is not None and thread is not threading.current_thread():
            thread.join(timeout=30)

    def _read_frame(self, conn: socket.socket) -> bytes | None:
        """Read one newline-terminated frame; None on clean EOF.
        Oversized input fails closed (never truncated-and-served)."""

        chunks: list[bytes] = []
        total = 0
        while True:
            try:
                piece = conn.recv(65536)
            except OSError:
                return None
            if not piece:
                return None if total == 0 else b"".join(chunks)
            total += len(piece)
            if total > self._limit + 1:
                self._invalidate("MEDIATED_FRAME_OVERSIZED")
                return b""
            chunks.append(piece)
            if piece.endswith(b"\n") or b"\n" in piece:
                break
        raw = b"".join(chunks)
        line, _, _ = raw.partition(b"\n")
        return line

    @staticmethod
    def _frame_shape(frame: Any) -> bool:
        return (isinstance(frame, dict)
                and isinstance(frame.get("phase"), str)
                and isinstance(frame.get("session_id"), str)
                and isinstance(frame.get("command"), str)
                and isinstance(frame.get("args"), list)
                and all(isinstance(a, str) for a in frame["args"]))

    def _serve_connection(self, conn: socket.socket) -> None:
        try:
            while not self._stop.is_set():
                line = self._read_frame(conn)
                if line is None:
                    return
                if line == b"":
                    # Oversized (already invalidated) or torn input:
                    # close without serving.
                    return
                try:
                    frame = json.loads(line.decode("utf-8"))
                except (UnicodeError, ValueError):
                    self._invalidate("MEDIATED_INGRESS_INVALID")
                    return
                if not self._frame_shape(frame):
                    self._invalidate("MEDIATED_INGRESS_INVALID")
                    return
                response = self._handle_frame(frame)
                if response is None:
                    return
                try:
                    conn.sendall(response + b"\n")
                except OSError:
                    return
        finally:
            try:
                conn.close()
            except OSError:
                pass

    def _handle_frame(self, frame: dict[str, Any]) -> bytes | None:
        """Serve one well-formed frame. Returns reply bytes, or None
        when the attempt is dead and serving must stop."""

        try:
            response = self._endpoint.handle(
                frame["phase"], frame["session_id"],
                frame["command"], frame["args"])
        except CaptureError as error:
            note = _canonical({"ok": False,
                               "error": "GATEWAY_CAPTURE_FAILED",
                               "detail": str(error)[:300],
                               "uncaptured": True})
            self._invalidate(f"MEDIATED_CAPTURE_FAILED: {error}"[:160])
            return note
        except GatewayError as error:
            capture = self._endpoint.capture

            def denied() -> tuple[bytes, dict[str, Any]]:
                return (_canonical({"ok": False,
                                    "error": "GATEWAY_COMMAND_DENIED",
                                    "detail": str(error)[:300]}),
                        {"failed": True})

            try:
                response = capture.exchange(
                    phase=frame["phase"] or "ingress",
                    session_id=frame["session_id"] or "unknown",
                    method=audit_label(frame["command"], frame["args"]),
                    request=_canonical({"command": frame["command"],
                                        "args": frame["args"]}),
                    handler=denied)
            except CaptureError as capture_error:
                note = _canonical({"ok": False,
                                   "error": "GATEWAY_CAPTURE_FAILED",
                                   "detail": str(capture_error)[:300],
                                   "uncaptured": True})
                self._invalidate(
                    f"MEDIATED_CAPTURE_FAILED: {capture_error}"[:160])
                return note
        with self._lock:
            self._frames += 1
        return response

    def _serve(self) -> None:
        listener = self._listener
        assert listener is not None
        while not self._stop.is_set():
            try:
                conn, _ = listener.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            worker = threading.Thread(target=self._serve_connection,
                                      args=(conn,), daemon=True)
            worker.start()


def _usage_bytes(capture_dir: Path) -> bytes | None:
    """Runner-owned cumulative usage ledger bytes. Missing ledger
    stays missing (never a measured zero)."""

    try:
        raw = (Path(capture_dir) / "budgets.json").read_bytes()
    except OSError:
        return None
    try:
        loaded = json.loads(raw)
    except (UnicodeError, ValueError):
        return None
    if not isinstance(loaded, dict) or not isinstance(
            loaded.get("totals"), dict):
        return None
    totals = loaded["totals"]
    if set(totals) != set(BUDGET_KEYS):
        return None
    return _canonical({"totals": totals}) + b"\n"


def _transcript_bytes(capture_dir: Path) -> bytes | None:
    """Runner-owned exchange transcript bytes (captured requests and
    responses, not candidate-side diagnostics)."""

    try:
        raw = (Path(capture_dir) / "exchanges.jsonl").read_bytes()
    except OSError:
        return None
    return raw or None


def execute_mediated_attempt(
    *,
    run_directory: Path,
    manifest: Mapping[str, Any],
    store: Any,
    adapter: Any,
    task_id: str,
    arm_id: str,
    seed: int,
    workspace_parent: Path,
    provider_runner: Callable[..., ProcessCapture],
    oracle_runner: Callable[..., tuple[dict[str, Any], bytes, bytes]],
    utc_now: Callable[[], str],
    mediated_share_net: bool = False,
) -> dict[str, Any]:
    """Execute one confined mediated sley_2_0 attempt and append the
    record. See the module docstring for the path contract."""

    # Reused declared campaign machinery (no second system): prompt,
    # environment, metrics, observation, oracle report, completion
    # binding, artifact put, and attempt builders.
    from bench.live.campaign import (
        MAX_PROVIDER_OUTPUT_BYTES,
        CampaignError,
        _apply_observation,
        _artifact,
        _completion_payload,
        _empty_metrics,
        _oracle_report,
    )
    from bench.live.environment import (
        environment_snapshot_bytes,
        provider_environment,
    )
    from bench.live.tooling import build_prompt

    run = Path(run_directory)
    prompt = build_prompt(task_id, seed, arm_id)
    environment_payload = environment_snapshot_bytes(manifest)
    frozen_provider_environment = provider_environment(manifest)
    started = utc_now()
    metrics = _empty_metrics()
    status = "harness_failure"
    failure_code: str | None = "LIVE_CAMPAIGN_INTERNAL"
    capture: ProcessCapture | None = None
    events = None
    final_message: bytes | None = None
    oracle_report_payload: bytes | None = None
    oracle_stdout: bytes | None = None
    oracle_stderr: bytes | None = None
    after_payload: bytes | None = None
    before_payload: bytes | None = None
    evidence_digests: dict[str, str | None] = {
        "agent_transcript_sha256": None,
        "agent_usage_sha256": None,
        "evidence_completion_sha256": None,
        "final_candidate_sha256": None,
    }

    def _store_evidence(payloads: Mapping[str, bytes | None],
                        provider_stdout: bytes, attempt: str) -> None:
        try:
            digests: dict[str, str | None] = {}
            for slot, data in payloads.items():
                digests[slot] = _artifact(store, data)
            completion = _completion_payload(
                attempt_id=attempt, evidence=payloads, digests=digests,
                provider_events=provider_stdout)
            digests["evidence_completion_sha256"] = _artifact(
                store, completion)
        except (OSError, ValueError) as error:
            raise CampaignError(
                f"LIVE_EVIDENCE_SINK_INVALID: store: {error}") from error
        evidence_digests.update(digests)

    def _capture_evidence() -> dict[str, bytes | None]:
        return {
            "agent_transcript_sha256": _transcript_bytes(capture_dir),
            "agent_usage_sha256": _usage_bytes(capture_dir),
            "final_candidate_sha256": final_bytes,
        }

    def _retain_evidence() -> None:
        """Best-effort retention of runner-owned capture evidence on
        failed attempts (never raises; missing observations stay
        missing)."""

        try:
            _store_evidence(_capture_evidence(), b"" if capture is None
                            else capture.stdout, attempt_id)
        except CampaignError:
            pass

    # Staging errors occur before the attempt boundary: raise (the
    # slot is not consumable without frozen bindings).
    require_bwrap()
    parent = Path(workspace_parent)
    parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    attempt_id = f"{manifest['run_id']}.{arm_id}.{task_id.lower()}.{seed}"
    capture_root = run / "captures"
    capture_root.mkdir(mode=0o700, parents=True, exist_ok=True)
    capture_dir = capture_root / attempt_id
    sley_binary = resolve_sley_binary()
    final_bytes: bytes | None = None
    server: GatewayServer | None = None

    import tempfile

    with tempfile.TemporaryDirectory(prefix="attempt-", dir=parent) as temporary:
        protected_root = Path(temporary) / "protected"
        protected_ws = protected_root / "candidate"
        stage_initial(arm_id, task_id, protected_ws)
        try:
            os.chmod(protected_root, 0o700)
            os.chmod(protected_ws, 0o700)
        except OSError:
            pass
        before_payload = encode_snapshot(snapshot_directory(protected_ws))
        frozen = frozen_bindings(protected_ws, task_id, sley_binary)
        caps = {"trial_max_exchanges": int(manifest["action_budget"]),
                "trial_max_wall_ms": int(manifest["wall_time_budget"])}
        trusted = TrustedCapture.create(capture_dir, attempt_id=attempt_id,
                                        frozen=frozen, caps=caps)
        endpoint = MediatedSleyEndpoint(sley_binary, protected_ws, trusted)
        scratch = Path(temporary) / "scratch"
        scratch.mkdir(mode=0o700)
        stage_mediated_scratch(scratch)
        assert_production_staging_clean(scratch)
        # Test-only stand-ins inject at the provider boundary AFTER
        # the real production staging is verified: the deterministic
        # adapter exercises the real confinement, mediation, capture,
        # oracle, append, and verification machinery. Production
        # adapters expose no such hook, so nothing extra is staged.
        extra_stager = getattr(adapter, "extra_scratch_files", None)
        if callable(extra_stager):
            for rel, data in extra_stager() or []:
                dest = scratch / rel
                if dest.exists():
                    from bench.live.campaign import CampaignError as _CE
                    raise _CE(f"LIVE_MEDIATED_TEST_OVERWRITE: {rel}")
                dest.write_bytes(data)
                os.chmod(dest, 0o444)
        sock_path = scratch / SOCK_NAME
        server = GatewayServer(sock_path, endpoint)
        # Masked prefixes: trial state, the run's records/artifacts,
        # and the repository source tree itself (fixtures carry
        # expected answers; oracle code and capture machinery are
        # runner-side only). The agent's sole channel is the socket.
        masks = (protected_root, run,
                 ROOT / "bench", ROOT / "oracle", ROOT / "crates")
        spec = SandboxSpec(scratch_dir=scratch, mask_paths=masks,
                           share_net=bool(mediated_share_net))
        agent_argv = adapter.command(Path("/scratch"))
        confined_argv = confinement_argv(spec, agent_argv)
        # Confinement environment mapping (explicit): HOME is the
        # sandbox scratch; host PATH/LANG are carried through; the
        # gateway socket reaches the agent ONLY as $SLEY2_GATEWAY_SOCK
        # naming the in-sandbox socket path.
        lang = str(frozen_provider_environment.get("LANG", "C.UTF-8"))
        marker = confined_argv.index("--")
        confined_argv = (
            confined_argv[:marker]
            + ["--setenv", "LANG", lang,
               "--setenv", SHIM_ENV, "/scratch/" + SOCK_NAME]
            + confined_argv[marker:]
        )
        try:
            server.start()
            try:
                capture = provider_runner(
                    confined_argv,
                    prompt,
                    timeout_ms=manifest["wall_time_budget"],
                    max_output_bytes=MAX_PROVIDER_OUTPUT_BYTES,
                    environment=frozen_provider_environment,
                )
            finally:
                server.stop()
            try:
                after_snapshot = snapshot_directory(protected_ws)
                after_payload = encode_snapshot(after_snapshot)
                metrics["canonical_storage_bytes"] = after_snapshot["total_bytes"]
            except Exception:
                failure_code = "LIVE_WORKSPACE_AFTER_INVALID"

            if capture.timed_out:
                status = "timeout"
                failure_code = "LIVE_PROVIDER_TIMEOUT"
                metrics["wall_time"] = capture.wall_time_ms
                try:
                    _store_evidence(_capture_evidence(), capture.stdout,
                                    attempt_id)
                except CampaignError:
                    pass
            elif capture.exit_code != 0:
                status = "harness_failure"
                failure_code = "LIVE_PROVIDER_EXIT_NONZERO"
                metrics["wall_time"] = capture.wall_time_ms
                try:
                    _store_evidence(_capture_evidence(), capture.stdout,
                                    attempt_id)
                except CampaignError:
                    pass
            else:
                try:
                    events = parse_codex_jsonl(capture.stdout)
                    _apply_observation(metrics, events, prompt, capture)
                    if events.final_message is not None:
                        final_message = events.final_message.encode("utf-8")
                except ProviderError:
                    status = "harness_failure"
                    failure_code = "LIVE_PROVIDER_EVENT_INVALID"
                else:
                    if (
                        metrics["model_input_tokens"] > manifest["context_budget"]
                        or metrics["tool_calls"] > manifest["action_budget"]
                    ):
                        status = "harness_failure"
                        failure_code = "LIVE_PROVIDER_BUDGET_EXCEEDED"
                    elif after_payload is not None:
                        if server.invalidated is not None:
                            # Fail-closed ingress: malformed/oversized
                            # input or an uncaptured reply permanently
                            # invalidates the attempt BEFORE any oracle
                            # verdict can be sought.
                            status = "harness_failure"
                            failure_code = server.invalidated
                            try:
                                _store_evidence(_capture_evidence(),
                                                capture.stdout, attempt_id)
                            except CampaignError:
                                pass
                        else:
                            final_bytes = endpoint.protected_final()
                            if not isinstance(final_bytes, bytes):
                                status = "harness_failure"
                                failure_code = "CAPTURE_GATE_NO_FINAL"
                                _retain_evidence()
                            else:
                                try:
                                    trusted.complete(final_bytes)
                                except CaptureError as error:
                                    status = "harness_failure"
                                    failure_code = (
                                        f"CAPTURE_GATE_STORAGE: {error}"[:160])
                                    _retain_evidence()
                                else:
                                    reconciled = reconcile(
                                        capture_dir, final_bytes)
                                    if not reconciled["reconciled"]:
                                        status = "harness_failure"
                                        failure_code = reconciled["code"]
                                        _retain_evidence()
                                    else:
                                        try:
                                            _store_evidence(
                                                _capture_evidence(),
                                                capture.stdout, attempt_id)
                                        except CampaignError as error:
                                            status = "harness_failure"
                                            failure_code = str(error)
                                        else:
                                            oracle_started = time.monotonic_ns()
                                            previous_capture = os.environ.get(
                                                "SLEY2_MEDIATED_CAPTURE_DIR")
                                            os.environ[
                                                "SLEY2_MEDIATED_CAPTURE_DIR"] = str(
                                                    capture_dir)
                                            try:
                                                try:
                                                    (verdict, oracle_stdout,
                                                     oracle_stderr) = oracle_runner(
                                                        arm_id=arm_id,
                                                        task_id=task_id,
                                                        candidate=protected_ws,
                                                    )
                                                finally:
                                                    if previous_capture is None:
                                                        del os.environ[
                                                            "SLEY2_MEDIATED_CAPTURE_DIR"]
                                                    else:
                                                        os.environ[
                                                            "SLEY2_MEDIATED_CAPTURE_DIR"] = (
                                                                previous_capture)
                                            except OracleError:
                                                status = "harness_failure"
                                                failure_code = (
                                                    "LIVE_ORACLE_INVALID")
                                                _retain_evidence()
                                            else:
                                                metrics["execution_latency"] = max(
                                                    0, (time.monotonic_ns()
                                                        - oracle_started)
                                                    // 1_000_000
                                                )
                                                report = _oracle_report(
                                                    verdict,
                                                    task_id=task_id,
                                                    arm_id=arm_id,
                                                    snapshots_complete=True,
                                                )
                                                oracle_report_payload = (
                                                    canonical_json_bytes(report)
                                                    + b"\n"
                                                )
                                                (status, failure_code) = adjudicate(
                                                    capture_dir, final_bytes,
                                                    verdict)
                                                if status in {"accepted",
                                                              "rejected"}:
                                                    metrics[
                                                        "accepted_correct_changes"
                                                    ] = (1 if status
                                                         == "accepted" else 0)
                                                    metrics[
                                                        "strict_accepted_correctness"
                                                    ] = status == "accepted"
                                                    metrics["invalid_candidates"] += (
                                                        1 if status
                                                        == "rejected" else 0
                                                    )
                                                    for field in (
                                                        "collateral_semantic_changes",
                                                        "invalid_committed_states",
                                                        "stale_candidates",
                                                        "stale_candidates_incorrectly_accepted",
                                                    ):
                                                        metrics[field] = report[
                                                            field
                                                        ]
        except Exception as error:
            # After provider launch, retain the attempt rather than
            # losing the denominator slot.
            status = "harness_failure"
            failure_code = f"LIVE_PROVIDER_PROCESS_FAILURE_{type(error).__name__.upper()}"

        ended = utc_now()
        artifacts = {
            "agent_transcript_sha256": evidence_digests["agent_transcript_sha256"],
            "agent_usage_sha256": evidence_digests["agent_usage_sha256"],
            "environment_snapshot_sha256": _artifact(store, environment_payload),
            "evidence_completion_sha256": evidence_digests["evidence_completion_sha256"],
            "final_candidate_sha256": evidence_digests["final_candidate_sha256"],
            "final_message_sha256": _artifact(store, final_message),
            "oracle_report_sha256": _artifact(store, oracle_report_payload),
            "oracle_stderr_sha256": _artifact(store, oracle_stderr),
            "oracle_stdout_sha256": _artifact(store, oracle_stdout),
            "prompt_sha256": _artifact(store, prompt),
            "provider_events_sha256": _artifact(store, b"" if capture is None else capture.stdout),
            "provider_stderr_sha256": _artifact(store, b"" if capture is None else capture.stderr),
            "workspace_after_sha256": _artifact(store, after_payload),
            "workspace_before_sha256": _artifact(store, before_payload),
        }
        record = build_attempt(
            manifest=manifest,
            attempt_id=attempt_id,
            task_id=task_id,
            arm_id=arm_id,
            seed=seed,
            started_at_utc=started,
            ended_at_utc=ended,
            status=status,
            failure_code=failure_code,
            provider_exit_code=None if capture is None else capture.exit_code,
            artifacts=artifacts,
            metrics=metrics,
        )
        append_attempt(run, record)
        return record
