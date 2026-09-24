#!/usr/bin/env python3
"""Mount-namespace confinement for launched trial processes (bwrap).

A different directory alone is not an access-control boundary. Trial
agent processes run under bubblewrap on this machine: a new user and
mount namespace where every runner-owned protected prefix (repository
state, fixture packs, oracle inputs, authoritative capture) is masked
by a private tmpfs. Inside the sandbox the protected absolute paths
resolve to an empty, sandbox-private tmpfs: reads fail (ENOENT) and
writes land in the private namespace and vanish on exit. The host
files are neither readable nor writable from the sandbox.

The agent's only route to trial state is the mediated gateway over
stdin/stdout pipes: request frames out, response frames in. The
sandbox keeps the host network posture configurable (deterministic
adapters run with no network; the real provider path shares it).

Verification inspects actual access outcomes (open/read/write errno
from a probe inside the sandbox, host digests before/after), never
only argument lists, environment variables, or process-group cleanup.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

BWRAP = "/usr/bin/bwrap"

# Host paths every confined trial needs to execute Python. Nothing else
# from the host exists inside the sandbox unless a spec binds it
# explicitly (extra_ro_binds / extra_binds); protected prefixes are
# additionally overmounted with private tmpfs entries below, so a later
# bind can never expose them. Visibility is not authority either way.
RO_BINDS = ("/usr", "/bin", "/lib", "/lib64", "/etc")


class ConfinementError(ValueError):
    """Confinement setup, launch, or verification failed."""


def _fail(symbol: str, detail: str = "") -> None:
    raise ConfinementError(symbol if not detail else f"{symbol}: {detail}")


def bwrap_available() -> bool:
    path = Path(BWRAP)
    try:
        return (path.is_file() and not path.is_symlink()
                and os.access(path, os.X_OK))
    except OSError:
        return False


def require_bwrap() -> None:
    if not bwrap_available():
        _fail("CONFINEMENT_UNAVAILABLE", "bwrap missing or not executable")


@dataclass(frozen=True)
class SandboxSpec:
    """Exact sandbox configuration for one launched trial process."""

    scratch_dir: Path
    mask_paths: tuple[Path, ...]
    share_net: bool = False
    extra_ro_binds: tuple[Path, ...] = ()
    # Ordered (mode, host source, sandbox destination) binds applied
    # after the scratch bind and before the masks; mode is "ro" or "rw".
    # A later bind may overmount a file inside an earlier directory bind
    # (the provider credential over its per-attempt home placeholder).
    extra_binds: tuple[tuple[str, str, str], ...] = ()
    # Ordered (name, value) environment entries set after the fixed
    # PATH/HOME defaults (a later entry wins).
    setenv: tuple[tuple[str, str], ...] = ()


def confinement_argv(spec: SandboxSpec, agent_argv: list[str],
                     workdir: str = "/scratch") -> list[str]:
    """Build the bwrap launch vector. Protected prefixes are masked with
    private tmpfs mounts; the agent scratch is the only writable bind."""

    require_bwrap()
    if not agent_argv or any(not isinstance(a, str) or not a or "\x00" in a
                             for a in agent_argv):
        _fail("CONFINEMENT_INVALID", "agent argv")
    scratch = Path(spec.scratch_dir)
    try:
        if not scratch.is_dir() or scratch.is_symlink():
            _fail("CONFINEMENT_INVALID", "scratch dir")
    except OSError as error:
        raise ConfinementError(
            f"CONFINEMENT_INVALID: scratch: {error}") from error
    argv = [BWRAP, "--unshare-user", "--unshare-all"]
    if spec.share_net:
        argv.append("--share-net")
    argv += ["--die-with-parent", "--clearenv",
             "--setenv", "PATH", "/usr/bin:/bin",
             "--setenv", "HOME", "/scratch",
             "--setenv", "PYTHONDONTWRITEBYTECODE", "1"]
    for name, value in spec.setenv:
        if (not isinstance(name, str) or not name or "=" in name
                or "\x00" in name or not isinstance(value, str)
                or "\x00" in value):
            _fail("CONFINEMENT_INVALID", "setenv")
        argv += ["--setenv", name, value]
    for host in RO_BINDS:
        argv += ["--ro-bind-try", host, host]
    for host in spec.extra_ro_binds:
        argv += ["--ro-bind", str(host), str(host)]
    argv += ["--bind", str(scratch), "/scratch"]
    for bind in spec.extra_binds:
        if (not isinstance(bind, tuple) or len(bind) != 3
                or bind[0] not in {"ro", "rw"}
                or any(not isinstance(part, str) or not part
                       or "\x00" in part for part in bind[1:])
                or not bind[2].startswith("/")):
            _fail("CONFINEMENT_INVALID", "extra bind")
        mode, source, destination = bind
        argv += ["--ro-bind" if mode == "ro" else "--bind",
                 source, destination]
    # Mask every protected prefix AFTER the ro-binds so the private
    # tmpfs wins over any host visibility.
    for protected in spec.mask_paths:
        argv += ["--tmpfs", str(protected)]
    argv += ["--tmpfs", "/tmp", "--proc", "/proc", "--dev", "/dev",
             "--chdir", workdir, "--"]
    argv += list(agent_argv)
    return argv


def run_confined(spec: SandboxSpec, agent_argv: list[str], payload: bytes,
                 *, timeout_s: float,
                 max_output_bytes: int = 64 * 1024 * 1024) -> subprocess.CompletedProcess[bytes]:
    """Run one confined agent process with piped stdio (gateway frames)."""

    argv = confinement_argv(spec, agent_argv)
    if timeout_s <= 0 or timeout_s > 86400:
        _fail("CONFINEMENT_INVALID", "timeout")
    try:
        completed = subprocess.run(
            argv, input=payload, capture_output=True, timeout=timeout_s,
            check=False)
    except subprocess.TimeoutExpired as error:
        raise ConfinementError(
            f"CONFINEMENT_TIMEOUT: {error}") from error
    except OSError as error:
        raise ConfinementError(f"CONFINEMENT_SPAWN_FAILED: {error}") from error
    if (len(completed.stdout) + len(completed.stderr)) > max_output_bytes:
        _fail("CONFINEMENT_OUTPUT_LIMIT",
              str(len(completed.stdout) + len(completed.stderr)))
    return completed


PROBE_SCRIPT = r"""
import json, os, sys
out = {"reads": {}, "writes": {}, "lists": {}, "scratch_ok": False}
for path in sys.argv[1:]:
    try:
        with open(path, "rb") as handle:
            data = handle.read(64)
        out["reads"][path] = {"ok": True, "prefix": data.hex()}
    except Exception as error:
        out["reads"][path] = {"ok": False,
                              "error": type(error).__name__,
                              "errno": getattr(error, "errno", None),
                              "detail": str(error)[:160]}
    try:
        with open(os.path.join(path, ".sley-probe-write"), "wb") as handle:
            handle.write(b"probe")
        out["writes"][path] = {"ok": True}
    except Exception as error:
        out["writes"][path] = {"ok": False,
                               "error": type(error).__name__,
                               "errno": getattr(error, "errno", None),
                               "detail": str(error)[:160]}
    try:
        out["lists"][path] = {"ok": True,
                              "entries": sorted(os.listdir(path))[:8]}
    except Exception as error:
        out["lists"][path] = {"ok": False,
                              "error": type(error).__name__,
                              "errno": getattr(error, "errno", None),
                              "detail": str(error)[:160]}
