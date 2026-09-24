"""Claude Code provider: adapter, stream accounting, credential copy,
isolation, and the provider-process-only egress rule.

Offline tests use synthetic stream-json. The bwrap tests prove isolation
by in-sandbox outcomes. ``LiveClaudeLaunchTests`` runs the real
``claude -p`` once (a trivial prompt on the small tier) and only when
SLEY2_LIVE_CLAUDE_ROOT names the installed release directory; it spends a
few thousand tokens of the operator's usage and is never a trial.
"""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

from bench.live.confined import bwrap_available, confinement_argv
from bench.live.metrics import derive_provider_observation
from bench.live.provider import (
    CLAUDE_TOOLS,
    ClaudeCodeAdapter,
    ProviderError,
    parse_claude_stream_json,
    parse_provider_events,
)
from bench.live.provider_sandbox import (
    PROFILES,
    SANDBOX_WORKSPACE,
    EgressProxy,
    ProviderSandbox,
    ProviderSandboxError,
    arm_spec,
    prepare_layout,
    wrap_agent,
)

ROOT = Path(__file__).resolve().parents[3]
LIVE_ROOT = os.environ.get("SLEY2_LIVE_CLAUDE_ROOT", "")
HOST_CREDENTIAL = Path.home() / ".claude" / ".credentials.json"


def line(value: dict) -> bytes:
    return json.dumps(value, sort_keys=True).encode() + b"\n"


def stream(*, error: bool = False) -> bytes:
    return b"".join([
        line({"type": "system", "subtype": "init", "session_id": "s1", "tools": CLAUDE_TOOLS.split(",")}),
        line({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "a", "name": "Read", "input": {"file_path": "/workspace/program.py"}},
            {"type": "tool_use", "id": "b", "name": "Bash", "input": {"command": "python3 -m unittest -v"}}]}}),
        line({"type": "user", "message": {"content": [
            {"type": "tool_result", "tool_use_id": "a", "content": "def f():\n"},
            {"type": "tool_result", "tool_use_id": "b", "is_error": True,
             "content": [{"type": "text", "text": "FAILED (failures=1)"}]}]}}),
        line({"type": "rate_limit_event", "rate_limit_info": {"status": "allowed"}}),
        line({"type": "result", "subtype": "error_during_execution" if error else "success",
              "is_error": error, "session_id": "s1", "result": "done",
              "usage": {"input_tokens": 10, "cache_creation_input_tokens": 200,
                        "cache_read_input_tokens": 3000, "output_tokens": 40,
                        "output_tokens_details": {"thinking_tokens": 7}}}),
    ])


def claude_credential(path: Path, *, expires_in_s: int) -> Path:
    path.write_text(json.dumps({
        "claudeAiOauth": {"accessToken": "ACCESS", "refreshToken": "REFRESH-SECRET",
                          "expiresAt": int((time.time() + expires_in_s) * 1000),
                          "scopes": ["user:inference"], "subscriptionType": "max",
                          "rateLimitTier": "t"},
        "mcpOAuth": {"plugin:x": {"accessToken": "MCP-SECRET"}}}))
    return path


class StreamAccountingTests(unittest.TestCase):
    def test_usage_is_normalized_and_tools_are_paired(self) -> None:
        events = parse_provider_events("anthropic-claude-code-oauth", stream())
        self.assertEqual(events.input_tokens, 3210)
        self.assertEqual(events.cached_input_tokens, 3000)
        self.assertEqual((events.output_tokens, events.reasoning_output_tokens), (40, 7))
        self.assertEqual(events.tool_calls, 2)
        self.assertEqual(events.final_message, "done")
        prompt = b"p" * 5
        observed = derive_provider_observation(events, prompt)
        self.assertEqual(observed["model_input_tokens"], 3210)
        self.assertEqual(observed["total_observable_tokens"], 3250)
        self.assertEqual(observed["tool_calls"], 2)
        self.assertEqual(observed["files_inspected"], 1)
        self.assertEqual(observed["compile_or_check_attempts"], 1)
        self.assertEqual(observed["repair_loops"], 1)
        self.assertEqual(observed["context_bytes"], 5 + len("def f():\n") + len("FAILED (failures=1)"))

    def test_error_result_and_truncated_streams_fail(self) -> None:
        with self.assertRaisesRegex(ProviderError, "LIVE_PROVIDER_TURN_FAILED"):
            parse_claude_stream_json(stream(error=True))
        with self.assertRaisesRegex(ProviderError, "terminal"):
            parse_claude_stream_json(b"".join(stream().splitlines(keepends=True)[:-1]))
        with self.assertRaises(ProviderError):
            parse_provider_events("unknown-provider", stream())


