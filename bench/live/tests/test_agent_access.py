"""Agent-access evidence tests for the CONTEXT bounded-maintenance flow.

The tool chains every invocation at the trusted agent boundary; the
judge verifies the chain and derives whole_store_reads (never
defaulted) before full CONTEXT acceptance. Missing, incomplete, or
unverifiable evidence rejects under the frozen unbounded-read code.
"""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from bench.fixtures import sley2_live_judge as judge
from bench.live import sley2_tool
from bench.live.taskpacks import stage_initial
from bench.live.tooling import stage_tooling


ROOT = Path(__file__).resolve().parents[3]
SLEY = Path(os.environ.get("SLEY2_SLEY_BINARY", "/home/gfarch/Work/target-sley2-succ/debug/sley"))
TASK_DIR = ROOT / "bench" / "fixtures" / "sley2" / "S2B-CONTEXT-001"


class AgentAccessTests(unittest.TestCase):
    def setUp(self) -> None:
        if not (SLEY.is_file() and os.access(SLEY, os.X_OK)):
            self.skipTest("sley binary unavailable")
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.ws = Path(self.temp.name) / "ws"
        stage_initial("sley_2_0", "S2B-CONTEXT-001", self.ws)
        stage_tooling("sley_2_0", self.ws)
        manifest = json.loads((TASK_DIR / "task_manifest.json").read_text())
        self.typedef = manifest["entities"]["typedef"]
        self._env = unittest.mock.patch.dict(os.environ, {"SLEY2_SLEY_BINARY": str(SLEY)})
        self._env.start()
        self.addCleanup(self._env.stop)

    def run_tool(self, *argv: str) -> tuple[int, dict]:
        import io
        out = io.StringIO()
        saved_cwd = os.getcwd()
        os.chdir(self.ws)
        try:
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
        finally:
            os.chdir(saved_cwd)
        return code, json.loads(out.getvalue())

    def audit(self) -> dict:
        return judge._audit_agent_access(TASK_DIR, self.ws)

    def test_missing_chain_rejects(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_bounded_reads_derive_zero_whole_store(self) -> None:
        code, _ = self.run_tool("read", self.typedef)
        self.assertEqual(code, 0)
        code, _ = self.run_tool("revision")
        self.assertEqual(code, 0)
        access = self.audit()
        self.assertEqual(access["whole_store_reads"], 0)
        self.assertEqual(access["targeted_reads"], 1)
        self.assertEqual(access["operations"], 0)
        self.assertEqual(access["refusals"], 0)
        self.assertLessEqual(access["max_response_bytes"], sley2_tool.MAX_RESPONSE_BYTES)

    def test_inventory_marks_whole_store_read(self) -> None:
        # A chained inventory entry derives whole_store_reads=1. The entry
        # is sealed through the real tool path; running a live inventory
        # over the 10k pack would enumerate every object (minutes), which
        # is itself why enumeration is the bounded violation here.
        sley2_tool.append_transcript(self.ws, ["inventory"], True, 5, 100, [], None)
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("whole_store_reads=1", raised.exception.detail)

    def test_tampered_chain_rejects(self) -> None:
        code, _ = self.run_tool("revision")
        self.assertEqual(code, 0)
        chain = self.ws / sley2_tool.CHAIN_NAME
        lines = chain.read_text(encoding="utf-8").splitlines()
        entry = json.loads(lines[0])
        entry["summary"]["requests"] += 1
        chain.write_text(json.dumps(entry) + "\n", encoding="utf-8")
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_truncated_chain_rejects(self) -> None:
        self.run_tool("revision")
        self.run_tool("revision")
        chain = self.ws / sley2_tool.CHAIN_NAME
        lines = chain.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 2)
        chain.write_text(lines[1] + "\n", encoding="utf-8")
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_refused_command_counted_as_refusal(self) -> None:
        code, _ = self.run_tool("read", self.typedef)
        self.assertEqual(code, 0)
        code, _ = self.run_tool("raw", "commit", "00")
        self.assertEqual(code, 2)
        access = self.audit()
        self.assertEqual(access["whole_store_reads"], 0)
        self.assertGreaterEqual(access["refusals"], 1)


if __name__ == "__main__":
    unittest.main()
