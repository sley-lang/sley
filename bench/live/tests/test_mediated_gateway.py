"""Unit tests for the mediated sley_2_0 gateway (binary-gated).

The gateway executes the real sley2_tool Session machinery on the
runner-owned protected workspace; every call crosses
TrustedCapture.exchange. Denials are recorded responses (failed),
continuations flow from the same session summary as the unmediated
tool, and adjudication requires BOTH a reconciled capture AND the
trusted oracle verdict.
"""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]

SLEY = Path(os.environ.get(
    "SLEY2_SLEY_BINARY",
    "/home/gfarch/Work/checkpoints/sley2-cargo-target/debug/sley"))

TASK_ID = "S2B-TYPE-001"

from bench.live import mediated_sley as gw  # noqa: E402
from bench.live import trusted_capture as tc  # noqa: E402
from bench.live.taskpacks import stage_initial  # noqa: E402


FROZEN = {
    "pack_sha256": "a" * 64,
    "task_manifest_sha256": "b" * 64,
    "tool_version": "1",
    "binary_sha256": "c" * 64,
}


def ok_handler(payload: bytes = b"r"):
    def run() -> tuple[bytes, dict]:
        return payload, {"failed": False}
    return run


class MediatedGatewayTests(unittest.TestCase):
    def setUp(self) -> None:
        if not (SLEY.is_file() and os.access(SLEY, os.X_OK)):
            self.skipTest("sley binary unavailable")
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.protected_ws = self.root / "protected" / "ws"
        stage_initial("sley_2_0", TASK_ID, self.protected_ws)
        self.capture_dir = self.root / "capture"
        self.cap = tc.TrustedCapture.create(
            self.capture_dir, attempt_id="test.gw.0", frozen=FROZEN)
        self.endpoint = gw.MediatedSleyEndpoint(SLEY, self.protected_ws,
                                                self.cap)

    def test_allowed_read_round_trip_captured(self) -> None:
        raw = self.endpoint.handle("read", "s1", "open", [])
        envelope = json.loads(raw)
        self.assertTrue(envelope.get("ok"), envelope)
        self.assertEqual(self.cap.totals["exchanges"], 1)
        self.assertEqual(self.cap.totals["failed"], 0)
        # revision takes the tx the agent's own open reported.
        tx = envelope["report"]["decoded"]["tx"]
        raw = self.endpoint.handle("read", "s1", "revision", [tx])
        envelope = json.loads(raw)
        self.assertTrue(envelope.get("ok"), envelope)
        self.assertEqual(self.cap.totals["exchanges"], 2)
        self.assertEqual(self.cap.totals["failed"], 0)

    def test_revision_without_tx_is_a_recorded_refusal(self) -> None:
        # The no-argument form has no harness-supplied head to fall back
        # on: it is refused as a recorded, counted failed response.
        raw = self.endpoint.handle("read", "s1", "revision", [])
        envelope = json.loads(raw)
        self.assertFalse(envelope.get("ok"))
        self.assertIn("LIVE_SLEY2_TOOL_INVALID", envelope.get("detail", ""))
        self.assertEqual(self.cap.totals["exchanges"], 1)
        self.assertEqual(self.cap.totals["failed"], 1)

    def test_denied_server_method_recorded_as_failed(self) -> None:
        # `commit` is outside the frozen allowlist: the server refuses
        # through the normal failed-envelope path (recorded, counted).
        raw = self.endpoint.handle("read", "s1", "raw", ["commit", "00"])
        envelope = json.loads(raw)
        self.assertFalse(envelope.get("ok"))
        self.assertEqual(self.cap.totals["failed"], 1)
        result = tc.reconcile(self.capture_dir, b"f")
        # No completion yet: incomplete, but the refusal is captured.
        self.assertFalse(result["reconciled"])
        self.assertEqual(result["code"], "CAPTURE_COMPLETION_MISSING")

    def test_denied_gateway_command_recorded(self) -> None:
        # `commit` is outside the gateway surface: the denial is an
        # ordinary failed response (captured, counted), never an
        # unrecorded path.
        raw = self.endpoint.handle("read", "s1", "commit", ["00"])
        envelope = json.loads(raw)
        self.assertFalse(envelope.get("ok"))
        self.assertEqual(envelope.get("error"), "GATEWAY_COMMAND_DENIED")
        self.assertEqual(self.cap.totals["failed"], 1)
        self.cap.complete(b"f")
        result = tc.reconcile(self.capture_dir, b"f")
        self.assertTrue(result["reconciled"], result)

    def test_resolve_derives_mechanically(self) -> None:
        from bench.live import sley2_codecs
        raw = self.endpoint.handle("compose", "s1", "propose",
                                   [json.dumps([{
                                       "class": "CreateEntity", "kind": 4,
                                       "target": None, "field_tag": None,
                                       "payload": {
                                           "type_parameters": [],
                                           "form": {"variant": "Variant",
                                                    "value": []},
                                           "invariants": [],
                                           "visibility": "Private"}}])])
        envelope = json.loads(raw)
        self.assertTrue(envelope.get("ok"), envelope)
        base_record = envelope["report"]["record"]
        raw2 = self.endpoint.handle(
            "compose", "s1", "resolve",
            [base_record, json.dumps([4, 7, 7])])
        resolved = json.loads(raw2)
        self.assertTrue(resolved.get("ok"), resolved)
        self.assertEqual(len(resolved["report"]["ids"]), 3)
        # Same derivation through the trusted codecs directly.
        [described] = sley2_codecs.run_batch(
            [{"op": "describe_record", "record": base_record}])
        self.assertEqual(resolved["report"]["nonce"], described["nonce"])

    def test_adjudicate_accepted(self) -> None:
        self.endpoint.handle("read", "s1", "open", [])
        final = b"held-final"
        self.cap.complete(final)
        status, code = gw.adjudicate(self.capture_dir, final,
                                     {"status": "accepted", "code": None})
        self.assertEqual((status, code), ("accepted", None))

    def test_adjudicate_rejected(self) -> None:
        self.endpoint.handle("read", "s1", "open", [])
        final = b"held-final"
        self.cap.complete(final)
        status, code = gw.adjudicate(self.capture_dir, final,
                                     {"status": "rejected",
                                      "code": "ORACLE_X"})
        self.assertEqual((status, code), ("rejected", "ORACLE_X"))

    def test_adjudicate_unreconciled_never_accepts(self) -> None:
        self.endpoint.handle("read", "s1", "open", [])
        # No completion: reconcile fails; even an accepted oracle
        # cannot rehabilitate.
        status, code = gw.adjudicate(self.capture_dir, b"held-final",
                                     {"status": "accepted", "code": None})
        self.assertEqual(status, "harness_failure")
        self.assertEqual(code, "CAPTURE_COMPLETION_MISSING")

    def test_adjudicate_no_final_never_accepts(self) -> None:
        self.endpoint.handle("read", "s1", "open", [])
        self.cap.complete(b"no-final-bytes")
        status, code = gw.adjudicate(self.capture_dir, None,
                                     {"status": "accepted", "code": None})
        self.assertEqual(status, "harness_failure")
        self.assertEqual(code, "CAPTURE_GATE_NO_FINAL")

    def test_adjudicate_final_mismatch(self) -> None:
        self.endpoint.handle("read", "s1", "open", [])
        self.cap.complete(b"real-final")
        status, code = gw.adjudicate(self.capture_dir, b"other-final",
                                     {"status": "accepted", "code": None})
        self.assertEqual(status, "harness_failure")
        self.assertEqual(code, "CAPTURE_FINAL_MISMATCH")


if __name__ == "__main__":
    unittest.main()
