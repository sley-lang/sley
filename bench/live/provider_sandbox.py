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

This module is the one launch shape for all three arms and every provider
profile (``PROFILES``: Codex, Claude Code). Each attempt gets a fresh
sandbox home and a fresh provider home holding only a private copy of the
credential stripped of every refresh token (the provider can use but never
rotate the operator's login; an attempt whose copied access token would
expire inside it is refused before launch), and a network namespace with
no route out: the provider reaches its API through a loopback forwarder to
a runner-side CONNECT proxy that admits only the profile's provider hosts
on port 443. The forwarder admits only connections held by the provider
process itself, so a model-issued command (which inherits the proxy
variables) is refused and the refusal is logged by the proxy; the
unix-socket gateway is a filesystem object and stays reachable for the
sley_2_0 tool.

Residual exposure (stated, not denied): the access-token copy is readable
by model-issued commands inside the sandbox (same user, same namespace).
It is short-lived, carries no refresh token, never leaves the sandbox over
the network (commands have no egress), and is deleted with the attempt; a
command could still print it into the retained provider transcript.
"""

from __future__ import annotations

import json
import os
import socket
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

from bench.live.confined import SandboxSpec


ROOT = Path(__file__).resolve().parents[2]
FORWARDER_SOURCE = ROOT / "bench" / "live" / "egress_forwarder.py"
PROVIDER_PORT = 443
LOOPBACK_PORT = 18080
SANDBOX_HOME = "/scratch"
SANDBOX_PATH = "/usr/bin:/bin"
SANDBOX_WORKSPACE = "/workspace"
SANDBOX_FORWARDER = "/opt/sley-live/egress_forwarder.py"
SANDBOX_EGRESS_DIR = "/opt/sley-live-egress"
EGRESS_SOCKET_NAME = "egress.sock"
PROXY_URL = f"http://127.0.0.1:{LOOPBACK_PORT}"
# A credential copy must outlive the attempt by this margin beyond the
# wall budget, so the provider never needs to refresh inside the sandbox
# (the copy carries no refresh token, so it could not).
CREDENTIAL_MARGIN_MS = 30 * 60 * 1000


class ProviderSandboxError(ValueError):
    """The provider sandbox is misconfigured or disagrees with its run."""


def _fail(symbol: str, detail: str = "") -> None:
    raise ProviderSandboxError(symbol if not detail else f"{symbol}: {detail}")


def _jwt_exp_ms(token: str) -> int | None:
    import base64

    try:
        payload = token.split(".")[1]
        payload += "=" * (-len(payload) % 4)
        return int(json.loads(base64.urlsafe_b64decode(payload))["exp"]) * 1000
    except (IndexError, ValueError, KeyError, TypeError):
        return None


def _codex_credential(data: dict) -> tuple[dict, int | None]:
    tokens = data.get("tokens")
    if not isinstance(tokens, dict) or not isinstance(tokens.get("access_token"), str):
        _fail("LIVE_PROVIDER_CREDENTIAL_INVALID", "codex tokens")
    kept = {key: tokens[key] for key in ("access_token", "account_id", "id_token") if key in tokens}
    copy = {"OPENAI_API_KEY": None, "tokens": kept}
    if "last_refresh" in data:
        copy["last_refresh"] = data["last_refresh"]
    return copy, _jwt_exp_ms(tokens["access_token"])


def _claude_credential(data: dict) -> tuple[dict, int | None]:
    oauth = data.get("claudeAiOauth")
    if not isinstance(oauth, dict) or not isinstance(oauth.get("accessToken"), str):
        _fail("LIVE_PROVIDER_CREDENTIAL_INVALID", "claudeAiOauth")
    kept = {key: oauth[key] for key in ("accessToken", "expiresAt", "scopes",
                                        "subscriptionType", "rateLimitTier") if key in oauth}
    expires = oauth.get("expiresAt")
    return {"claudeAiOauth": kept}, expires if isinstance(expires, int) else None


@dataclass(frozen=True)
class ProviderProfile:
    """Everything provider-specific about the outer sandbox."""

    name: str
    model_provider: str
    home_env: str                 # the variable naming the provider home
    sandbox_home: str             # its path inside the sandbox
    credential_name: str          # file name inside the provider home
    allowed_hosts: tuple[str, ...]
    fixed_extra: tuple[tuple[str, str], ...]
    strip: Callable[[dict], tuple[dict, int | None]]

    def fixed_environment(self) -> dict[str, str]:
        fixed = {
            self.home_env: self.sandbox_home,
            "HOME": SANDBOX_HOME,
            "HTTPS_PROXY": PROXY_URL,
            "HTTP_PROXY": PROXY_URL,
            "PATH": SANDBOX_PATH,
        }
        fixed.update(dict(self.fixed_extra))
        return fixed


PROFILES = {
    "codex": ProviderProfile(
        name="codex", model_provider="openai-chatgpt-oauth", home_env="CODEX_HOME",
        sandbox_home="/codex-home", credential_name="auth.json",
        allowed_hosts=("auth.openai.com", "chatgpt.com"), fixed_extra=(),
        strip=_codex_credential),
    "claude-code": ProviderProfile(
        name="claude-code", model_provider="anthropic-claude-code-oauth",
        home_env="CLAUDE_CONFIG_DIR", sandbox_home="/claude-config",
        credential_name=".credentials.json", allowed_hosts=("api.anthropic.com",),
        # Background tasks off: the 2026-09-24 pilot showed Claude Code
        # auto-backgrounding slow tool commands (legacy tool calls take tens
        # of seconds), which forked extra queries after the result and ended
        # one attempt with exit 1; Codex has no such mode. The Bash timeout
        # is raised so a slow documented tool call is not cut at 2 minutes.
        fixed_extra=(("BASH_DEFAULT_TIMEOUT_MS", "300000"),
                     ("BASH_MAX_TIMEOUT_MS", "600000"),
                     ("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1"),
                     ("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1"),
                     ("CLAUDE_CODE_DISABLE_CLAUDE_MDS", "1"),
                     ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
                     ("DISABLE_AUTOUPDATER", "1"),
                     ("DISABLE_ERROR_REPORTING", "1"),
                     ("DISABLE_TELEMETRY", "1")),
        strip=_claude_credential),
}
PROVIDER_HOSTS = PROFILES["codex"].allowed_hosts
FIXED_ENVIRONMENT = PROFILES["codex"].fixed_environment()


@dataclass(frozen=True)
class ProviderSandbox:
    """Frozen host inputs of the provider launch.

    ``provider_root`` is the provider's installed release directory, bound
    read-only at its own path (the executable and its sibling helpers);
    ``auth_source`` is the host credential file. It is never bound: each
    attempt gets a private copy stripped of every refresh token (the
    provider can use, but never rotate, the operator's login) and the
    attempt is refused before launch unless the copied access token
    outlives the wall budget by ``CREDENTIAL_MARGIN_MS``.
    """

    provider_root: Path
    auth_source: Path
    allowed_hosts: tuple[str, ...] = PROVIDER_HOSTS
    profile: str = "codex"

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
        if self.profile not in PROFILES:
            _fail("LIVE_PROVIDER_SANDBOX_INVALID", "profile")
        if (not self.allowed_hosts
                or any(not isinstance(host, str) or not host or ":" in host
                       for host in self.allowed_hosts)):
            _fail("LIVE_PROVIDER_SANDBOX_INVALID", "allowed_hosts")

    @property
    def spec(self) -> ProviderProfile:
        return PROFILES[self.profile]

    def describe(self) -> dict[str, object]:
        """Run-manifest description (paths and policy, never secrets)."""

        return {
            "allowed_hosts": sorted(self.allowed_hosts),
            "allowed_port": PROVIDER_PORT,
            "auth_source": str(self.auth_source),
            "contract": "sley2.live-provider-sandbox.v1",
            "credential": "per-attempt copy without refresh tokens",
            "egress": "provider process only (model-issued commands refused)",
            "fixed_environment": dict(sorted(self.spec.fixed_environment().items())),
            "profile": self.profile,
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


def _write_private(path: Path, payload: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        os.write(descriptor, payload)
    finally:
        os.close(descriptor)


def prepare_layout(attempt_root: Path, sandbox: "ProviderSandbox | None" = None, *,
                   wall_time_budget_ms: int = 0, now_ms: int | None = None) -> SandboxLayout:
    """Create the cold per-attempt home, provider home, and egress dir.

    With ``sandbox`` the provider home receives the stripped credential
    copy; a copy that would expire within the attempt raises
    ``LIVE_PROVIDER_CREDENTIAL_EXPIRING`` before anything is launched.
    """

    root = Path(attempt_root)
    layout = SandboxLayout(home=root / "sandbox-home",
                           provider_home=root / "provider-home",
                           egress_dir=root / "egress")
    for directory in (layout.home, layout.provider_home, layout.egress_dir):
        directory.mkdir(mode=0o700)
    if sandbox is None:
        _write_private(layout.provider_home / "auth.json", b"")
        return layout
    try:
        data = json.loads(Path(sandbox.auth_source).read_bytes())
    except (OSError, ValueError) as error:
        raise ProviderSandboxError(f"LIVE_PROVIDER_CREDENTIAL_INVALID: {error}") from error
    if not isinstance(data, dict):
        _fail("LIVE_PROVIDER_CREDENTIAL_INVALID", "shape")
    copy, expires_ms = sandbox.spec.strip(data)
    current = int(time.time() * 1000) if now_ms is None else now_ms
    if expires_ms is None or expires_ms - current < wall_time_budget_ms + CREDENTIAL_MARGIN_MS:
        _fail("LIVE_PROVIDER_CREDENTIAL_EXPIRING",
              "access token does not outlive the attempt; refresh the host login and resume")
    _write_private(layout.provider_home / sandbox.spec.credential_name,
                   json.dumps(copy, sort_keys=True).encode("utf-8"))
    return layout


def check_provider_environment(environment: dict[str, str],
                               profile: str = "codex") -> None:
    """The manifest's provider environment must name the sandbox layout."""

    for name, expected in PROFILES[profile].fixed_environment().items():
        if environment.get(name) != expected:
            _fail("LIVE_PROVIDER_SANDBOX_MISMATCH", name)


def provider_binds(sandbox: ProviderSandbox,
                   layout: SandboxLayout) -> tuple[tuple[str, str, str], ...]:
    root = str(Path(sandbox.provider_root))
    return (
        ("ro", root, root),
        ("rw", str(layout.provider_home), sandbox.spec.sandbox_home),
        ("rw", str(layout.egress_dir), SANDBOX_EGRESS_DIR),
        ("ro", str(FORWARDER_SOURCE), SANDBOX_FORWARDER),
    )


def provider_setenv(environment: dict[str, str],
                    profile: str = "codex") -> tuple[tuple[str, str], ...]:
    check_provider_environment(environment, profile)
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
        setenv=provider_setenv(environment, sandbox.profile) + tuple(extra_setenv),
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
        if parts[:1] == ["LOCAL-DENY"]:
            # The in-sandbox relay refused a connection held by a process
            # other than the provider (a model-issued command).
            self._record("deny_local_process", line)
            self._reply_close(client, b"HTTP/1.1 204 No Content\r\n\r\n")
            return
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
