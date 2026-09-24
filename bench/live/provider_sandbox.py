"""Outer confinement for the real live provider, identical for every arm.

The pilot of 2026-09-24 found that the live provider could not run the
campaign as the earlier harness launched it:

- raw_files and sley_1_2_0 ran ``codex exec`` directly on the host, so the
  agent's commands could read the whole home directory, including the
  frozen fixtures' positive programs and oracles under ``bench/fixtures``
  and the operator's provider history, while the sley_2_0 agent was
  confined with those prefixes masked (an arm-isolation asymmetry);
- the provider inherited the operator's ``CODEX_HOME`` (global
  AGENTS.md, about 200 skills, memories, and session history) into every
  arm's context;
- the confined sley_2_0 launch bound neither the provider executable, its
  home, nor any network route, so a real provider could not start; and
  Codex's own workspace-write sandbox refuses AF_UNIX connects unless its
  network access is enabled, so the mediated tool could not reach its
  gateway from a model-issued command.

This module is the one launch shape for all three arms. Each attempt gets
a fresh sandbox home, a fresh provider home (only the credential file is
bound in from the host), and a network namespace with no route out: the
provider reaches its API through a loopback forwarder to a runner-side
CONNECT proxy that admits only the frozen provider hosts on port 443.
Model-issued commands inherit no proxy variables, so they have no
network at all; the unix-socket gateway is a filesystem object and stays
reachable for the sley_2_0 tool.
"""

from __future__ import annotations

import json
import os
import socket
import threading
from dataclasses import dataclass, field
from pathlib import Path

from bench.live.confined import SandboxSpec


ROOT = Path(__file__).resolve().parents[2]
FORWARDER_SOURCE = ROOT / "bench" / "live" / "egress_forwarder.py"
PROVIDER_HOSTS = ("auth.openai.com", "chatgpt.com")
PROVIDER_PORT = 443
LOOPBACK_PORT = 18080
SANDBOX_HOME = "/scratch"
SANDBOX_PATH = "/usr/bin:/bin"
SANDBOX_PROVIDER_HOME = "/codex-home"
SANDBOX_WORKSPACE = "/workspace"
SANDBOX_FORWARDER = "/opt/sley-live/egress_forwarder.py"
SANDBOX_EGRESS_DIR = "/opt/sley-live-egress"
EGRESS_SOCKET_NAME = "egress.sock"
PROXY_URL = f"http://127.0.0.1:{LOOPBACK_PORT}"
# Host-side name resolution inside the namespace is never needed (the
# proxy resolves), but the stub resolver path is bound so a provider
# that resolves before proxying fails the same way in every arm.
RESOLVER_DIR = "/run/systemd/resolve"
# Sandbox-layout environment every provider launch must carry exactly.
FIXED_ENVIRONMENT = {
    "CODEX_HOME": SANDBOX_PROVIDER_HOME,
    "HOME": SANDBOX_HOME,
    "HTTPS_PROXY": PROXY_URL,
    "HTTP_PROXY": PROXY_URL,
    "PATH": SANDBOX_PATH,
}


class ProviderSandboxError(ValueError):
    """The provider sandbox is misconfigured or disagrees with its run."""


def _fail(symbol: str, detail: str = "") -> None:
    raise ProviderSandboxError(symbol if not detail else f"{symbol}: {detail}")


@dataclass(frozen=True)
class ProviderSandbox:
    """Frozen host inputs of the provider launch.

    ``provider_root`` is the provider's installed release directory, bound
    read-only at its own path (the executable and its sibling helpers);
    ``auth_source`` is the host credential file bound over the fresh
    per-attempt provider home's placeholder.
    """

    provider_root: Path
    auth_source: Path
    allowed_hosts: tuple[str, ...] = PROVIDER_HOSTS

    def __post_init__(self) -> None:
        root = Path(self.provider_root)
        auth = Path(self.auth_source)
        try:
            if not root.is_absolute() or not root.is_dir() or root.is_symlink():
                _fail("LIVE_PROVIDER_SANDBOX_INVALID", "provider_root")
            if not auth.is_absolute() or not auth.is_file():
                _fail("LIVE_PROVIDER_SANDBOX_INVALID", "auth_source")
        except OSError as error:
            raise ProviderSandboxError(
                f"LIVE_PROVIDER_SANDBOX_INVALID: {error}") from error
        if (not self.allowed_hosts
                or any(not isinstance(host, str) or not host or ":" in host
                       for host in self.allowed_hosts)):
            _fail("LIVE_PROVIDER_SANDBOX_INVALID", "allowed_hosts")

    def describe(self) -> dict[str, object]:
        """Run-manifest description (paths and policy, never secrets)."""

        return {
            "allowed_hosts": sorted(self.allowed_hosts),
            "allowed_port": PROVIDER_PORT,
            "auth_source": str(self.auth_source),
            "contract": "sley2.live-provider-sandbox.v1",
            "fixed_environment": dict(sorted(FIXED_ENVIRONMENT.items())),
            "provider_root": str(self.provider_root),
            "sandbox_workspace": SANDBOX_WORKSPACE,
        }


