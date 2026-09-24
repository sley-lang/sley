"""Outer provider sandbox: one launch shape for every arm.

Offline tests pin the sandbox layout, the environment contract, and the
egress proxy's allowlist. The bwrap tests prove isolation facts by actual
outcomes inside a sandbox (no route out, allowlist refusal, no repository
visibility). The provider tests (gated on a real Codex release directory
in SLEY2_LIVE_PROVIDER_ROOT) run each arm's documented tool surface under
the provider's own command sandbox inside the outer sandbox, the exact
nesting a live attempt uses; the sley_2_0 proof enters through
execute_attempt with a stand-in agent. None of these are model trials.
"""

from __future__ import annotations

import hashlib
import json
import os
import socket
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path

from bench.live.artifacts import ArtifactStore
from bench.live.attempts import verify_attempts
from bench.live.campaign import execute_attempt
from bench.live.confined import bwrap_available, confinement_argv
from bench.live.manifest import build_manifest, write_manifest_once
from bench.live.provider_sandbox import (
    FIXED_ENVIRONMENT,
    SANDBOX_WORKSPACE,
    EgressProxy,
    ProviderSandbox,
    ProviderSandboxError,
    arm_spec,
    check_provider_environment,
    prepare_layout,
    wrap_agent,
)
from bench.live.taskpacks import stage_initial
from bench.live.tooling import stage_tooling

ROOT = Path(__file__).resolve().parents[3]
PROVIDER_ROOT = os.environ.get("SLEY2_LIVE_PROVIDER_ROOT", "")
NEEDS_BWRAP = bwrap_available()
NEEDS_PROVIDER = bool(PROVIDER_ROOT) and Path(PROVIDER_ROOT, "bin", "codex").is_file()
NEEDS_BINARY = bool(os.environ.get("SLEY2_SLEY_BINARY", ""))


def _sha(label: str) -> str:
    return hashlib.sha256(label.encode()).hexdigest()


def sandbox_manifest() -> dict:
    environment = dict(FIXED_ENVIRONMENT)
    environment.update({"LANG": "C.UTF-8", "TZ": "UTC"})
    return build_manifest(
        run_id="sandbox-proof",
        created_at_utc="2026-09-24T00:00:00Z",
        repo_commit="3" * 40,
        model_exact_version="gpt-6-luna",
        model_tier="small",
        reasoning_effort="medium",
        trial_count=1,
        random_seeds=[1],
        context_budget=6_000_000,
        action_budget=200,
        wall_time_budget=600_000,
        retry_policy={"provider_attempts": 1, "retryable_failures": []},
        hardware_manifest={"node": "test"},
        cache_state="cold-per-attempt",
        environment_manifest={"provider_environment": environment},
        arm_fixture_digests={arm: _sha(arm) for arm in ("raw_files", "sley_1_2_0", "sley_2_0")},
        tool_description_digests={arm: _sha(arm + "t") for arm in ("raw_files", "sley_1_2_0", "sley_2_0")},
        oracle_digest=_sha("o"),
        prompt_template_digest=_sha("p"),
        provider_executable_sha256=_sha("c"),
        provider_version="codex-cli test",
    )


def fake_sandbox(root: Path) -> ProviderSandbox:
    provider = root / "provider"
    (provider / "bin").mkdir(parents=True)
    auth = root / "auth.json"
    auth.write_text("{}")
    return ProviderSandbox(provider_root=provider, auth_source=auth)


class LayoutTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR"))
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_environment_must_name_the_sandbox_layout(self) -> None:
        good = dict(FIXED_ENVIRONMENT, LANG="C.UTF-8")
        check_provider_environment(good)
        for name in FIXED_ENVIRONMENT:
            bad = dict(good)
            bad[name] = "/home/operator"
            with self.assertRaisesRegex(ProviderSandboxError, name):
                check_provider_environment(bad)

    def test_fresh_layout_carries_only_a_credential_placeholder(self) -> None:
        layout = prepare_layout(self.root)
        self.assertEqual(sorted(p.name for p in layout.provider_home.iterdir()), ["auth.json"])
        self.assertEqual((layout.provider_home / "auth.json").stat().st_size, 0)
        self.assertEqual(list(layout.home.iterdir()), [])
        with self.assertRaises(FileExistsError):
            prepare_layout(self.root)

    def test_raw_and_legacy_share_one_shape_and_only_legacy_adds_its_tool(self) -> None:
        sandbox = fake_sandbox(self.root)
        environment = dict(FIXED_ENVIRONMENT, LANG="C.UTF-8")
        specs = {}
        for arm in ("raw_files", "sley_1_2_0"):
            layout_root = self.root / arm
            layout_root.mkdir()
            specs[arm] = arm_spec(arm_id=arm, sandbox=sandbox,
                                  layout=prepare_layout(layout_root),
                                  environment=environment,
                                  workspace=self.root / "ws")
        raw_dest = [bind[2] for bind in specs["raw_files"].extra_binds]
        legacy_dest = [bind[2] for bind in specs["sley_1_2_0"].extra_binds]
        self.assertEqual(legacy_dest[: len(raw_dest)], raw_dest)
        self.assertIn(SANDBOX_WORKSPACE, raw_dest)
        extra = legacy_dest[len(raw_dest):]
        self.assertTrue(extra)
        for destination in extra:
            self.assertNotIn("/fixtures", destination)
            self.assertNotIn("/oracle", destination)
        for spec in specs.values():
            self.assertFalse(spec.share_net)
            self.assertEqual(dict(spec.setenv)["CODEX_HOME"], "/codex-home")
        with self.assertRaises(ProviderSandboxError):
            (self.root / "none").mkdir()
            arm_spec(arm_id="raw_files", sandbox=sandbox,
                     layout=prepare_layout(self.root / "none"),
                     environment=environment)

    def test_wrapped_agent_keeps_its_argv(self) -> None:
        wrapped = wrap_agent(["/opt/codex", "exec", "-"])
        self.assertEqual(wrapped[-3:], ["/opt/codex", "exec", "-"])
        self.assertEqual(wrapped[0], "/usr/bin/python3")


class EgressProxyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR"))
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _ask(self, proxy_socket: Path, request: bytes) -> bytes:
        client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        client.settimeout(10)
        client.connect(str(proxy_socket))
        client.sendall(request)
        reply = client.recv(4096)
        client.close()
        return reply

    def test_only_allowlisted_connect_is_admitted(self) -> None:
        log = self.root / "log.jsonl"
        with EgressProxy(self.root / "p.sock", log, ("chatgpt.com",)) as proxy:
            self.assertIn(b" 403 ", self._ask(self.root / "p.sock",
                                              b"CONNECT example.com:443 HTTP/1.1\r\n\r\n"))
            self.assertIn(b" 403 ", self._ask(self.root / "p.sock",
                                              b"CONNECT chatgpt.com:80 HTTP/1.1\r\n\r\n"))
            self.assertIn(b" 405 ", self._ask(self.root / "p.sock",
                                              b"GET http://chatgpt.com/ HTTP/1.1\r\n\r\n"))
        decisions = [json.loads(line)["decision"] for line in log.read_text().splitlines()]
        self.assertEqual(decisions, ["deny", "deny", "deny_method"])
        self.assertEqual(len(proxy.decisions), 3)

    def test_allowlisted_connect_relays_bytes(self) -> None:
        upstream = socket.socket()
        upstream.bind(("127.0.0.1", 0))
        upstream.listen(1)
        port = upstream.getsockname()[1]

        def echo() -> None:
            connection, _ = upstream.accept()
            connection.sendall(b"pong:" + connection.recv(16))
            connection.close()

        threading.Thread(target=echo, daemon=True).start()
        import bench.live.provider_sandbox as module

        original_create = socket.create_connection
        original_port = module.PROVIDER_PORT
        module.PROVIDER_PORT = port
        try:
            socket.create_connection = lambda address, timeout=None: original_create(("127.0.0.1", port), timeout)  # type: ignore[assignment]
            with EgressProxy(self.root / "p.sock", self.root / "log.jsonl", ("localhost",)):
                client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                client.settimeout(10)
                client.connect(str(self.root / "p.sock"))
                client.sendall(f"CONNECT localhost:{port} HTTP/1.1\r\n\r\n".encode())
                self.assertIn(b" 200 ", client.recv(4096))
                client.sendall(b"ping")
                self.assertEqual(client.recv(64), b"pong:ping")
                client.close()
        finally:
            socket.create_connection = original_create  # type: ignore[assignment]
            module.PROVIDER_PORT = original_port
            upstream.close()