class AdapterTests(unittest.TestCase):
    def test_command_pins_model_tools_and_no_operator_configuration(self) -> None:
        argv = ClaudeCodeAdapter("/opt/claude", "claude-haiku-4-5-20251001", "medium").command("/workspace")
        self.assertEqual(argv[:2], ["/opt/claude", "-p"])
        pairs = dict(zip(argv, argv[1:]))
        self.assertEqual(pairs["--model"], "claude-haiku-4-5-20251001")
        self.assertEqual(pairs["--tools"], "Bash,Read,Edit,Write,Glob,Grep")
        self.assertEqual(pairs["--permission-mode"], "bypassPermissions")
        self.assertEqual(pairs["--output-format"], "stream-json")
        for flag in ("--setting-sources=", "--strict-mcp-config", "--disable-slash-commands",
                     "--no-session-persistence", "--verbose"):
            self.assertIn(flag, argv)
        self.assertNotIn("--dangerously-skip-permissions", argv)
        for bad in ({"model": "claude-latest"}, {"reasoning_effort": "ultra"}):
            with self.assertRaises(ProviderError):
                ClaudeCodeAdapter(**{"executable": "/opt/claude", "model": "claude-haiku-4-5-20251001",
                                     "reasoning_effort": "medium", **bad})


class CredentialCopyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR"))
        self.root = Path(self.temporary.name)
        (self.root / "provider").mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def sandbox(self, expires_in_s: int) -> ProviderSandbox:
        return ProviderSandbox(provider_root=self.root / "provider",
                               auth_source=claude_credential(self.root / "c.json", expires_in_s=expires_in_s),
                               allowed_hosts=("api.anthropic.com",), profile="claude-code")

    def test_copy_carries_no_refresh_token_and_nothing_else(self) -> None:
        (self.root / "a").mkdir()
        layout = prepare_layout(self.root / "a", self.sandbox(8 * 3600), wall_time_budget_ms=3_600_000)
        self.assertEqual([p.name for p in layout.provider_home.iterdir()], [".credentials.json"])
        text = (layout.provider_home / ".credentials.json").read_text()
        self.assertNotIn("REFRESH-SECRET", text)
        self.assertNotIn("MCP-SECRET", text)
        self.assertEqual(json.loads(text)["claudeAiOauth"]["accessToken"], "ACCESS")
        self.assertEqual(oct((layout.provider_home / ".credentials.json").stat().st_mode & 0o777), "0o600")

    def test_token_that_would_expire_inside_the_attempt_is_refused_before_launch(self) -> None:
        (self.root / "b").mkdir()
        with self.assertRaisesRegex(ProviderSandboxError, "LIVE_PROVIDER_CREDENTIAL_EXPIRING"):
            prepare_layout(self.root / "b", self.sandbox(3600), wall_time_budget_ms=3_600_000)

    def test_profile_environment_is_enforced(self) -> None:
        sandbox = self.sandbox(8 * 3600)
        (self.root / "c").mkdir()
        layout = prepare_layout(self.root / "c", sandbox, wall_time_budget_ms=0)
        good = PROFILES["claude-code"].fixed_environment()
        arm_spec(arm_id="raw_files", sandbox=sandbox, layout=layout, environment=good,
                 workspace=self.root)
        with self.assertRaisesRegex(ProviderSandboxError, "CLAUDE_CONFIG_DIR"):
            arm_spec(arm_id="raw_files", sandbox=sandbox, layout=layout,
                     environment=dict(good, CLAUDE_CONFIG_DIR=str(Path.home() / ".claude")),
                     workspace=self.root)


EGRESS_PROBE = r"""
import json, socket, subprocess, sys
def ask(host):
    c = socket.create_connection(("127.0.0.1", 18080), timeout=10)
    c.sendall(f"CONNECT {host}:443 HTTP/1.1\r\n\r\n".encode())
    reply = c.recv(64).decode().split("\r\n")[0]
    c.close()
    return reply
if len(sys.argv) > 1:
    print(ask("example.org"))
    sys.exit(0)
out = {"agent": ask("example.com")}
out["child"] = subprocess.run([sys.executable, __file__, "child"], capture_output=True, text=True).stdout.strip()
paths = [str(p) for p in json.loads(open("/workspace/paths.json").read())]
out["visible"] = {p: __import__("os").path.exists(p) for p in paths}
import os
out["config"] = sorted(os.listdir(os.environ["CLAUDE_CONFIG_DIR"]))
print(json.dumps(out))
"""


