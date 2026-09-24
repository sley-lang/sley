"""Unit tests for runner-owned trusted capture (no binary needed).

Archival integrity, capture completeness, crash durability, and
access control are evidenced separately:
- archival integrity: ArtifactStore create-only semantics (existing
  test_artifacts.py) — retained as archival infrastructure, never
  the authoritative source here.
- capture completeness: every request/response pair chained here;
  gaps, orphans, and torn suffixes reject.
- crash durability: fsync file + directory per record; a torn
  suffix preserves the prefix as failure evidence.
- access control: confined-process probes (test_confined_capture.py)
  inspect actual open() outcomes; candidate-side files are never
  consulted by reconcile().
"""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path

from bench.live import trusted_capture as tc


FROZEN = {
    "pack_sha256": "a" * 64,
    "task_manifest_sha256": "b" * 64,
    "tool_version": "1",
    "binary_sha256": "c" * 64,
}


def ok_handler(payload: bytes = b"resp", **usage: object):
    def run() -> tuple[bytes, dict]:
        meta: dict = {"failed": False}
        meta.update(usage)
        return payload, meta
    return run


def fail_handler() -> tuple[bytes, dict]:
    return b"refused", {"failed": True}


class TrustedCaptureTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def make(self, name: str = "cap",
             caps: dict | None = None) -> tc.TrustedCapture:
        return tc.TrustedCapture.create(self.root / name,
                                        attempt_id="run1.arm.task.0",
                                        frozen=FROZEN, caps=caps)

    def test_happy_path_reconciles(self) -> None:
        cap = self.make()
        out = cap.exchange(phase="read", session_id="s1", method="revision",
                           request=b"{}", handler=ok_handler(b"r1"))
        self.assertEqual(out, b"r1")
        cap.exchange(phase="compose", session_id="s1", method="propose",
                     request=b"{}", handler=ok_handler(b"r2", continued=True,
                                                       omitted=3))
        cap.exchange(phase="finish", session_id="s2", method="finish",
                     request=b"{}", handler=fail_handler())
        final = b"final-bytes"
        completion = cap.complete(final)
        self.assertEqual(completion["exchanges"], 3)
        result = tc.reconcile(self.root / "cap", final)
        self.assertTrue(result["reconciled"], result)
        self.assertEqual(result["code"], "CAPTURE_RECONCILED")
        self.assertEqual(result["complete_exchanges"], 3)
        self.assertEqual(result["totals"]["exchanges"], 3)
        self.assertEqual(result["totals"]["failed"], 1)
        self.assertEqual(result["totals"]["continuations"], 1)
        self.assertEqual(result["totals"]["omitted"], 3)
        self.assertEqual(sorted(result["sessions"]), ["s1", "s2"])
        self.assertEqual(sorted(result["phases"]),
                         ["compose", "finish", "read"])

    def test_handler_exception_recorded_as_failed_response(self) -> None:
        cap = self.make()

        def boom() -> tuple[bytes, dict]:
            raise RuntimeError("protected op failed")

        out = cap.exchange(phase="read", session_id="s1", method="read",
                           request=b"{}", handler=boom)
        reply = json.loads(out)
        self.assertFalse(reply["ok"])
        final = b"f"
        cap.complete(final)
        result = tc.reconcile(self.root / "cap", final)
        self.assertTrue(result["reconciled"], result)
        self.assertEqual(result["totals"]["failed"], 1)

    def test_capture_error_leaves_orphan_and_rejects(self) -> None:
        cap = self.make()

        def storage_boom() -> tuple[bytes, dict]:
            raise tc.CaptureError("CAPTURE_STORAGE_INVALID: simulated")

        with self.assertRaises(tc.CaptureError):
            cap.exchange(phase="read", session_id="s1", method="read",
                         request=b"{}", handler=storage_boom)
        # The request is captured but no response exists: orphan.
        result = tc.reconcile(self.root / "cap", b"f")
        self.assertFalse(result["reconciled"])
        self.assertEqual(result["code"], "CAPTURE_ORPHAN_REQUEST")

    def test_torn_suffix_preserves_prefix_and_rejects(self) -> None:
        cap = self.make()
        cap.exchange(phase="read", session_id="s1", method="a",
                     request=b"{}", handler=ok_handler(b"r1"))
        cap.exchange(phase="read", session_id="s1", method="b",
                     request=b"{}", handler=ok_handler(b"r2"))
        cap.complete(b"f")
        path = self.root / "cap" / "exchanges.jsonl"
        raw = path.read_bytes()
        # Simulate a crash mid-append: extra bytes with no trailing
        # newline after two complete exchanges.
        self.assertTrue(raw.endswith(b"\n"))
        path.write_bytes(raw + b'{"contract": "sley2.trusted-capture')
        result = tc.reconcile(self.root / "cap", b"f")
        self.assertFalse(result["reconciled"])
        self.assertEqual(result["code"], "CAPTURE_TORN_SUFFIX")
        self.assertEqual(result["complete_exchanges"], 2)

    def test_missing_completion_rejects(self) -> None:
        cap = self.make()
        cap.exchange(phase="read", session_id="s1", method="a",
                     request=b"{}", handler=ok_handler())
        result = tc.reconcile(self.root / "cap", b"f")
        self.assertFalse(result["reconciled"])
        self.assertEqual(result["code"], "CAPTURE_COMPLETION_MISSING")

    def test_final_mismatch_rejects(self) -> None:
        cap = self.make()
        cap.exchange(phase="read", session_id="s1", method="a",
                     request=b"{}", handler=ok_handler())
        cap.complete(b"real-final")
        result = tc.reconcile(self.root / "cap", b"tampered-final")
        self.assertFalse(result["reconciled"])
        self.assertEqual(result["code"], "CAPTURE_FINAL_MISMATCH")

    def test_missing_start_rejects(self) -> None:
        result = tc.reconcile(self.root / "nope", b"f")
        self.assertFalse(result["reconciled"])
        self.assertEqual(result["code"], "CAPTURE_MISSING_START")

    def test_tampered_record_rejects(self) -> None:
        cap = self.make()
        cap.exchange(phase="read", session_id="s1", method="a",
                     request=b"{}", handler=ok_handler())
        cap.complete(b"f")
        path = self.root / "cap" / "exchanges.jsonl"
        lines = path.read_text(encoding="utf-8").splitlines()
        entry = json.loads(lines[1])
        entry["response_bytes"] = 999999
        lines[1] = json.dumps(entry, sort_keys=True)
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        result = tc.reconcile(self.root / "cap", b"f")
        self.assertFalse(result["reconciled"])
        self.assertEqual(result["code"], "CAPTURE_CHAIN_BROKEN")

    def test_deleted_response_rejects_as_orphan(self) -> None:
        cap = self.make()
        cap.exchange(phase="read", session_id="s1", method="a",
                     request=b"{}", handler=ok_handler())
        cap.exchange(phase="read", session_id="s1", method="b",
                     request=b"{}", handler=ok_handler())
        cap.complete(b"f")
        path = self.root / "cap" / "exchanges.jsonl"
        lines = path.read_text(encoding="utf-8").splitlines()
        # Delete the second exchange's response (line 3 of 4).
        self.assertEqual(len(lines), 4)
        path.write_text("\n".join([lines[0], lines[1], lines[2]]) + "\n",
                        encoding="utf-8")
        result = tc.reconcile(self.root / "cap", b"f")
        self.assertFalse(result["reconciled"])
        self.assertIn(result["code"],
                      ("CAPTURE_ORPHAN_REQUEST", "CAPTURE_COMPLETION_INVALID",
                       "CAPTURE_TOTALS_MISMATCH"))

    def test_budget_corrupt_never_resets_to_zero(self) -> None:
        cap = self.make()
        cap.exchange(phase="read", session_id="s1", method="a",
                     request=b"xx", handler=ok_handler(b"yy" * 10))
        ledger_path = self.root / "cap" / "budgets.json"
        raw_before = ledger_path.read_bytes()
        before = json.loads(raw_before)["totals"]
        self.assertGreater(before["response_bytes"], 0)
        ledger_path.write_text('{"totals": {"exchanges": "lots"}}',
                               encoding="utf-8")
        with self.assertRaises(tc.CaptureError) as raised:
            tc.BudgetLedger.resume(ledger_path)
        self.assertIn("CAPTURE_BUDGET_CORRUPT", str(raised.exception))
        # Raw bytes preserved, not zeroed.
        self.assertEqual(ledger_path.read_bytes(),
                         b'{"totals": {"exchanges": "lots"}}')
        result = tc.reconcile(self.root / "cap", b"f")
        self.assertFalse(result["reconciled"])
        self.assertEqual(result["code"], "CAPTURE_BUDGET_CORRUPT")

    def test_budget_missing_fails_closed(self) -> None:
        cap = self.make()
        cap.exchange(phase="read", session_id="s1", method="a",
                     request=b"{}", handler=ok_handler())
        os.unlink(self.root / "cap" / "budgets.json")
        result = tc.reconcile(self.root / "cap", b"f")
        self.assertFalse(result["reconciled"])
        self.assertEqual(result["code"], "CAPTURE_BUDGET_MISSING")

    def test_per_response_cap_refusal_recorded_and_counted(self) -> None:
        cap = self.make(caps={"per_response_max_bytes": 8})
        with self.assertRaises(tc.CaptureError) as raised:
            cap.exchange(phase="read", session_id="s1", method="a",
                         request=b"{}", handler=ok_handler(b"x" * 64))
        self.assertIn("CAPTURE_RESPONSE_OVER_CAP", str(raised.exception))
        totals = tc.BudgetLedger.resume(
            self.root / "cap" / "budgets.json").totals
        # The over-cap response is retained evidence (bytes counted)
        # and the refusal is counted separately.
        self.assertEqual(totals["response_bytes"], 64)
        self.assertEqual(totals["refused"], 1)

    def test_trial_budget_exhaustion_recorded(self) -> None:
        cap = self.make(caps={"trial_max_response_bytes": 10})
        cap.exchange(phase="read", session_id="s1", method="a",
                     request=b"{}", handler=ok_handler(b"12345"))
        with self.assertRaises(tc.CaptureError) as raised:
            cap.exchange(phase="read", session_id="s1", method="b",
                         request=b"{}", handler=ok_handler(b"123456"))
        self.assertIn("CAPTURE_TRIAL_BUDGET_EXCEEDED", str(raised.exception))

    def test_budgets_accumulate_across_sessions_phases(self) -> None:
        cap = self.make()
        for session_id, phase in (("s1", "read"), ("s1", "compose"),
                                  ("s2", "compose"), ("s2", "finish")):
            cap.exchange(phase=phase, session_id=session_id, method="m",
                         request=b"q",
                         handler=ok_handler(b"r" * 7, continued=True,
                                            omitted=2, truncated=True))
        totals = cap.totals
        self.assertEqual(totals["exchanges"], 4)
        self.assertEqual(totals["response_bytes"], 28)
        self.assertEqual(totals["continuations"], 4)
        self.assertEqual(totals["omitted"], 8)
        self.assertEqual(totals["truncated"], 4)
        cap.complete(b"f")
        result = tc.reconcile(self.root / "cap", b"f")
        self.assertTrue(result["reconciled"], result)

    def test_start_rejects_weak_frozen_bindings(self) -> None:
        weak = dict(FROZEN)
        weak["binary_sha256"] = ""
        with self.assertRaises(tc.CaptureError):
            tc.TrustedCapture.create(self.root / "weak",
                                     attempt_id="a", frozen=weak)

    def test_capture_dir_mode(self) -> None:
        cap = self.make()
        mode = os.stat(cap.directory).st_mode & 0o777
        self.assertEqual(mode, 0o700)


if __name__ == "__main__":
    unittest.main()