PROBE = r"""
import json, os, socket, sys
out = {}
try:
    s = socket.create_connection(("1.1.1.1", 443), timeout=5); s.close(); out["direct"] = "open"
except OSError as error:
    out["direct"] = type(error).__name__
c = socket.create_connection(("127.0.0.1", 18080), timeout=10)
c.sendall(b"CONNECT example.com:443 HTTP/1.1\r\n\r\n")
out["proxy_denied"] = c.recv(64).decode().split("\r\n")[0]
for path in sys.argv[1:]:
    out[path] = os.path.exists(path)
out["codex_home"] = sorted(os.listdir(os.environ["CODEX_HOME"]))
print(json.dumps(out))
"""


@unittest.skipUnless(NEEDS_BWRAP, "needs bwrap")
class SandboxIsolationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR"))
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_no_route_out_and_no_repository_visibility(self) -> None:
        sandbox = fake_sandbox(self.root)
        layout_root = self.root / "layout"
        layout_root.mkdir()
        layout = prepare_layout(layout_root)
        workspace = self.root / "ws"
        workspace.mkdir()
        (workspace / "probe.py").write_text(PROBE)
        spec = arm_spec(arm_id="raw_files", sandbox=sandbox, layout=layout,
                        environment=dict(FIXED_ENVIRONMENT, LANG="C.UTF-8"),
                        workspace=workspace)
        secret_paths = [str(ROOT / "bench" / "fixtures"), str(ROOT / "oracle"),
                        str(Path.home() / ".codex")]
        argv = confinement_argv(
            spec, wrap_agent(["/usr/bin/python3", f"{SANDBOX_WORKSPACE}/probe.py",
                              *secret_paths]),
            workdir=SANDBOX_WORKSPACE)
        with EgressProxy(layout.egress_socket, self.root / "log.jsonl", ("chatgpt.com",)):
            completed = subprocess.run(argv, capture_output=True, timeout=60, check=False)
        self.assertEqual(completed.returncode, 0, completed.stderr.decode()[-400:])
        observed = json.loads(completed.stdout)
        self.assertNotEqual(observed["direct"], "open")
        self.assertIn("403", observed["proxy_denied"])
        for path in secret_paths:
            self.assertFalse(observed[path], path)
        self.assertEqual(observed["codex_home"], ["auth.json"])


def _inner(provider_root: str) -> list[str]:
    """The provider's own command sandbox exactly as `codex exec` applies it."""

    return [f"{provider_root}/bin/codex", "sandbox",
            "-c", 'sandbox_mode="workspace-write"',
            "-c", "sandbox_workspace_write.network_access=true", "--"]


@unittest.skipUnless(NEEDS_BWRAP and NEEDS_PROVIDER,
                     "needs bwrap + SLEY2_LIVE_PROVIDER_ROOT")
