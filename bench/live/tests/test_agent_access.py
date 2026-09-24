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
        code, _ = self.run_tool("open")
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
        code, _ = self.run_tool("open")
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
        code, opened = self.run_tool("open")
        self.assertEqual(code, 0)
        code, _ = self.run_tool("revision", opened["report"]["decoded"]["tx"])
        self.assertEqual(code, 0)
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

    def seal(self, command: str, session: list) -> None:
        sley2_tool.append_transcript(self.ws, [command], True, 5, 100, session, None)

    Q = "a1" * 32

    @staticmethod
    def cur(n: int) -> str:
        return "1:" + f"{n:02x}" * 32

    def bounded_page(self, truncated: bool, omitted: int, returned: int = 100,
                     nxt: str | None = None, query: str | None = None) -> list:
        chain = {"query": query or self.Q, "after": None,
                 "truncated": truncated,
                 "next": nxt if nxt is not None else (self.cur(1) if truncated else None)}
        return [
            {"direction": "request", "method": "query.root",
             "body_sha256": "0" * 64},
            {"direction": "response", "failed": False,
             "body_sha256": "1" * 64, "omitted": omitted,
             "truncated": truncated, "returned_bytes": returned,
             "chain": chain},
        ]

    def continuation_follow(self, returned: int = 100,
                            truncated: bool = False, omitted: int = 0,
                            after: str | None = None, nxt: str | None = None,
                            failed: bool = False) -> list:
        response = {"direction": "response", "failed": failed,
                    "body_sha256": "3" * 64, "omitted": omitted,
                    "truncated": truncated, "returned_bytes": returned}
        if not failed:
            response["chain"] = {"query": self.Q,
                                 "after": after or self.cur(1),
                                 "truncated": truncated, "next": nxt}
        return [
            {"direction": "request", "method": "query.continue",
             "body_sha256": "2" * 64},
            response,
        ]

    def assert_rejects(self, needle: str) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn(needle, raised.exception.detail)

    def test_bounded_continuation_positive_accepted(self) -> None:
        # A genuinely bounded page (truncated with explicit omitted
        # accounting) followed by its query.continue is the documented
        # continuation path: not a whole-store read, no hidden
        # truncation.
        session = self.bounded_page(True, 5) + self.continuation_follow()
        self.seal("raw", session)
        access = self.audit()
        self.assertEqual(access["whole_store_reads"], 0)
        self.assertEqual(access["continuations"], 1)
        self.assertGreaterEqual(access["bounded_reads"], 2)

    def test_multi_page_continuation_chain_accepted(self) -> None:
        # Each truncated page opens its next cursor; each continue
        # discharges the page whose next cursor it carries. The final
        # page's `omitted` counts entities earlier pages returned.
        session = (self.bounded_page(True, 6)
                   + self.continuation_follow(truncated=True, omitted=6,
                                              nxt=self.cur(2))
                   + self.continuation_follow(omitted=6, after=self.cur(2)))
        self.seal("raw", session)
        access = self.audit()
        self.assertEqual(access["continuations"], 2)

    def test_continuation_across_invocations_accepted(self) -> None:
        # One request per invocation (the documented sley-tool route): the
        # continue in the next invocation discharges the page it names.
        self.seal("raw", self.bounded_page(True, 5))
        self.seal("raw", self.continuation_follow(omitted=5))
        access = self.audit()
        self.assertEqual(access["continuations"], 1)

    def test_truncated_continuation_without_follow_rejects(self) -> None:
        # Stopping on a truncated continuation page is hidden truncation.
        session = (self.bounded_page(True, 6)
                   + self.continuation_follow(truncated=True, omitted=3,
                                              nxt=self.cur(2)))
        self.seal("raw", session)
        self.assert_rejects("without continuation")

    def test_refused_continue_does_not_discharge(self) -> None:
        self.seal("raw", self.bounded_page(True, 5))
        self.seal("raw", self.continuation_follow(failed=True))
        self.assert_rejects("without continuation")

    def test_past_the_end_cursor_rejects(self) -> None:
        self.seal("raw", self.bounded_page(True, 5))
        self.seal("raw", self.continuation_follow(
            omitted=5, after="1:" + "ff" * 32))
        self.assert_rejects("inconsistent continuation")

    def test_two_truncated_pages_one_continue_rejects(self) -> None:
        self.seal("raw", self.bounded_page(True, 5))
        self.seal("raw", self.bounded_page(True, 5, query="b2" * 32))
        self.seal("raw", self.continuation_follow(omitted=5))
        self.assert_rejects("without continuation")

    def test_prior_tool_version_rejects(self) -> None:
        # Evidence stamped with the revision 4 tool identity ("1") is not
        # evidence of the revision 5 surface.
        self.seal("raw", self.bounded_page(False, 0))
        chain = self.ws / sley2_tool.CHAIN_NAME
        entry = json.loads(chain.read_text(encoding="utf-8").splitlines()[0])
        entry["tool_version"] = "1"
        body = {key: value for key, value in entry.items() if key != "hash"}
        sealed = sley2_tool._chain_entry(body, "0" * 64)
        chain.write_text(json.dumps(sealed, sort_keys=True) + "\n",
                         encoding="utf-8")
        self.assertEqual(sley2_tool.TOOL_VERSION, "2")
        self.assert_rejects("tool version 1")

    def test_hidden_truncation_rejects(self) -> None:
        # Truncated page with no following continue: hidden truncation.
        self.seal("raw", self.bounded_page(True, 5))
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("continuation", raised.exception.detail)

    def test_omitted_without_continuation_rejects(self) -> None:
        self.seal("raw", self.bounded_page(False, 3))
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_inconsistent_continuation_rejects(self) -> None:
        # query.continue with no preceding truncated page.
        self.seal("raw", self.continuation_follow())
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("inconsistent", raised.exception.detail)

    def test_over_cap_response_rejects(self) -> None:
        self.seal("raw", self.bounded_page(
            False, 0, returned=sley2_tool.MAX_RESPONSE_BYTES + 1))
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("over bound", raised.exception.detail)

    def test_cumulative_budget_rejects(self) -> None:
        # Each page fits the per-response cap, but cumulative bytes
        # exceed the trial budget: unbounded accumulation rejects.
        pages = 5
        per = judge.AGENT_CUMULATIVE_RESPONSE_BUDGET // pages + 1
        for _ in range(pages):
            self.seal("raw", self.bounded_page(False, 0, returned=per))
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.audit()
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("cumulative", raised.exception.detail)

    def test_incomplete_impact_facts_reject(self) -> None:
        # Required facts unresolved: the pristine pack has no added
        # member (added-member diff is empty), and a claimed member is
        # absent from every impact constant (not merely a
        # read-boundary question).
        from bench.live.sley2_tool import Session
        session = Session(SLEY, self.ws, [], seed_pack=True)
        try:
            manifest = json.loads((TASK_DIR / "task_manifest.json").read_text())
            typedef = manifest["entities"]["typedef"]
            added = judge._typedef_added_members(
                TASK_DIR, session, self.ws, typedef)
            self.assertEqual(added, [])
            with self.assertRaises(judge.JudgeRejection) as raised:
                judge._verify_impact_closure(
                    session, self.ws, typedef,
                    [("ff" * 32, {"variant": "Bool"})])
            self.assertEqual(raised.exception.code, "ORACLE_IMPACT_INCOMPLETE")
        finally:
            session.close()


if __name__ == "__main__":
    unittest.main()
