"""Unit regressions for the mediated access-evidence audit.

Builds synthetic runner-owned captures (start/exchanges/completion
with recomputed seals) and drives ``_audit_mediated_access`` directly:
binding mismatches, broken chains, inconsistent continuations,
truncation without continuation, whole-store reads, commit paths,
and budget overruns must all fail closed, and only a clean bounded
capture may pass. End-to-end mediated proof (real capture through
execute_attempt into the real oracle) lives in the CONTEXT
campaign-path tests (``test_mediated_context.py``, stand-in sequence
``context MODE``).
"""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path

from bench.fixtures import sley2_live_judge as judge
from bench.live.sley2_tool import TOOL_VERSION

Q = "a1" * 32
Q2 = "b2" * 32


def cur(n: int) -> str:
    return "1:" + f"{n:02x}" * 32


def chain(query: str = Q, after=None, truncated: bool = False, nxt=None) -> dict:
    return {"query": query, "after": after, "truncated": truncated,
            "next": nxt}

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
            "tool_version": TOOL_VERSION,
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
                 response_bytes: int = 100, chain: dict | None = None) -> None:
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
                     "t_utc": "2026-09-21T00:00:00Z",
                     **({"chain": chain} if chain is not None else {})},
                    self.head)
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

    def rejects(self, builder, needle: str | None = None) -> None:
        builder.close(self.trial)
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_mediated_access(self.capture, TASK_DIR, self.trial)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")
        if needle is not None:
            self.assertIn(needle, raised.exception.detail)

    def test_truncated_page_with_continuation_passes(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=5,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", omitted=5,
                         chain=chain(after=cur(1)))
        builder.close(self.trial)
        access = judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(access["continuations"], 1)
        self.assertEqual(access["bounded_reads"], 2)

    def test_multi_page_continuation_chain_passes(self) -> None:
        # Each truncated page opens its own next cursor; each continue
        # discharges exactly the page whose next cursor it carries.
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=6,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", truncated=True, omitted=6,
                         chain=chain(after=cur(1), truncated=True, nxt=cur(2)))
        # Final page: not truncated; `omitted` still counts the entities
        # earlier pages returned (server accounting).
        builder.exchange("raw:query.continue", omitted=6,
                         chain=chain(after=cur(2)))
        builder.close(self.trial)
        access = judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(access["continuations"], 2)
        self.assertEqual(access["bounded_reads"], 3)

    def test_cross_scope_bound_continuation_passes(self) -> None:
        # Binding is by query and cursor, not by session: a one-shot
        # invocation (its own scope) may continue a page another opened.
        path, builder = self.build()
        builder.exchange("raw:query.root", session="shim-1", truncated=True,
                         omitted=4, chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", session="shim-2", omitted=4,
                         chain=chain(after=cur(1)))
        builder.close(self.trial)
        access = judge._audit_mediated_access(path, TASK_DIR, self.trial)
        self.assertEqual(access["continuations"], 1)

    def test_cross_scope_wrong_cursor_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", session="shim-1", truncated=True,
                         omitted=4, chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", session="shim-2", omitted=4,
                         chain=chain(after=cur(2)))
        self.rejects(builder, "inconsistent continuation")

    def test_truncated_continuation_without_follow_rejects(self) -> None:
        # Stopping on a truncated continuation page is hidden truncation.
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=6,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", truncated=True, omitted=3,
                         chain=chain(after=cur(1), truncated=True, nxt=cur(2)))
        self.rejects(builder, "without continuation")

    def test_refused_continue_does_not_discharge(self) -> None:
        # Vulcan r5 probe (a): a refused continue closes nothing.
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=4,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", failed=True)
        self.rejects(builder, "without continuation")

    def test_refused_continue_after_truncated_continue_does_not_discharge(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=6,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", truncated=True, omitted=6,
                         chain=chain(after=cur(1), truncated=True, nxt=cur(2)))
        builder.exchange("raw:query.continue", failed=True)
        self.rejects(builder, "without continuation")

    def test_forged_continue_label_rejects(self) -> None:
        # Vulcan r5 probe (b): a label outside the closed vocabulary (an
        # agent-chosen command string) is not evidence of a routed call.
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=4,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue:forged", failed=True)
        self.rejects(builder, "capture label")

    def test_denied_label_does_not_discharge(self) -> None:
        # A denied frame (the gateway's fixed label) continues nothing.
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=4,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("denied", failed=True)
        self.rejects(builder, "without continuation")

    def test_past_the_end_cursor_rejects(self) -> None:
        # Vulcan r5 probe (c): the server answers an `ff..ff` cursor with a
        # successful empty page; it matches no open page.
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=4,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", omitted=4,
                         chain=chain(after="1:" + "ff" * 32))
        self.rejects(builder, "inconsistent continuation")

    def test_skipping_cursor_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=6,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", omitted=6,
                         chain=chain(after=cur(2)))
        self.rejects(builder, "inconsistent continuation")

    def test_other_query_continue_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=4,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", omitted=4,
                         chain=chain(query=Q2, after=cur(1)))
        self.rejects(builder, "inconsistent continuation")

    def test_two_truncated_pages_one_continue_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=4,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.root", truncated=True, omitted=4,
                         chain=chain(query=Q2, truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", omitted=4,
                         chain=chain(after=cur(1)))
        self.rejects(builder, "without continuation")

    def test_unbound_continue_rejects(self) -> None:
        # A continue with no binding (unparseable body) continues nothing.
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=4,
                         chain=chain(truncated=True, nxt=cur(1)))
        builder.exchange("raw:query.continue", omitted=4)
        self.rejects(builder, "inconsistent continuation")

    def test_binding_flag_mismatch_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True, omitted=4,
                         chain=chain(truncated=False))
        self.rejects(builder, "continuation binding")

    def test_prior_tool_version_rejects(self) -> None:
        # The frozen start binding names the tool boundary version; the
        # revision 4 identity ("1") is not the revision 5 surface.
        path, builder = self.build()
        builder.frozen["tool_version"] = "1"
        (path / "start.json").write_bytes(canonical({
            "contract": "sley2.trusted-capture.v1",
            "attempt_id": builder.attempt, "frozen": builder.frozen,
            "caps": {}, "created_utc": "2026-09-21T00:00:00Z",
            "creator_pid": 1}) + b"\n")
        builder.exchange("read")
        self.rejects(builder, "tool version mismatch")

    def test_continue_without_truncation_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.continue", chain=chain(after=cur(1)))
        self.rejects(builder, "inconsistent continuation")

    def test_truncated_without_continuation_rejects(self) -> None:
        path, builder = self.build()
        builder.exchange("raw:query.root", truncated=True,
                         chain=chain(truncated=True, nxt=cur(1)))
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
