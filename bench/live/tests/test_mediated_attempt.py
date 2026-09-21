"""Campaign-path integration proofs for confined mediated sley_2_0.

Every test enters through ``execute_attempt`` and finishes through
the normal record-verification path (``verify_attempts``). The only
stand-in is the agent command itself (a deterministic confined
adapter sequence); confinement, gateway, endpoint, capture, oracle,
artifact store, attempt append, and verification are the real
production path. These runs are NOT model trials and are never
counted toward the preregistered live-model campaign.
"""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path

from bench.live.artifacts import ArtifactStore
from bench.live.attempts import verify_attempts
from bench.live.campaign import execute_attempt
from bench.live.confined import bwrap_available
from bench.live.manifest import write_manifest_once
from bench.live.process import ProcessCapture, run_provider_process
from bench.live.tests.test_attempts import manifest

ROOT = Path(__file__).resolve().parents[3]
TASK_ID = "S2B-TYPE-001"

NEEDS_BINARY = os.environ.get("SLEY2_SLEY_BINARY", "")
NEEDS_JUDGE_BINARY = os.environ.get("SUCC_JUDGE_TEST_BINARY", "")
NEEDS_CONFINEMENT = bwrap_available()


def sley2_oracle_line(task_id: str, status: str,
                      code: str | None) -> bytes:
    return (json.dumps({"arm": "sley2", "code": code,
                        "detail": "campaign-path stub",
                        "status": status, "task_id": task_id},
                       sort_keys=True).encode() + b"\n")


class StandInAdapter:
    """Deterministic provider stand-in at the provider boundary: the
    configured model identity is preserved; the launched command is
    a confined deterministic adapter sequence instead of a model.

    The stand-in is test-only: production staging holds only the
    generic transport/shim/tooling, and this adapter injects its
    scripted client AFTER that staging is verified, exercising the
    real confinement, mediation, capture, oracle, append, and
    verification machinery."""

    def __init__(self, sequence: str, extra: list[str] | None = None) -> None:
        self.model = "gpt-5.6-sol"
        self.reasoning_effort = "medium"
        self._sequence = sequence
        self._extra = list(extra or [])

    def extra_scratch_files(self) -> list[tuple[str, bytes]]:
        return [("mediated_client.py",
                 (ROOT / "bench" / "live" / "mediated_client.py").read_bytes())]

    def command(self, workspace) -> list[str]:
        _ = workspace
        return ["python3", "/scratch/mediated_client.py",
                self._sequence, *self._extra]


class MediatedAttemptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.run = self.root / "run"
        self.run.mkdir()
        write_manifest_once(self.run / "run_manifest.json", manifest())
        self.store = ArtifactStore(self.run / "artifacts")
        self.oracle_calls: list[tuple[str, str, str]] = []
        self.oracle_listings: list[list[str]] = []
        self.oracle_status = "accepted"
        self.oracle_code: str | None = None

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def oracle_stub(self, arm_id, task_id, candidate):
        self.oracle_calls.append(
            (arm_id, task_id, str(candidate)))
        # Snapshot the candidate layout during the call: the
        # disposable protected workspace is removed on return.
        self.oracle_listings.append(
            sorted(p.name for p in Path(candidate).iterdir()))
        return ({"status": self.oracle_status, "code": self.oracle_code},
                sley2_oracle_line(task_id, self.oracle_status,
                                  self.oracle_code), b"")

    def attempt(self, sequence: str, extra: list[str] | None = None,
                provider_runner=run_provider_process) -> dict:
        return execute_attempt(
            run_directory=self.run,
            store=self.store,
            adapter=StandInAdapter(sequence, extra),
            task_id=TASK_ID,
            arm_id="sley_2_0",
            seed=17,
            workspace_parent=self.root / "workspaces",
            provider_runner=provider_runner,
            oracle_runner=self.oracle_stub,
            utc_now=lambda: "2026-09-17T12:01:00Z",
        )

    def capture_dir(self, record: dict) -> Path:
        return self.run / "captures" / record["attempt_id"]

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_legitimate_flow_accepts_and_verifies(self) -> None:
        record = self.attempt("finish_skeleton")
        self.assertEqual(record["status"], "accepted")
        self.assertIsNone(record["failure_code"])
        self.assertEqual(record["provider_exit_code"], 0)
        # The oracle ran against the runner-owned protected
        # workspace (staged pack + repo), not the agent scratch.
        self.assertEqual(len(self.oracle_calls), 1)
        arm_id, task_id, candidate = self.oracle_calls[0]
        self.assertEqual((arm_id, task_id), ("sley_2_0", TASK_ID))
        self.assertIn("base.pack", self.oracle_listings[0])
        self.assertIn("repo", self.oracle_listings[0])
        self.assertNotIn("scratch", candidate)
        # The frozen start binding names the real staged pack.
        start = json.loads(
            (self.capture_dir(record) / "start.json").read_bytes())
        fixture_pack = (ROOT / "bench" / "fixtures" / "sley2" / TASK_ID
                        / "base.pack").read_bytes()
        self.assertEqual(start["frozen"]["pack_sha256"],
                         hashlib.sha256(fixture_pack).hexdigest())
        # The held final is the protected finish, bound end to end.
        stored_final = self.store.read(
            record["artifacts"]["final_candidate_sha256"])
        completion = json.loads(
            (self.capture_dir(record) / "completion.json").read_bytes())
        self.assertEqual(hashlib.sha256(stored_final).hexdigest(),
                         completion["final_sha256"])
        self.assertNotIn("forged", stored_final.decode("utf-8", "replace"))
        # Observed tool count reconciles with captured exchanges.
        self.assertEqual(record["metrics"]["tool_calls"],
                         completion["exchanges"])
        self.assertGreater(completion["exchanges"], 0)
        # Normal record-verification path promotes the attempt.
        verified = verify_attempts(self.run, self.store)
        self.assertEqual(len(verified), 1)
        self.assertEqual(verified[0]["evidence_status"],
                         "VERIFIED_LIVE_EVIDENCE")

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_oracle_accept_cannot_override_failed_capture(self) -> None:
        record = self.attempt("malformed_ingress")
        self.assertEqual(record["status"], "harness_failure")
        self.assertEqual(record["failure_code"], "MEDIATED_INGRESS_INVALID")
        # No oracle verdict was ever sought for the dead attempt.
        self.assertEqual(self.oracle_calls, [])
        self.assertEqual(len(verify_attempts(self.run, self.store)), 1)

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_oversized_ingress_invalidates(self) -> None:
        record = self.attempt("oversized_ingress")
        self.assertEqual(record["status"], "harness_failure")
        self.assertEqual(record["failure_code"], "MEDIATED_FRAME_OVERSIZED")
        self.assertEqual(self.oracle_calls, [])

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_valid_capture_cannot_override_oracle_rejection(self) -> None:
        self.oracle_status = "rejected"
        self.oracle_code = "ORACLE_TYPE_MISMATCH"
        record = self.attempt("finish_skeleton")
        self.assertEqual(record["status"], "rejected")
        self.assertEqual(record["failure_code"], "ORACLE_TYPE_MISMATCH")
        self.assertEqual(len(self.oracle_calls), 1)
        self.assertEqual(len(verify_attempts(self.run, self.store)), 1)

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_scratch_forgery_never_becomes_evidence(self) -> None:
        record = self.attempt("forge_scratch")
        self.assertEqual(record["status"], "accepted")
        stored_final = self.store.read(
            record["artifacts"]["final_candidate_sha256"])
        self.assertNotEqual(stored_final, b"forged-by-agent\n")
        transcript = self.store.read(
            record["artifacts"]["agent_transcript_sha256"])
        self.assertNotIn(b"forged", transcript)
        self.assertEqual(len(verify_attempts(self.run, self.store)), 1)

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_missing_final_blocks_acceptance(self) -> None:
        record = self.attempt("no_finish")
        self.assertEqual(record["status"], "harness_failure")
        self.assertEqual(record["failure_code"], "CAPTURE_GATE_NO_FINAL")
        self.assertEqual(self.oracle_calls, [])

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_timeout_is_retained_without_an_oracle_claim(self) -> None:
        def provider(argv, prompt, **kwargs):
            return ProcessCapture(b'{"type":"turn.started"}\n', b"", 124,
                                  True, 1)

        record = self.attempt("finish_skeleton", provider_runner=provider)
        self.assertEqual(record["status"], "timeout")
        self.assertEqual(record["failure_code"], "LIVE_PROVIDER_TIMEOUT")
        self.assertIsNone(record["artifacts"]["oracle_report_sha256"])
        self.assertEqual(self.oracle_calls, [])
        self.assertEqual(len(verify_attempts(self.run, self.store)), 1)

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_process_failure_is_retained(self) -> None:
        def provider(argv, prompt, **kwargs):
            raise RuntimeError("stand-in spawn failed")

        record = self.attempt("finish_skeleton", provider_runner=provider)
        self.assertEqual(record["status"], "harness_failure")
        self.assertEqual(record["failure_code"],
                         "LIVE_PROVIDER_PROCESS_FAILURE_RUNTIMEERROR")
        self.assertEqual(self.oracle_calls, [])

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_budgets_cumulative_across_sessions(self) -> None:
        record = self.attempt("two_phase")
        # No finish was produced, so no acceptance is possible; the
        # cumulative ledger is still retained runner-side.
        self.assertEqual(record["status"], "harness_failure")
        ledger = json.loads(
            (self.capture_dir(record) / "budgets.json").read_bytes())
        self.assertEqual(ledger["totals"]["exchanges"], 2)
        usage = self.store.read(record["artifacts"]["agent_usage_sha256"])
        self.assertEqual(json.loads(usage)["totals"]["exchanges"], 2)

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_denied_command_recorded_and_counted(self) -> None:
        record = self.attempt("denied_then_finish")
        self.assertEqual(record["status"], "accepted")
        ledger = json.loads(
            (self.capture_dir(record) / "budgets.json").read_bytes())
        # revision + denied commit + revision + propose + finish.
        self.assertEqual(ledger["totals"]["exchanges"], 5)
        self.assertGreaterEqual(ledger["totals"]["failed"], 1)

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + bwrap")
    def test_trial_has_intended_access_restrictions(self) -> None:
        record = self.attempt(
            "access_probe",
            extra=[str(ROOT / "bench" / "fixtures"),
                   str(ROOT / "oracle"),
                   str(ROOT / "crates"),
                   str(self.run)])
        self.assertEqual(record["provider_exit_code"], 0)
        # The probe outcome is scratch diagnostics; the authoritative
        # check is host-side: protected source stayed unreadable
        # inside the sandbox (per-path errno outcomes ride the
        # provider stderr stream, retained as evidence). Writes may
        # land in the sandbox-private tmpfs and vanish on exit; reads
        # must fail.
        stderr = self.store.read(record["artifacts"]["provider_stderr_sha256"])
        self.assertIn(b"ACCESS_PROBE_RESULT", stderr)
        probe = json.loads(
            stderr.split(b"ACCESS_PROBE_RESULT ", 1)[1].split(b"\n")[0])
        for path in (str(ROOT / "bench" / "fixtures"), str(ROOT / "oracle"),
                     str(ROOT / "crates"), str(self.run)):
            self.assertFalse(probe["reads"][path]["ok"], path)

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_JUDGE_BINARY
                         and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + SUCC_JUDGE_TEST_BINARY + bwrap")
    def test_full_type_migration_real_oracle(self) -> None:
        """Crown proof: a legitimate mediated task through the real
        oracle and the normal verification path. The only stand-in
        is the agent command; everything else is production."""

        from bench.live.oracle import run_fixture_oracle

        record = execute_attempt(
            run_directory=self.run,
            store=self.store,
            adapter=StandInAdapter("type_pos"),
            task_id=TASK_ID,
            arm_id="sley_2_0",
            seed=17,
            workspace_parent=self.root / "workspaces",
            provider_runner=run_provider_process,
            oracle_runner=run_fixture_oracle,
            utc_now=lambda: "2026-09-17T12:01:00Z",
        )
        self.assertEqual(record["status"], "accepted")
        self.assertIsNone(record["failure_code"])
        verified = verify_attempts(self.run, self.store)
        self.assertEqual(len(verified), 1)
        self.assertEqual(verified[0]["evidence_status"],
                         "VERIFIED_LIVE_EVIDENCE")

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_JUDGE_BINARY
                         and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + SUCC_JUDGE_TEST_BINARY + bwrap")
    def test_full_stale_flip_real_oracle(self) -> None:
        """Non-TYPE mediated proof (no staged inputs): the stand-in
        discovers the guard via inventory/read through the gateway
        (captured, counted) and finishes; the real oracle judges the
        protected workspace. Same production path as the TYPE proof."""

        from bench.live.oracle import run_fixture_oracle

        record = execute_attempt(
            run_directory=self.run,
            store=self.store,
            adapter=StandInAdapter("stale_pos"),
            task_id="S2B-STALE-001",
            arm_id="sley_2_0",
            seed=17,
            workspace_parent=self.root / "workspaces",
            provider_runner=run_provider_process,
            oracle_runner=run_fixture_oracle,
            utc_now=lambda: "2026-09-17T12:01:00Z",
        )
        self.assertEqual(record["status"], "accepted")
        self.assertIsNone(record["failure_code"])
        verified = verify_attempts(self.run, self.store)
        self.assertEqual(len(verified), 1)
        self.assertEqual(verified[0]["evidence_status"],
                         "VERIFIED_LIVE_EVIDENCE")


if __name__ == "__main__":
    unittest.main()
