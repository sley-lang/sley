"""Unit regressions for the mediated access-evidence audit.

Builds synthetic runner-owned captures (start/exchanges/completion
with recomputed seals) and drives ``_audit_mediated_access`` directly:
binding mismatches, broken chains, inconsistent continuations,
truncation without continuation, whole-store reads, commit paths,
and budget overruns must all fail closed, and only a clean bounded
capture may pass. End-to-end mediated proof (real capture through
execute_attempt into the real oracle) lives in the context_pos
campaign-path tests.
"""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path

from bench.fixtures import sley2_live_judge as judge

ROOT = Path(__file__).resolve().parents[3]
TASK_DIR = ROOT / "bench" / "fixtures" / "sley2" / "S2B-CONTEXT-001"

NEEDS_BINARY = bool(os.environ.get("SLEY2_SLEY_BINARY", ""))


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"),
                      ensure_ascii=True).encode("utf-8")


def seal(record: dict, previous: str) -> dict:
    body = {key: value for key, value in record.items()
            if key not in ("prev", "hash")}
    digest = hashlib.sha256(
        (previous + canonical(body).decode("ascii")).encode("ascii")
    ).hexdigest()
    sealed = dict(record)
    sealed["prev"] = previous
    sealed["hash"] = digest
    return sealed


class CaptureBuilder:
    def __init__(self, root: Path, final: bytes = b"final-bytes") -> None:
        self.root = root
        self.attempt = "run1.sley_2_0.s2b-context-001.7"
        pack = (TASK_DIR / "base.pack").read_bytes()
        manifest = (TASK_DIR / "task_manifest.json").read_bytes()
        binary = Path(os.environ["SLEY2_SLEY_BINARY"]).read_bytes()
        self.frozen = {
            "pack_sha256": hashlib.sha256(pack).hexdigest(),
            "task_manifest_sha256": hashlib.sha256(manifest).hexdigest(),
            "tool_version": "1",
            "binary_sha256": hashlib.sha256(binary).hexdigest(),
        }
        (root / "start.json").write_bytes(canonical({
            "contract": "sley2.trusted-capture.v1",
            "attempt_id": self.attempt,
            "frozen": self.frozen,
            "caps": {},
            "created_utc": "2026-09-21T00:00:00Z",
            "creator_pid": 1,
        }) + b"\n")
        self.head = "0" * 64
        self.seq = 0
        self.lines: list[str] = []
        self.final = final

    def exchange(self, method: str, session: str = "s",
                 failed: bool = False, omitted: int = 0,
                 truncated: bool = False, continued: bool = False,
                 response_bytes: int = 100) -> None:
        req = seal({"contract": "sley2.trusted-capture.v1",
                    "attempt_id": self.attempt, "kind": "request",
                    "seq": self.seq, "phase": "read",
                    "session_id": session, "method": method,
                    "request_sha256": "ab" * 32, "request_bytes": 10,
                    "t_utc": "2026-09-21T00:00:00Z"}, self.head)
        self.lines.append(canonical(req).decode("ascii"))
        self.head = req["hash"]
        resp = seal({"contract": "sley2.trusted-capture.v1",
                     "attempt_id": self.attempt, "kind": "response",
                     "seq": self.seq, "phase": "read",
                     "session_id": session, "method": method,
                     "response_sha256": "cd" * 32,
                     "response_bytes": response_bytes, "failed": failed,
                     "omitted": omitted, "truncated": truncated,
                     "continued": continued, "wall_ms": 1,
                     "t_utc": "2026-09-21T00:00:00Z"}, self.head)
        self.lines.append(canonical(resp).decode("ascii"))
        self.head = resp["hash"]
        self.seq += 1

    def close(self, trial_ws: Path) -> Path:
        (self.root / "exchanges.jsonl").write_text(
            "\n".join(self.lines) + "\n", encoding="utf-8")
        (trial_ws / "final_candidate.hex").write_bytes(self.final)
        (self.root / "completion.json").write_bytes(canonical({
            "contract": "sley2.trusted-capture-completion.v1",
            "attempt_id": self.attempt,
            "final_sha256": hashlib.sha256(self.final).hexdigest(),
            "final_bytes": len(self.final),
            "exchanges": self.seq,
            "chain_head": self.head,
            "totals": {},
            "sessions": ["s"],
            "phases": ["read"],
            "t_utc": "2026-09-21T00:00:00Z",
        }) + b"\n")
        return self.root


@unittest.skipUnless(NEEDS_BINARY, "needs SLEY2_SLEY_BINARY")
class MediatedAccessCase(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.capture = self.root / "capture"
        self.capture.mkdir()
        self.trial = self.root / "trial"
        self.trial.mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def build(self, final: bytes = b"final-bytes") -> tuple[Path, CaptureBuilder]:
        builder = CaptureBuilder(self.capture, final)
        return builder.close(self.trial), builder

    def test_clean_bounded_capture_passes(self) -> None:
        path, builder = self.build()
        builder.exchange("revision")
        builder.exchange("caps")
        builder.exchange("read")
        builder.close(self.trial)
        access = judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(access["whole_store_reads"], 0)
        self.assertEqual(access["agent_requests"], 3)
        self.assertEqual(access["refusals"], 0)

    def test_truncated_page_with_continuation_passes(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True)
        builder.exchange("raw:query.continue")
        builder.close(self.trial)
        access = judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(access["continuations"], 1)
        self.assertEqual(access["bounded_reads"], 2)

    def test_continue_without_truncation_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.continue")
        builder.close(self.trial)
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_truncated_without_continuation_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True)
        builder.close(self.trial)
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_omitted_without_continuation_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.restricted", omitted=5)
        builder.close(self.trial)
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_whole_store_read_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("inventory")
        builder.close(self.trial)
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_commit_path_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("commit")
        builder.close(self.trial)
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_hidden_truncation_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("read", truncated=True)
        builder.close(self.trial)
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_cumulative_over_budget_rejects(self) -> None:
        path, builder = self.build()
        for _ in range(5):
            builder.exchange("read", response_bytes=1048576)
        builder.close(self.trial)
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_missing_capture_rejects(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(
                self.root / "absent", TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_tampered_hash_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("read")
        builder.close(self.trial)
        lines = (path / "exchanges.jsonl").read_text(
            encoding="utf-8").splitlines()
        record = json.loads(lines[1])
        record["response_bytes"] = 101
        lines[1] = canonical(record).decode("ascii")
        (path / "exchanges.jsonl").write_text(
            "\n".join(lines) + "\n", encoding="utf-8")
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_fixture_binding_mismatch_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("read")
        builder.close(self.trial)
        start = json.loads((path / "start.json").read_text(
            encoding="utf-8"))
        start["frozen"]["pack_sha256"] = "00" * 64
        (path / "start.json").write_text(
            canonical(start).decode("ascii"), encoding="utf-8")
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_final_binding_mismatch_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("read")
        builder.close(self.trial)
        (self.trial / "final_candidate.hex").write_bytes(b"other-bytes")
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")


if __name__ == "__main__":
    unittest.main()
