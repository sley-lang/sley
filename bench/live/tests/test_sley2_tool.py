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

from bench.live import sley2_codecs, sley2_tool


ROOT = Path(__file__).resolve().parents[3]
SLEY = Path(os.environ.get("SLEY2_SLEY_BINARY", "/home/gfarch/Work/target-sley2-succ/debug/sley"))
PACK_HEX = json.loads((ROOT / "conformance/repository-exchange/v1/accepted.json").read_text())["vectors"][0]["exchange_hex"]


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
        (self.ws / "base.pack").write_bytes(bytes.fromhex(PACK_HEX))
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
        code, _ = self.run_tool("revision")
        self.assertEqual(code, 2)
        (self.ws / "base.pack").write_bytes(bytes.fromhex(PACK_HEX))
        with mock.patch.dict(os.environ, {}, clear=True):
            code, _ = self.run_tool("revision")
            self.assertEqual(code, 2)

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
        code, result = self.run_tool("revision")
        self.assertEqual(code, 0)
        self.assertFalse(result["report"]["failed"])
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


if __name__ == "__main__":
    unittest.main()
