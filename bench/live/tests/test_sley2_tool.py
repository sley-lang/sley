"""Deterministic integration tests for the live sley_2_0 trial tool.

No model calls, no campaign: a real `sley serve` binary over scratch
repositories seeded with the frozen conformance exchange pack exercises
the exact trial mechanics (session, reads, propose, finish, refusals).
"""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from bench.live import mediated_sley, sley2_codecs, sley2_tool
from bench.live.tooling import SLEY2_TOOLING
from bench.sley2 import runner


ROOT = Path(__file__).resolve().parents[3]
SLEY = Path(os.environ.get("SLEY2_SLEY_BINARY", "/home/dev/Work/target-sley2-succ/debug/sley"))
# Mechanics fixture: a real trial pack whose policy admits the trial
# principal (the conformance exchange pack denies it at phase 9, so
# no-op proposals there can never decide Valid).
PACK_PATH = ROOT / "bench" / "fixtures" / "sley2" / "S2B-TEST-001" / "base.pack"


class Sley2ToolSurfacePinTests(unittest.TestCase):
    """Binary-free pins over the coupled command/method enumerations."""

    def test_tool_methods_equal_runner_allowlist_in_order(self) -> None:
        # The tool's guard list and the smoke runner's frozen allowlist are
        # the same nineteen names in the same order (contract revision 5).
        self.assertEqual(tuple(sley2_tool.TOOL_METHODS),
                         tuple(runner.ARM_AFFORDANCES))
        self.assertEqual(len(sley2_tool.TOOL_METHODS), 19)
        self.assertEqual(sley2_tool.TOOL_METHODS[-1], "workspace.open")

    def test_dispatch_arity_refuses_before_any_server_contact(self) -> None:
        # `revision` requires exactly one 64-hex transaction id (the tx of
        # a prior `open`); `open` takes none. Every malformed form is
        # refused by the dispatcher itself, before any request is sent.
        class NoServer:
            def call(self, method, body_hex):
                raise AssertionError(f"server contacted: {method}")

            @property
            def head(self):
                raise AssertionError("harness head read")

        for argv in (["revision"], ["revision", "ab" * 31],
                     ["revision", "ab" * 33], ["revision", "zz" * 32],
                     ["revision", "ab" * 32, "cd" * 32], ["open", "00"]):
            with self.subTest(argv=argv):
                with self.assertRaises(sley2_tool.Sley2ToolError):
                    sley2_tool.dispatch(NoServer(), Path("."), argv)