class ProviderSurfaceTests(unittest.TestCase):
    """Each arm's documented tool surface, run by the provider's command
    sandbox nested in the outer sandbox (the live attempt nesting)."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR"))
        self.root = Path(self.temporary.name)
        auth = self.root / "auth.json"
        auth.write_text("{}")
        self.sandbox = ProviderSandbox(provider_root=Path(PROVIDER_ROOT), auth_source=auth)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _run_in_arm(self, arm: str, task: str, command: list[str]) -> subprocess.CompletedProcess:
        workspace = self.root / f"{arm}-ws"
        stage_initial(arm, task, workspace)
        stage_tooling(arm, workspace)
        layout_root = self.root / f"{arm}-layout"
        layout_root.mkdir()
        layout = prepare_layout(layout_root)
        spec = arm_spec(arm_id=arm, sandbox=self.sandbox, layout=layout,
                        environment=dict(FIXED_ENVIRONMENT, LANG="C.UTF-8"),
                        workspace=workspace)
        argv = confinement_argv(spec, wrap_agent(_inner(PROVIDER_ROOT) + command),
                                workdir=SANDBOX_WORKSPACE)
        with EgressProxy(layout.egress_socket, self.root / f"{arm}.jsonl", ("chatgpt.com",)):
            return subprocess.run(argv, capture_output=True, timeout=300, check=False)

    def test_raw_tests_run(self) -> None:
        completed = self._run_in_arm("raw_files", "S2B-REPAIR-001",
                                     ["python3", "-m", "unittest", "-v"])
        self.assertIn(b"Ran ", completed.stderr, completed.stderr[-600:])

    def test_legacy_tool_runs_the_frozen_artifact(self) -> None:
        workspace_name = "program.sley"
        completed = self._run_in_arm("sley_1_2_0", "S2B-REPAIR-001",
                                     [f"{SANDBOX_WORKSPACE}/.sley-live/sley-tool",
                                      "check", workspace_name])
        reply = json.loads(completed.stdout.decode().strip().splitlines()[-1])
        self.assertEqual(reply.get("outcome"), "completed", completed.stdout[-600:])


class _SandboxedStandIn:
    """Stand-in agent at the provider boundary: the provider's own command
    sandbox runs the deterministic mediated client, so the gateway socket
    must be reachable from a provider-sandboxed command."""

    def __init__(self, sequence: str, network: bool) -> None:
        self.model = "gpt-6-luna"
        self.reasoning_effort = "medium"
        self._sequence = sequence
        self._network = network

    def extra_scratch_files(self) -> list[tuple[str, bytes]]:
        return [("mediated_client.py",
                 (ROOT / "bench" / "live" / "mediated_client.py").read_bytes())]

    def command(self, workspace) -> list[str]:
        inner = _inner(PROVIDER_ROOT)
        if not self._network:
            inner = [f"{PROVIDER_ROOT}/bin/codex", "sandbox",
                     "-c", 'sandbox_mode="workspace-write"', "--"]
        return inner + ["python3", "/scratch/mediated_client.py", self._sequence]


@unittest.skipUnless(NEEDS_BWRAP and NEEDS_PROVIDER and NEEDS_BINARY,
                     "needs bwrap + SLEY2_LIVE_PROVIDER_ROOT + SLEY2_SLEY_BINARY")
class MediatedProviderSandboxTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR"))
        self.root = Path(self.temporary.name)
        self.run = self.root / "run"
        self.run.mkdir()
        write_manifest_once(self.run / "run_manifest.json", sandbox_manifest())
        self.store = ArtifactStore(self.run / "artifacts")
        auth = self.root / "auth.json"
        auth.write_text("{}")
        self.sandbox = ProviderSandbox(provider_root=Path(PROVIDER_ROOT), auth_source=auth)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _attempt(self, network: bool) -> dict:
        def oracle(arm_id, task_id, candidate):
            line = json.dumps({"arm": "sley2", "code": None, "detail": "stub",
                               "status": "accepted", "task_id": task_id},
                              sort_keys=True).encode() + b"\n"
            return {"status": "accepted", "code": None}, line, b""

        return execute_attempt(
            run_directory=self.run, store=self.store,
            adapter=_SandboxedStandIn("finish_skeleton", network),
            task_id="S2B-TYPE-001", arm_id="sley_2_0", seed=1,
            workspace_parent=self.root / "ws", oracle_runner=oracle,
            utc_now=lambda: "2026-09-24T00:00:01Z",
            provider_sandbox=self.sandbox)

    def test_gateway_reachable_from_provider_sandboxed_command(self) -> None:
        record = self._attempt(network=True)
        self.assertEqual(record["status"], "accepted", record["failure_code"])
        self.assertGreater(record["metrics"]["tool_calls"], 0)
        self.assertEqual(len(verify_attempts(self.run, self.store)), 1)

    def test_provider_default_sandbox_blocks_the_gateway(self) -> None:
        # Regression for the pilot finding: without network access the
        # provider's command sandbox refuses the unix-socket connect, so
        # no tool call can reach the gateway and nothing is judged.
        record = self._attempt(network=False)
        self.assertEqual(record["status"], "harness_failure")
        self.assertEqual(record["failure_code"], "LIVE_PROVIDER_EXIT_NONZERO")
        exchanges = self.run / "captures" / record["attempt_id"] / "exchanges.jsonl"
        self.assertFalse(exchanges.exists() and exchanges.read_bytes().strip())


if __name__ == "__main__":
    unittest.main()