try:
    with open("/scratch/.sley-probe-scratch", "wb") as handle:
        handle.write(b"scratch-write-ok")
    with open("/scratch/.sley-probe-scratch", "rb") as handle:
        out["scratch_ok"] = handle.read() == b"scratch-write-ok"
    os.unlink("/scratch/.sley-probe-scratch")
except Exception:
    out["scratch_ok"] = False
sys.stdout.write(json.dumps(out))
"""


def probe_access(scratch_dir: Path, mask_paths: list[Path], *,
                 timeout_s: float = 60) -> dict:
    """Run an actual open/read/write probe inside the trial sandbox.

    Returns the probe's observed outcomes (errno-level), for assertion
    by tests: protected paths must be unreadable, scratch writable.
    """

    import json
    import tempfile
    require_bwrap()
    scratch = Path(scratch_dir)
    scratch.mkdir(mode=0o700, parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False,
                                     dir=str(scratch)) as handle:
        handle.write(PROBE_SCRIPT)
        script = handle.name
    try:
        spec = SandboxSpec(scratch_dir=scratch,
                           mask_paths=tuple(mask_paths))
        argv = confinement_argv(
            spec, ["python3", "/scratch/" + Path(script).name,
                   *[str(p) for p in mask_paths]])
        # The probe script must be visible at the same /scratch path:
        # it already lives in scratch, bound at /scratch.
        completed = subprocess.run(argv, capture_output=True,
                                   timeout=timeout_s, check=False)
    except subprocess.TimeoutExpired as error:
        raise ConfinementError(f"CONFINEMENT_TIMEOUT: {error}") from error
    except OSError as error:
        raise ConfinementError(f"CONFINEMENT_SPAWN_FAILED: {error}") from error
    finally:
        try:
            os.unlink(script)
        except OSError:
            pass
    if completed.returncode != 0:
        raise ConfinementError(
            f"CONFINEMENT_PROBE_FAILED: exit {completed.returncode}: "
            f"{completed.stderr.decode('utf-8', 'replace')[:400]}")
    try:
        return json.loads(completed.stdout.decode("utf-8"))
    except (UnicodeError, ValueError) as error:
        raise ConfinementError(
            f"CONFINEMENT_PROBE_FAILED: output: {error}") from error


def sha256_file(path: Path) -> str:
    import hashlib
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