class Sley2ToolTests(unittest.TestCase):
    def setUp(self) -> None:
        if not (SLEY.is_file() and os.access(SLEY, os.X_OK)):
            self.skipTest("sley binary unavailable")
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.ws = self.root / "ws"
        self.ws.mkdir(mode=0o700)
        (self.ws / "repo").mkdir(mode=0o700)
        (self.ws / "base.pack").write_bytes(PACK_PATH.read_bytes())
        self.env = dict(os.environ)
        self.env["SLEY2_SLEY_BINARY"] = str(SLEY)
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

    def test_unknown_commands_and_bad_shapes_refuse(self) -> None:
        for argv in (["nope"], ["read"], ["read", "zz"], ["read", "ab" * 31],
                     ["raw", "commit", "00"], ["raw", "query.root", "zz"],
                     ["propose", "not-json"], ["propose", "[]"], ["finish"]):
            with self.subTest(argv=argv):
                code, result = self.run_tool(*argv)
                self.assertEqual(code, 2)
                self.assertIn("LIVE_SLEY2_TOOL_INVALID", json.dumps(result))

    def test_missing_pack_or_binary_refuses(self) -> None:
        (self.ws / "base.pack").unlink()
        code, _ = self.run_tool("open")
        self.assertEqual(code, 2)
        (self.ws / "base.pack").write_bytes(PACK_PATH.read_bytes())
        with mock.patch.dict(os.environ, {}, clear=True):
            code, _ = self.run_tool("open")
            self.assertEqual(code, 2)
        # Control: with pack and binary restored the same command succeeds,
        # so the two refusals above are the pack and binary refusals.
        code, _ = self.run_tool("open")
        self.assertEqual(code, 0)

    def test_inventory_lists_served_objects_with_entities(self) -> None:
        code, result = self.run_tool("inventory")
        self.assertEqual(code, 0)
        inventory = result["report"]["inventory"]
        self.assertGreater(inventory["count"], 0)
        first = inventory["objects"][0]
        self.assertEqual(len(first["object"]), 64)
        self.assertEqual(len(first["entity"]), 64)
        self.assertIsInstance(first["kind"], int)

    def test_read_revision_and_session_views(self) -> None:
        code, result = self.run_tool("inventory")
        entity = result["report"]["inventory"]["objects"][0]["entity"]
        code, result = self.run_tool("read", entity)
        self.assertEqual(code, 0)
        self.assertFalse(result["report"]["failed"])
        self.assertIn("decoded", result["report"])
        code, opened = self.run_tool("open")
        self.assertEqual(code, 0)
        self.assertFalse(opened["report"]["failed"])
        head = opened["report"]["decoded"]
        self.assertEqual(len(head["tx"]), 64)
        code, result = self.run_tool("revision", head["tx"])
        self.assertEqual(code, 0)
        self.assertFalse(result["report"]["failed"])
        # revision.read of the opened head carries the same eight fields;
        # only workspace.open may carry field 9 (snapshot).
        self.assertNotIn("snapshot", result["report"]["decoded"])
        self.assertEqual({key: value for key, value in head.items()
                          if key != "snapshot"}, result["report"]["decoded"])
        code, result = self.run_tool("budgets")
        self.assertEqual(code, 0)

    def test_propose_identical_body_validates_and_finishes(self) -> None:
        # The load-bearing loop, semantically empty: read an entity, propose
        # its own body back, validate, finish. Proves envelope assembly,
        # principal, expiry, preconditions, and server validity end to end.
        code, result = self.run_tool("inventory")
        entity = result["report"]["inventory"]["objects"][0]["entity"]
        code, read = self.run_tool("read", entity)
        self.assertEqual(code, 0)
        entry = read["report"]["decoded"]["entries"][0]
        self.assertEqual(entry["entity"], entity)
        ops = [{"class": "ReplaceEntityVersion", "kind": entry["kind"],
                "target": entity, "field_tag": None, "payload": entry["body"]}]
        code, proposed = self.run_tool("propose", json.dumps(ops))
        self.assertEqual(code, 0, json.dumps(proposed)[:400])
        self.assertTrue(proposed["report"].get("created"), json.dumps(proposed)[:400])
        self.assertTrue(proposed["report"].get("valid"), json.dumps(proposed)[:400])
        record = proposed["report"]["record"]
        stored = proposed["report"]["stored"]
        code, done = self.run_tool("finish", record)
        self.assertEqual(code, 0)
        self.assertTrue(done["report"]["finished"])
        self.assertTrue((self.ws / "final_candidate.hex").is_file())
        self.assertEqual((self.ws / "final_candidate.hex").read_text(encoding="utf-8").strip(), stored)

    def test_propose_broken_operation_never_validates(self) -> None:
        code, result = self.run_tool("inventory")
        entity = result["report"]["inventory"]["objects"][0]["entity"]
        ops = [{"class": "Nope", "kind": 1, "target": entity,
                "field_tag": None, "payload": None}]
        code, _ = self.run_tool("propose", json.dumps(ops))
        self.assertEqual(code, 2)

    def test_propose_reports_negative_decision_honestly(self) -> None:
        # A structurally encodable but semantically invalid candidate
        # (block owned by a missing function) validates to a negative
        # decision: valid False with the decision tag, never assumed
        # True from delivery, and finish refuses it unwritten. Derived
        # identities are still reported (derivation is not a claim).
        ops = [{"class": "CreateEntity", "kind": 7, "target": None, "field_tag": None,
                "payload": {"function": "ab" * 32, "operations": [], "parameters": [],
                            "reachability": "Required",
                            "terminator": {"variant": "Trap", "value": {
                                "code": "Unreachable",
                                "payload": {"variant": "None"}}}}}]
        code, proposed = self.run_tool("propose", json.dumps(ops))
        self.assertEqual(code, 0, json.dumps(proposed)[:300])
        self.assertTrue(proposed["report"].get("created"))
        self.assertFalse(proposed["report"].get("valid"))
        self.assertNotEqual(proposed["report"].get("decision", {}).get("tag"), 1)
        self.assertEqual(len(proposed["report"].get("identities", [])), 1)
        code, done = self.run_tool("finish", proposed["report"]["record"])
        self.assertEqual(code, 0)
        self.assertFalse(done["report"].get("finished"))
        self.assertFalse((self.ws / "final_candidate.hex").exists())

    def _identity_op(self) -> tuple[str, dict]:
        code, result = self.run_tool("inventory")
        self.assertEqual(code, 0)
        entity = result["report"]["inventory"]["objects"][0]["entity"]
        code, read = self.run_tool("read", entity)
        self.assertEqual(code, 0)
        entry = read["report"]["decoded"]["entries"][0]
        return entity, {"class": "ReplaceEntityVersion", "kind": entry["kind"],
                        "target": entity, "field_tag": None,
                        "payload": entry["body"]}

    def _describe(self, record_hex: str) -> dict:
        [described] = sley2_codecs.run_batch([{"op": "describe_record", "record": record_hex}])
        return described

    def test_append_concatenation_fails_closed_with_owner_code(self) -> None:
        # The server rebuilds base-operations + addition-operations and
        # re-checks contiguity: both sides must number from zero, so the
        # join always collides. The refusal keeps its owner code (never
        # a PROTOCOL_ symbol) and the accepted head does not move.
        _, op = self._identity_op()
        code, proposed = self.run_tool("propose", json.dumps([op]))
        self.assertEqual(code, 0)
        self.assertTrue(proposed["report"].get("valid"))
        code, before = self.run_tool("open")
        self.assertEqual(code, 0)
        code, appended = self.run_tool("append", proposed["report"]["record"],
                                       json.dumps([op]))
        self.assertEqual(code, 0, json.dumps(appended)[:400])
        self.assertFalse(appended["report"].get("appended"))
        body = bytes.fromhex(appended["report"]["body"])
        self.assertIn(b"MUTATION_CANDIDATE_OPERATION_ORDINAL", body)
        self.assertNotIn(b"PROTOCOL_", body)
        code, after = self.run_tool("open")
        self.assertEqual(code, 0)
        self.assertEqual(after["report"]["body"], before["report"]["body"])

    def test_append_rejects_garbage_base(self) -> None:
        _, op = self._identity_op()
        for bad in ("zz", "00" * 32, "00" * 100):
            code, _ = self.run_tool("append", bad, json.dumps([op]))
            self.assertEqual(code, 2)

    def test_append_and_compose_reject_oversize_lists(self) -> None:
        _, op = self._identity_op()
        code, proposed = self.run_tool("propose", json.dumps([op]))
        self.assertEqual(code, 0)
        base = proposed["report"]["record"]
        code, _ = self.run_tool("append", base, json.dumps([op] * 65))
        self.assertEqual(code, 2)
        code, _ = self.run_tool("compose", base, json.dumps([op] * 65))
        self.assertEqual(code, 2)

    def test_compose_reassembles_full_list_under_base_nonce(self) -> None:
        _, op = self._identity_op()
        code, proposed = self.run_tool("propose", json.dumps([op]))
        self.assertEqual(code, 0)
        base = proposed["report"]["record"]
        code, composed = self.run_tool("compose", base, json.dumps([op, op]))
        self.assertEqual(code, 0, json.dumps(composed)[:400])
        self.assertTrue(composed["report"].get("created"))
        self.assertTrue(composed["report"].get("valid"), json.dumps(composed)[:400])
        [origin] = sley2_codecs.run_batch([{"op": "describe_record", "record": base}])
        [full] = sley2_codecs.run_batch(
            [{"op": "describe_record", "record": composed["report"]["record"]}])
        self.assertEqual(full["op_count"], 2)
        self.assertEqual(full["ordinals"], [0, 1])
        self.assertEqual(full["nonce"], origin["nonce"])

    def test_compose_rejects_foreign_target(self) -> None:
        # Session binding: preconditions come from live reads against
        # this trial's head; an entity from another workspace cannot be
        # bound, and the tool refuses before the server is reached.
        _, op = self._identity_op()
        code, proposed = self.run_tool("propose", json.dumps([op]))
        base = proposed["report"]["record"]
        foreign = dict(op)
        foreign["target"] = "ab" * 32
        code, _ = self.run_tool("compose", base, json.dumps([foreign]))
        self.assertEqual(code, 2)

    def test_usage_ledger_accumulates_across_invocations(self) -> None:
        self.run_tool("inventory")
        self.run_tool("open")
        ledger = self.ws / ".sley-live-usage"
        self.assertTrue(ledger.is_file())
        first = json.loads(ledger.read_text(encoding="utf-8"))
        self.run_tool("budgets")
        second = json.loads(ledger.read_text(encoding="utf-8"))
        self.assertGreaterEqual(second["totals"]["invocations"],
                                first["totals"]["invocations"] + 1)
        self.assertGreaterEqual(second["totals"]["wall_ms"], first["totals"]["wall_ms"])

    def test_raw_refuses_outside_allowlist(self) -> None:
        # No commit, merge, execute, export/import, report, session
        # management, or workspace creation paths exist for the agent:
        # every one is refused before any server contact, including
        # commit. (workspace.open is afforded since contract revision 5.)
        for method in ("commit", "merge", "execute", "export", "import", "report",
                       "session.open", "session.close", "workspace.create",
                       "exchange.import"):
            with self.subTest(method=method):
                code, _ = self.run_tool("raw", method, "00")
                self.assertEqual(code, 2)

    def test_open_and_raw_workspace_open_are_guarded_agent_reads(self) -> None:
        # The agent's own opener is transcript-captured like any read, and
        # the same method is reachable through the guarded raw path.
        code, opened = self.run_tool("open")
        self.assertEqual(code, 0)
        code, raw = self.run_tool("raw", "workspace.open", "")
        self.assertEqual(code, 0)
        self.assertFalse(raw["report"]["failed"])
        self.assertEqual(raw["report"]["body"], opened["report"]["body"])
        code, revision = self.run_tool("revision", opened["report"]["decoded"]["tx"])
        self.assertEqual(code, 0)
        entries, reason = sley2_tool.verify_transcript_chain(
            self.ws / ".sley-live-transcript.jsonl")
        self.assertIsNone(reason, reason)
        self.assertEqual([entry["command"] for entry in entries],
                         ["open", "raw", "revision"])
        self.assertEqual(entries[0]["args"], {"opened": True})
        self.assertEqual(entries[2]["args"],
                         {"tx": opened["report"]["decoded"]["tx"]})
        agent_methods = [item.get("method") for item in entries[0]["session"]
                         if item.get("direction") == "request"]
        # Harness bookkeeping (session.open + its own head read) plus the
        # agent's guarded workspace.open.
        self.assertEqual(agent_methods.count("workspace.open"), 2)

    def test_agent_transcript_chain_verifies_and_counts(self) -> None:
        self.run_tool("inventory")
        self.run_tool("open")
        _, op = self._identity_op()
        code, proposed = self.run_tool("propose", json.dumps([op]))
        self.assertEqual(code, 0)
        chain = self.ws / ".sley-live-transcript.jsonl"
        self.assertTrue(chain.is_file())
        entries, reason = sley2_tool.verify_transcript_chain(chain)
        self.assertIsNone(reason, reason)
        self.assertEqual([entry["seq"] for entry in entries], list(range(len(entries))))
        self.assertTrue(all(entry["tool_version"] == sley2_tool.TOOL_VERSION for entry in entries))
        kinds = [entry["command"] for entry in entries]
        # _identity_op runs inventory+read; plus our 3 calls.
        self.assertEqual(kinds.count("inventory"), 2)
        self.assertIn("propose", kinds)
        usage = json.loads((self.ws / ".sley-live-usage").read_text(encoding="utf-8"))
        self.assertEqual(usage["totals"]["invocations"], len(entries))

    def test_transcript_tampering_breaks_verification(self) -> None:
        self.run_tool("open")
        chain = self.ws / ".sley-live-transcript.jsonl"
        entries, reason = sley2_tool.verify_transcript_chain(chain)
        self.assertIsNone(reason)
        lines = chain.read_text(encoding="utf-8").splitlines()
        tampered = json.loads(lines[0])
        tampered["ok"] = not tampered["ok"]
        bad = self.root / "bad.jsonl"
        bad.write_text(json.dumps(tampered) + "\n", encoding="utf-8")
        _, reason = sley2_tool.verify_transcript_chain(bad)
        self.assertIsNotNone(reason)
        gap = self.root / "gap.jsonl"
        gap.write_text("\n".join(lines[:1] + lines[2:]) + "\n" if len(lines) > 2
                       else lines[0] + "\n", encoding="utf-8")
        if len(lines) > 2:
            _, reason = sley2_tool.verify_transcript_chain(gap)
            self.assertIsNotNone(reason)
        missing = self.root / "missing.jsonl"
        missing.write_text("", encoding="utf-8")
        found, _ = sley2_tool.verify_transcript_chain(missing)
        self.assertEqual(found, [])
        _, reason = sley2_tool.verify_transcript_chain(self.root / "nope.jsonl")
        self.assertIsNotNone(reason)

    def test_continuation_attempts_are_accounted_not_silent(self) -> None:
        code, result = self.run_tool("raw", "query.root", "00")
        self.assertEqual(code, 0)
        chain = self.ws / ".sley-live-transcript.jsonl"
        entries, reason = sley2_tool.verify_transcript_chain(chain)
        self.assertIsNone(reason, reason)
        total_cont = sum(entry["summary"]["continuations"] for entry in entries)
        self.assertGreaterEqual(total_cont, 1)

    def test_agent_commands_never_touch_repo_files(self) -> None:
        import hashlib as _hashlib

        def snapshot() -> dict[str, str]:
            out = {}
            for path in sorted((self.ws / "repo").rglob("*")):
                if path.is_file() and not path.is_symlink():
                    out[str(path.relative_to(self.ws))] = _hashlib.sha256(
                        path.read_bytes()).hexdigest()
            return out

        before = snapshot()
        self.run_tool("inventory")
        # Seeding imports the base pack through the server (the only
        # writer to repo/ by construction); snapshot after the seed, then
        # prove agent commands never touch repo files again.
        before = snapshot()
        self.assertTrue(before)
        _, op = self._identity_op()
        code, proposed = self.run_tool("propose", json.dumps([op]))
        self.assertEqual(code, 0)
        code, _ = self.run_tool("finish", proposed["report"]["record"])
        self.assertEqual(code, 0)
        self.assertEqual(snapshot(), before)
        managed = {"final_candidate.hex", ".sley-live-usage", ".sley-live-transcript.jsonl",
                   ".sley-live-seed", "serve-report.json", "probe-report.json"}
        for path in self.ws.iterdir():
            if path.name in ("repo", "base.pack", ".sley-live"):
                continue
            self.assertIn(path.name, managed, path.name)