@unittest.skipUnless(bwrap_available(), "needs bwrap")
class ClaudeSandboxIsolationTests(unittest.TestCase):
    def test_config_isolation_and_provider_process_only_egress(self) -> None:
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as temporary:
            root = Path(temporary)
            (root / "provider").mkdir()
            sandbox = ProviderSandbox(provider_root=root / "provider",
                                      auth_source=claude_credential(root / "c.json", expires_in_s=8 * 3600),
                                      allowed_hosts=("api.anthropic.com",), profile="claude-code")
            (root / "layout").mkdir()
            layout = prepare_layout(root / "layout", sandbox, wall_time_budget_ms=0)
            workspace = root / "ws"
            workspace.mkdir()
            (workspace / "probe.py").write_text(EGRESS_PROBE)
            secrets = [ROOT / "bench" / "fixtures", ROOT / "oracle", Path.home() / ".claude",
                       Path.home() / ".codex", HOST_CREDENTIAL]
            (workspace / "paths.json").write_text(json.dumps([str(p) for p in secrets]))
            spec = arm_spec(arm_id="raw_files", sandbox=sandbox, layout=layout,
                            environment=PROFILES["claude-code"].fixed_environment(), workspace=workspace)
            argv = confinement_argv(spec, wrap_agent(["/usr/bin/python3", f"{SANDBOX_WORKSPACE}/probe.py"]),
                                    workdir=SANDBOX_WORKSPACE)
            log = root / "egress.jsonl"
            with EgressProxy(layout.egress_socket, log, ("api.anthropic.com",)):
                completed = subprocess.run(argv, capture_output=True, timeout=60, check=False)
            self.assertEqual(completed.returncode, 0, completed.stderr.decode()[-500:])
            observed = json.loads(completed.stdout)
            # The provider process itself reaches the proxy (which denies a
            # non-allowlisted host); its child is refused inside the sandbox.
            self.assertIn("403", observed["agent"])
            self.assertIn("403", observed["child"])
            decisions = [json.loads(item) for item in log.read_text().splitlines()]
            self.assertEqual([d["decision"] for d in decisions].count("deny"), 1)
            self.assertIn("example.com:443", decisions[[d["decision"] for d in decisions].index("deny")]["target"])
            self.assertIn("deny_local_process", [d["decision"] for d in decisions])
            self.assertFalse(any("example.org" in d["target"] for d in decisions if d["decision"] == "deny"))
            for path, visible in observed["visible"].items():
                self.assertFalse(visible, path)
            self.assertEqual(observed["config"], [".credentials.json"])


@unittest.skipUnless(bwrap_available() and LIVE_ROOT and HOST_CREDENTIAL.is_file(),
                     "needs bwrap + SLEY2_LIVE_CLAUDE_ROOT + a host Claude login")
class LiveClaudeLaunchTests(unittest.TestCase):
    """One real, trivial `claude -p` turn in the raw arm's sandbox."""

    def test_real_launch_usage_and_command_isolation(self) -> None:
        from bench.live.process import run_provider_process

        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as temporary:
            root = Path(temporary)
            sandbox = ProviderSandbox(provider_root=Path(LIVE_ROOT), auth_source=HOST_CREDENTIAL,
                                      allowed_hosts=("api.anthropic.com",), profile="claude-code")
            (root / "layout").mkdir()
            layout = prepare_layout(root / "layout", sandbox, wall_time_budget_ms=600_000)
            workspace = root / "ws"
            workspace.mkdir()
            environment = dict(PROFILES["claude-code"].fixed_environment(), LANG="C.UTF-8")
            spec = arm_spec(arm_id="raw_files", sandbox=sandbox, layout=layout,
                            environment=environment, workspace=workspace)
            adapter = ClaudeCodeAdapter(f"{LIVE_ROOT}/claude", "claude-haiku-4-5-20251001", "low")
            argv = confinement_argv(spec, wrap_agent(adapter.command(SANDBOX_WORKSPACE)),
                                    workdir=SANDBOX_WORKSPACE)
            prompt = (f"Run exactly this one shell command with the Bash tool and then reply DONE: "
                      f"ls {ROOT}/bench/fixtures {Path.home()}/.claude; "
                      f"curl -sS -m 5 https://api.anthropic.com >/dev/null && echo NET_OPEN || echo NET_BLOCKED").encode()
            with EgressProxy(layout.egress_socket, root / "egress.jsonl", ("api.anthropic.com",)):
                capture = run_provider_process(argv, prompt, timeout_ms=300_000,
                                               max_output_bytes=64 * 1024 * 1024, environment=environment)
            self.assertEqual(capture.exit_code, 0, capture.stderr[-500:])
            events = parse_provider_events("anthropic-claude-code-oauth", capture.stdout)
            self.assertGreater(events.input_tokens, 0)
            self.assertGreaterEqual(events.tool_calls, 1)
            outputs = " ".join(item["aggregated_output"] for item in events.commands)
            self.assertIn("NET_BLOCKED", outputs)
            self.assertIn("No such file or directory", outputs)
            decisions = [json.loads(item)["decision"] for item in (root / "egress.jsonl").read_text().splitlines()]
            self.assertIn("allow", decisions)
            self.assertIn("deny_local_process", decisions)


if __name__ == "__main__":
    unittest.main()
