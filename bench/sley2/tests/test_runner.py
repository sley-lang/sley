"""Offline tests for the S20-620 runner: trace chain, guard, metrics, claims, and a fake endpoint."""

from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from bench.raw.runner import PLAN_PATH, canonical_json_bytes, manifest_digest, write_run_manifest
from bench.sley2.runner import (
    ARM,
    CLAIM_CONTRACT,
    HANDLE_SURFACE,
    EndpointHandle,
    ScriptedAgent,
    ScriptedOracle,
    Sley2ErrorCode,
    Sley2RunnerError,
    Trace,
    append_trace_record,
    append_trial_claim,
    derive_trace_metrics,
    handle_surface,
    request_frame,
    run_scripted_trial,
    smoke_manifest,
    verify_trace,
    verify_trial_claims,
)


def digest(byte: int) -> str:
    return f"{byte:02x}" * 32


class Clock:
    def __init__(self) -> None:
        self.tick = 0

    def now_utc(self) -> str:
        self.tick += 1
        return f"2026-09-03T12:00:{self.tick:02d}Z"


class FakeEndpoint:
    """Answers like the endpoint: hello, then one response per request, failing unknown methods."""

    def __init__(self, repository: Path, report: Path) -> None:
        self.report = report
        self.frames: list[dict] = []
        self.session = "ab" * 32

    def send(self, frame: dict) -> list[dict]:
        self.frames.append(frame)
        if frame["kind"] == "hello":
            return [dict(frame)]
        failed = frame["method"] in {"candidate.validate"}
        body = self.session if frame["method"] == "session.open" else "0102"
        bounds = dict(frame["bounds"])
        bounds["returned_entities"] = 3 if frame["method"] == "handle.expand" else 0
        bounds["returned_edges"] = 2 if frame["method"] == "handle.expand" else 0
        response = {
            "body": body,
            "bounds": bounds,
            "flags": {"cancel": False, "failed": failed, "stream": False},
            "kind": "response",
            "method": frame["method"],
            "protocol_version": 1,
            "request_id": frame["request_id"],
            "session": frame["session"],
        }
        if frame["method"] == "capsule":
            return [{**response, "kind": "event", "body": "aa" * 8}, response]
        return [response]

    def close(self) -> tuple[int, str]:
        self.report.write_text(json.dumps({"answers": len(self.frames) - 1, "failed_answers": 0}, sort_keys=True), encoding="utf-8")
        return 0, ""


def manifest(run_id: str = "offline-sley2-001") -> dict:
    values = smoke_manifest(run_id, "2026-09-03T12:00:00Z", "e" * 40, digest(3))
    values["random_seeds"] = [1, 2]
    values["trial_count"] = 2
    return values