class Sley2AppendCreateTests(unittest.TestCase):
    """Create-identity continuation across appends on the TEST task pack.

    The server verifies CreateEntity targets against
    derive(workspace, nonce, kind, create-ordinal); only a correct
    continuation validates, so acceptance proves the adapter contract.
    """

    def setUp(self) -> None:
        if not (SLEY.is_file() and os.access(SLEY, os.X_OK)):
            self.skipTest("sley binary unavailable")
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        from bench.live.taskpacks import stage_initial
        from bench.live.tooling import stage_tooling

        self.ws = Path(self.temp.name) / "ws"
        stage_initial("sley_2_0", "S2B-TEST-001", self.ws)
        stage_tooling("sley_2_0", self.ws)
        manifest = json.loads((ROOT / "bench/fixtures/sley2/S2B-TEST-001/task_manifest.json").read_text())
        self.func = manifest["entities"]["func"]
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

    def _case(self, first: int, second: int) -> dict:
        sint = {"value_type": {"variant": "SInt", "value": 64},
                "data": {"variant": "SInt", "value": 0}}
        typed = dict(sint)
        typed["data"] = {"variant": "SInt", "value": second}
        head = dict(sint)
        head["data"] = {"variant": "SInt", "value": first}
        err_t = {"variant": "BuiltinFailure", "value": "ArithmeticError"}
        expected = {"value_type": {"variant": "Result", "value": {
            "ok": {"variant": "SInt", "value": 64}, "error": err_t}},
            "data": {"variant": "Result", "value": {"variant": "Ok", "value": head}}}
        return {"class": "CreateEntity", "kind": 14, "target": None, "field_tag": None,
                "payload": {"target": self.func, "inputs": [head, typed],
                            "effect_environment": {"variant": "Replay", "value": []},
                            "expected": {"variant": "Value", "value": expected},
                            "observations": [],
                            "resource_limits": {"fuel": 1000000, "memory_bytes": 1000000,
                                                "output_bytes": 1000000, "effect_count": 1000,
                                                "call_depth": 64, "wall_timeout_millis": 60000}}}

    def test_compose_preserves_create_identities_byte_for_byte(self) -> None:
        first = self._case(7, 2)
        code, proposed = self.run_tool("propose", json.dumps([first]))
        self.assertEqual(code, 0, json.dumps(proposed)[:400])
        self.assertTrue(proposed["report"].get("valid"))
        base = proposed["report"]["record"]
        [origin] = sley2_codecs.run_batch([{"op": "describe_record", "record": base}])
        code, composed = self.run_tool(
            "compose", base, json.dumps([self._case(7, 2), self._case(7, 3)]))
        self.assertEqual(code, 0, json.dumps(composed)[:400])
        self.assertTrue(composed["report"].get("created"))
        self.assertTrue(composed["report"].get("valid"), json.dumps(composed)[:400])
        [full] = sley2_codecs.run_batch(
            [{"op": "describe_record", "record": composed["report"]["record"]}])
        self.assertEqual(full["op_count"], 2)
        self.assertEqual(full["classes"], ["CreateEntity", "CreateEntity"])
        self.assertEqual(full["nonce"], origin["nonce"])
        # The resupplied first create re-derives the identical identity:
        # preservation, not silent change.
        self.assertEqual(full["targets"][0], origin["targets"][0])
        self.assertNotEqual(full["targets"][1], origin["targets"][0])


if __name__ == "__main__":
    unittest.main()