@dataclass(frozen=True)
class SandboxLayout:
    """Fresh per-attempt host directories behind the sandbox paths."""

    home: Path
    provider_home: Path
    egress_dir: Path

    @property
    def egress_socket(self) -> Path:
        return self.egress_dir / EGRESS_SOCKET_NAME


def prepare_layout(attempt_root: Path) -> SandboxLayout:
    """Create the cold per-attempt home, provider home, and egress dir."""

    root = Path(attempt_root)
    layout = SandboxLayout(home=root / "sandbox-home",
                           provider_home=root / "provider-home",
                           egress_dir=root / "egress")
    for directory in (layout.home, layout.provider_home, layout.egress_dir):
        directory.mkdir(mode=0o700)
    placeholder = layout.provider_home / "auth.json"
    descriptor = os.open(placeholder, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.close(descriptor)
    return layout


def check_provider_environment(environment: dict[str, str]) -> None:
    """The manifest's provider environment must name the sandbox layout."""

    for name, expected in FIXED_ENVIRONMENT.items():
        if environment.get(name) != expected:
            _fail("LIVE_PROVIDER_SANDBOX_MISMATCH", name)


def provider_binds(sandbox: ProviderSandbox,
                   layout: SandboxLayout) -> tuple[tuple[str, str, str], ...]:
    root = str(Path(sandbox.provider_root))
    return (
        ("ro", root, root),
        ("rw", str(layout.provider_home), SANDBOX_PROVIDER_HOME),
        ("rw", str(Path(sandbox.auth_source)),
         f"{SANDBOX_PROVIDER_HOME}/auth.json"),
        ("rw", str(layout.egress_dir), SANDBOX_EGRESS_DIR),
        ("ro", str(FORWARDER_SOURCE), SANDBOX_FORWARDER),
    )


def provider_setenv(environment: dict[str, str]) -> tuple[tuple[str, str], ...]:
    check_provider_environment(environment)
    return tuple(sorted(environment.items()))


def wrap_agent(agent_argv: list[str]) -> list[str]:
    return ["/usr/bin/python3", SANDBOX_FORWARDER,
            f"{SANDBOX_EGRESS_DIR}/{EGRESS_SOCKET_NAME}",
            str(LOOPBACK_PORT), *agent_argv]


def legacy_tool_binds() -> tuple[tuple[str, str, str], ...]:
    """Read-only closure the frozen 1.2.0 tool needs inside the sandbox:
    the thin CLI and task runner modules (never fixtures or oracles), the
    pinned artifact, and the pinned node toolchain it shells out to."""

    from bench.legacy.runner import DEFAULT_ARTIFACT_PATH
    from bench.legacy.task_runner import NODE_DIR

    files = [
        ROOT / "bench" / "live" / "__init__.py",
        ROOT / "bench" / "live" / "legacy_tool.py",
        ROOT / "bench" / "legacy" / "__init__.py",
        ROOT / "bench" / "legacy" / "runner.py",
        ROOT / "bench" / "legacy" / "task_runner.py",
        Path(DEFAULT_ARTIFACT_PATH),
    ]
    binds = [("ro", str(path), str(path)) for path in files]
    if Path(NODE_DIR).is_dir():
        binds.append(("ro", NODE_DIR, NODE_DIR))
    return tuple(binds)


def arm_spec(*, arm_id: str, sandbox: ProviderSandbox, layout: SandboxLayout,
             environment: dict[str, str], workspace: Path | None = None,
             scratch_dir: Path | None = None,
             mask_paths: tuple[Path, ...] = (),
             extra_setenv: tuple[tuple[str, str], ...] = ()) -> SandboxSpec:
    """The provider sandbox spec for one arm.

    raw_files / sley_1_2_0: HOME is the fresh sandbox home at /scratch and
    the candidate workspace is bound read-write at /workspace. sley_2_0:
    /scratch is the mediated agent scratch (tool doc, shim, transport,
    gateway socket) and the protected state is masked by the caller.
    """

    binds = list(provider_binds(sandbox, layout))
    if arm_id in {"raw_files", "sley_1_2_0"}:
        if workspace is None:
            _fail("LIVE_PROVIDER_SANDBOX_INVALID", "workspace")
        binds.append(("rw", str(Path(workspace)), SANDBOX_WORKSPACE))
        if arm_id == "sley_1_2_0":
            binds.extend(legacy_tool_binds())
        scratch = layout.home
    elif arm_id == "sley_2_0":
        if scratch_dir is None:
            _fail("LIVE_PROVIDER_SANDBOX_INVALID", "scratch")
        scratch = Path(scratch_dir)
    else:
        _fail("LIVE_PROVIDER_SANDBOX_INVALID", "arm")
    return SandboxSpec(
        scratch_dir=scratch,
        mask_paths=tuple(mask_paths),
        share_net=False,
        extra_binds=tuple(binds),
        setenv=provider_setenv(environment) + tuple(extra_setenv),
    )


@dataclass
class EgressProxy:
    """Runner-side CONNECT proxy on a unix socket; allowlist decides.

    Every decision is appended to ``log_path`` (runner-owned, outside the
    sandbox) as one JSON line. Only ``CONNECT host:443`` to an allowed
    host is relayed; anything else is answered 403/405 and closed.
    """

    socket_path: Path
    log_path: Path
    allowed_hosts: tuple[str, ...]
    connect_timeout_s: float = 30.0
    decisions: list[dict[str, object]] = field(default_factory=list)

    def __post_init__(self) -> None:
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._listener: socket.socket | None = None
        self._thread: threading.Thread | None = None

    def _record(self, decision: str, target: str) -> None:
        entry = {"decision": decision, "target": target[:300]}
        with self._lock:
            self.decisions.append(entry)
            with open(self.log_path, "a", encoding="utf-8") as handle:
                handle.write(json.dumps(entry, sort_keys=True) + "\n")

    def start(self) -> None:
        Path(self.log_path).parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        listener.bind(str(self.socket_path))
        os.chmod(self.socket_path, 0o600)
        listener.listen(64)
        listener.settimeout(1.0)
        self._listener = listener
        self._thread = threading.Thread(target=self._serve, daemon=True,
                                        name="provider-egress")
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        listener, self._listener = self._listener, None
        if listener is not None:
            try:
                listener.close()
            except OSError:
                pass
        thread, self._thread = self._thread, None
        if thread is not None:
            thread.join(timeout=10)

    def __enter__(self) -> "EgressProxy":
        self.start()
        return self

    def __exit__(self, *_exc: object) -> None:
        self.stop()

    def _serve(self) -> None:
        while not self._stop.is_set():
            listener = self._listener
            if listener is None:
                return
            try:
                client, _ = listener.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            threading.Thread(target=self._handle, args=(client,),
                             daemon=True).start()

    def _handle(self, client: socket.socket) -> None:
        client.settimeout(30)
        head = b""
        try:
            while b"\r\n\r\n" not in head:
                piece = client.recv(4096)
                if not piece or len(head) > 16384:
                    client.close()
                    return
                head += piece
        except OSError:
            client.close()
            return
        line = head.split(b"\r\n", 1)[0].decode("latin-1")
        parts = line.split()
        if len(parts) < 2 or parts[0] != "CONNECT":
            self._record("deny_method", line)
            self._reply_close(client, b"HTTP/1.1 405 Method Not Allowed\r\n\r\n")
            return
        host, _, port = parts[1].rpartition(":")
        if host not in self.allowed_hosts or port != str(PROVIDER_PORT):
            self._record("deny", parts[1])
            self._reply_close(client, b"HTTP/1.1 403 Forbidden\r\n\r\n")
            return
        try:
            upstream = socket.create_connection((host, PROVIDER_PORT),
                                                timeout=self.connect_timeout_s)
        except OSError as error:
            self._record("upstream_error", f"{parts[1]} {error}")
            self._reply_close(client, b"HTTP/1.1 502 Bad Gateway\r\n\r\n")
            return
        self._record("allow", parts[1])
        try:
            client.sendall(b"HTTP/1.1 200 Connection established\r\n\r\n")
            rest = head.split(b"\r\n\r\n", 1)[1]
            if rest:
                upstream.sendall(rest)
        except OSError:
            client.close()
            upstream.close()
            return
        client.settimeout(None)
        upstream.settimeout(None)
        from bench.live.egress_forwarder import _pump

        _pump(client, upstream)

    @staticmethod
    def _reply_close(client: socket.socket, payload: bytes) -> None:
        try:
            client.sendall(payload)
        except OSError:
            pass
        try:
            client.close()
        except OSError:
            pass