class Sley2RunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.run = Path(self.temp.name) / "run"
        self.manifest = manifest()
        self.manifest_digest = write_run_manifest(self.run, self.manifest)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def trial(self, trial_id: str, task_index: int, agent, seed: int = 1):
        corpus = json.loads((Path(__file__).resolve().parents[3] / "bench/corpus/v1/tasks.json").read_text())
        task_id = sorted(task["id"] for task in corpus["tasks"])[task_index]
        return run_scripted_trial(
            run_directory=self.run,
            trial_id=trial_id,
            task_id=task_id,
            seed=seed,
            endpoint_factory=FakeEndpoint,
            endpoint_digest=digest(9),
            hello={"body": "00", "bounds": request_frame("x", "", None, 0)["bounds"], "flags": {"cancel": False, "failed": False, "stream": False}, "kind": "hello", "method": "", "protocol_version": 1, "request_id": 0, "session": None},
            affordances=["session.capabilities", "refs.list", "handle.expand", "session.budgets", "candidate.validate", "capsule", "exchange.import", "session.open", "session.close"],
            endpoint_version={"cli": "1"},
            handshake_id=digest(7),
            exchange_hex="cafe",
            fixture_digest=digest(3),
            prompt_digest=digest(5),
            agent=agent,
            oracle=ScriptedOracle(),
            clock=Clock(),
        )

    def test_trace_chain_is_complete_and_tamper_evident(self) -> None:
        trace = Trace(self.run, "t1", self.manifest_digest)
        append_trace_record(trace, {"kind": "header", "trial_id": "t1"})
        frame = request_frame("refs.list", "10", "ab" * 32, 1)
        append_trace_record(trace, {"direction": "request", "frame": frame, "frame_sha256": hashlib.sha256(canonical_json_bytes(frame)).hexdigest(), "kind": "frame", "seq": 0})
        append_trace_record(trace, {"frames_recorded": 1, "kind": "footer", "outcome": "completed"})
        trace.close()
        records = verify_trace(trace.path, self.manifest_digest)
        self.assertEqual([r["kind"] for r in records], ["header", "frame", "footer"])
        with self.assertRaises(Sley2RunnerError) as duplicate:
            Trace(self.run, "t1", self.manifest_digest)
        self.assertEqual(duplicate.exception.code, Sley2ErrorCode.DUPLICATE)
        lines = trace.path.read_bytes().split(b"\n")
        tampered = trace.path.with_name("t1-tampered.trace.jsonl")
        tampered.write_bytes(b"\n".join([lines[0], lines[2], b""]))
        with self.assertRaises(Sley2RunnerError) as broken:
            verify_trace(tampered, self.manifest_digest)
        self.assertEqual(broken.exception.code, Sley2ErrorCode.TRACE_INVALID)
        edited = json.loads(lines[1])
        edited["frame"]["body"] = "11"
        tampered.write_bytes(b"\n".join([lines[0], canonical_json_bytes(edited), lines[2], b""]))
        with self.assertRaises(Sley2RunnerError):
            verify_trace(tampered, self.manifest_digest)
        tampered.write_bytes(b"\n".join([lines[0], lines[1], b""]))
        with self.assertRaises(Sley2RunnerError) as footer:
            verify_trace(tampered, self.manifest_digest)
        self.assertEqual(footer.exception.detail, "missing footer")

    def test_metrics_derive_only_from_frame_records(self) -> None:
        session = "ab" * 32
        frames = [
            ("request", request_frame("exchange.import", "cafe", None, 1)),
            ("request", request_frame("capsule", "", session, 2)),
            ("event", {**request_frame("capsule", "aa" * 10, session, 2), "kind": "event"}),
            ("response", {**request_frame("capsule", "bb" * 6, session, 2), "kind": "response", "bounds": {**request_frame("x", "", None, 0)["bounds"], "returned_entities": 5, "returned_edges": "7"}}),
            ("request", request_frame("candidate.validate", "", session, 3)),
            ("response", {**request_frame("candidate.validate", "", session, 3), "kind": "response", "flags": {"cancel": False, "failed": True, "stream": False}}),
            ("request", request_frame("candidate.validate", "", session, 4)),
            ("response", {**request_frame("candidate.validate", "", session, 4), "kind": "response"}),
        ]
        records = [{"kind": "header"}]
        for seq, (direction, frame) in enumerate(frames):
            records.append({"kind": "frame", "seq": seq, "direction": direction, "frame": frame})
        records.append({"kind": "footer"})
        metrics = derive_trace_metrics(records)
        self.assertEqual(metrics["tool_calls"], 3)
        self.assertEqual(metrics["context_bytes"], 16)
        self.assertEqual(metrics["entities_inspected"], 5)
        self.assertEqual(metrics["relationships_inspected"], 7)
        self.assertEqual(metrics["compile_or_check_attempts"], 2)
        self.assertEqual(metrics["repair_loops"], 1)
        self.assertEqual(metrics["invalid_candidates"], 1)
        self.assertEqual((metrics["attempted_tasks"], metrics["files_inspected"], metrics["human_interventions"]), (1, 0, 0))

    def test_handle_exposes_exactly_two_operations_and_refuses_runner_fields(self) -> None:
        handle = EndpointHandle(lambda request: [dict(request)], ["refs.list"])
        self.assertEqual(handle_surface(handle), set(HANDLE_SURFACE))
        self.assertEqual(handle.affordances(), ["refs.list"])
        self.assertEqual(handle.exchange({"method": "refs.list", "body": "10"}), [{"method": "refs.list", "body": "10"}])
        with self.assertRaises(AttributeError):
            handle.repository = "/tmp"  # type: ignore[attr-defined]

        class Intruder:
            def run(self, handle: EndpointHandle):
                handle.exchange({"method": "refs.list", "body": "10", "request_id": 99})
                return {}

        summary = self.trial("guard-1", 0, Intruder())
        self.assertEqual(summary["status"], "harness_failure")
        self.assertEqual(summary["failure_code"], "SLEY2_TRIAL_PRIVILEGED_CONTEXT")
        records = verify_trace(Path(summary["trace_path"]), self.manifest_digest)
        self.assertEqual(records[-1]["outcome"], "harness_failure")
        self.assertEqual(records[-1]["failure_code"], "SLEY2_TRIAL_PRIVILEGED_CONTEXT")

        class Unlisted:
            def run(self, handle: EndpointHandle):
                handle.exchange({"method": "commit", "body": ""})
                return {}

        summary = self.trial("guard-2", 1, Unlisted())
        self.assertEqual(summary["failure_code"], "SLEY2_TRIAL_FRAME_INVALID")

    def test_scripted_trial_over_a_fake_endpoint_traces_claims_and_verifies(self) -> None:
        summary = self.trial("trial-1", 0, ScriptedAgent())
        self.assertEqual((summary["outcome"], summary["status"]), ("completed", "rejected"))
        self.assertEqual(summary["metrics"]["tool_calls"], 4)
        self.assertEqual(summary["metrics"]["entities_inspected"], 3)
        self.assertEqual(summary["metrics"]["accepted_correct_changes"], 0)
        self.assertIsNone(summary["metrics"]["accepted_change_tokens"])
        self.assertEqual(summary["endpoint_exit_status"], 0)
        records = verify_trace(Path(summary["trace_path"]), self.manifest_digest)
        self.assertEqual(records[0]["kind"], "header")
        self.assertEqual(records[-1]["frames_recorded"], summary["metrics"]["tool_calls"] * 2 + 2 + 2 + 2 + 2)
        self.assertEqual(records[-1]["record_digest"], summary["trace_head_digest"])
        methods = [r["frame"]["method"] for r in records if r.get("kind") == "frame" and r["direction"] == "request"]
        self.assertEqual(methods, ["", "exchange.import", "session.open", "session.capabilities", "refs.list", "handle.expand", "session.budgets", "session.close"])
        claims = verify_trial_claims(self.run)
        self.assertEqual(len(claims), 1)
        self.assertEqual(claims[0]["arm_id"], ARM)
        self.assertEqual(claims[0]["contract"], CLAIM_CONTRACT)
        self.assertEqual(claims[0]["trace_head_digest"], summary["trace_head_digest"])
        self.assertEqual(claims[0]["record_digest"], summary["claim_digest"])
        with self.assertRaises(Sley2RunnerError) as duplicate:
            self.trial("trial-1", 0, ScriptedAgent())
        self.assertEqual(duplicate.exception.code, Sley2ErrorCode.DUPLICATE)
        second = self.trial("trial-2", 1, ScriptedAgent(), seed=2)
        self.assertEqual(len(verify_trial_claims(self.run)), 2)
        with self.assertRaises(Sley2RunnerError) as incomplete:
            verify_trial_claims(self.run, require_complete=True)
        self.assertEqual(incomplete.exception.code, Sley2ErrorCode.DUPLICATE)
        self.assertNotEqual(second["claim_digest"], summary["claim_digest"])

    def test_claim_validation_fails_closed(self) -> None:
        claims_path = self.run / "sley2" / "claims.jsonl"
        base = {
            "accounting_verification_status": "UNVERIFIED_ADAPTER_CLAIM",
            "arm_id": ARM,
            "contract": CLAIM_CONTRACT,
            "endpoint_sha256": digest(9),
            "ended_at_utc": "2026-09-03T12:00:02Z",
            "evidence_status": "UNVERIFIED_INJECTED_DIGEST_CLAIMS",
            "exchange_digest": digest(4),
            "failure_code": "ORACLE_REJECTED",
            "fixture_digest": digest(3),
            "handshake_id": digest(7),
            "metrics": {name: 0 for name in json.loads(PLAN_PATH.read_text())["metrics"]},
            "model_output_digest": digest(11),
            "oracle_report_digest": digest(12),
            "oracle_verification_status": "UNVERIFIED_ADAPTER_CLAIM",
            "prompt_digest": digest(5),
            "report_digest": digest(13),
            "run_id": self.manifest["run_id"],
            "seed": 1,
            "started_at_utc": "2026-09-03T12:00:01Z",
            "status": "rejected",
            "task_id": "S2B-CREATE-001",
            "timeout": False,
            "trace_head_digest": digest(8),
            "trace_record_count": 3,
            "trial_id": "claim-1",
        }
        base["metrics"].update({"attempted_tasks": 1, "strict_accepted_correctness": False, "accepted_change_tokens": None})
        append_trial_claim(self.run, base)
        self.assertTrue(claims_path.exists())
        for field, value in (("arm_id", "raw_files"), ("status", "accepted"), ("timeout", True), ("trace_record_count", 1), ("handshake_id", "zz")):
            bad = dict(base, trial_id="claim-2", seed=2)
            bad[field] = value
            with self.assertRaises(Sley2RunnerError, msg=field) as error:
                append_trial_claim(self.run, bad)
            self.assertEqual(error.exception.code, Sley2ErrorCode.CLAIM_INVALID, field)
        wrong_task = dict(base, trial_id="claim-3", seed=2, task_id="S2B-NOPE-999")
        with self.assertRaises(Sley2RunnerError):
            append_trial_claim(self.run, wrong_task)
        self.assertEqual(len(verify_trial_claims(self.run)), 1)
        raw = claims_path.read_bytes()
        claims_path.write_bytes(raw[:-3] + b"\n")
        with self.assertRaises(Sley2RunnerError) as broken:
            verify_trial_claims(self.run)
        self.assertEqual(broken.exception.code, Sley2ErrorCode.CLAIM_INVALID)
        self.assertEqual(manifest_digest(self.manifest), self.manifest_digest)


if __name__ == "__main__":
    unittest.main()
