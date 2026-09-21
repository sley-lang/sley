"""CONTEXT campaign-path integration proofs for confined mediated sley_2_0.

Every test enters through ``execute_attempt`` with the REAL fixture
oracle (``run_fixture_oracle``) and finishes through the normal
record-verification path (``verify_attempts``). The only stand-in is
the agent command (a deterministic confined adapter sequence);
confinement, gateway, endpoint, capture, oracle, append, and
verification are the real production path. These runs are NOT model
trials and are never counted toward the preregistered live-model
campaign.

Mechanics scaffolding (disclosed, not fairness proof): the stand-in
receives typedef/const/member identities as invocation arguments.
No permitted bounded route can enumerate a typedef's users today
(inventory is whole-store; reads need ids; server queries need an
unmintable snapshot), and the required member literal lives only in
the private manifest — so impact discovery itself is retained as
the review gate. These tests prove the fix mechanics and the
mediated access-evidence route end to end: the judge derives
access/budget evidence from the runner-owned reconciled capture
(``SLEY2_MEDIATED_CAPTURE_DIR``), never the obsolete
candidate-workspace chain file.
"""

from __future__ import annotations

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
from bench.live.process import run_provider_process
from bench.live.tests.test_attempts import manifest
from bench.live.tests.test_mediated_attempt import StandInAdapter

ROOT = Path(__file__).resolve().parents[3]
TASK_ID = "S2B-CONTEXT-001"

NEEDS_BINARY = os.environ.get("SLEY2_SLEY_BINARY", "")
NEEDS_JUDGE_BINARY = os.environ.get("SUCC_JUDGE_TEST_BINARY", "")
NEEDS_CONFINEMENT = bwrap_available()

_FIXTURE = json.loads(
    (ROOT / "bench" / "fixtures" / "sley2" / TASK_ID
     / "task_manifest.json").read_text(encoding="utf-8"))
_TYPEDEF = _FIXTURE["entities"]["typedef"]
_CONSTS = ",".join(_FIXTURE["entities"][role] for role in
                   ("user_const_0", "user_const_1", "user_const_2"))
_MEMBER = "e1" * 32


class MediatedContextTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.run = self.root / "run"
        self.run.mkdir()
        write_manifest_once(self.run / "run_manifest.json", manifest())
        self.store = ArtifactStore(self.run / "artifacts")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def attempt(self, mode: str) -> dict:
        from bench.live.oracle import run_fixture_oracle

        return execute_attempt(
            run_directory=self.run,
            store=self.store,
            adapter=StandInAdapter(
                "context_pos", [_TYPEDEF, _CONSTS, _MEMBER, mode]),
            task_id=TASK_ID,
            arm_id="sley_2_0",
            seed=17,
            workspace_parent=self.root / "workspaces",
            provider_runner=run_provider_process,
            oracle_runner=run_fixture_oracle,
            utc_now=lambda: "2026-09-17T12:01:00Z",
        )

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_JUDGE_BINARY
                         and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + SUCC_JUDGE_TEST_BINARY + bwrap")
    def test_context_pos_mediated_oracle(self) -> None:
        """Genuine bounded context_pos proof: the stand-in fixes the
        typedef plus the complete impact closure through bounded
        reads/propose/finish on the mediated path; the real oracle
        judges the protected workspace with access evidence derived
        from the reconciled capture."""

        record = self.attempt("pos")
        self.assertEqual(record["status"], "accepted")
        self.assertIsNone(record["failure_code"])
        verified = verify_attempts(self.run, self.store)
        self.assertEqual(len(verified), 1)
        self.assertEqual(verified[0]["evidence_status"],
                         "VERIFIED_LIVE_EVIDENCE")

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_JUDGE_BINARY
                         and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + SUCC_JUDGE_TEST_BINARY + bwrap")
    def test_context_incomplete_impact_cannot_finish(self) -> None:
        """Typedef-only fix: production validation refuses the
        incomplete closure (phase 6), so no finishable candidate
        forms and the attempt fails closed with no final — the same
        negative the direct-path witness records. ORACLE_IMPACT_
        INCOMPLETE stays in the judge as defense in depth behind
        validation."""

        record = self.attempt("incomplete")
        self.assertEqual(record["status"], "harness_failure")
        self.assertEqual(record["failure_code"], "CAPTURE_GATE_NO_FINAL")

    @unittest.skipUnless(NEEDS_BINARY and NEEDS_JUDGE_BINARY
                         and NEEDS_CONFINEMENT,
                         "needs SLEY2_SLEY_BINARY + SUCC_JUDGE_TEST_BINARY + bwrap")
    def test_context_inconsistent_continuation_rejects(self) -> None:
        """query.continue frames with no preceding truncation in
        scope: the mediated access audit rejects with the frozen
        unbounded-read code even though the fix itself is complete."""

        record = self.attempt("spam")
        self.assertEqual(record["status"], "rejected")
        self.assertEqual(record["failure_code"],
                         "QUERY_REQUIRED_FACT_OMITTED")


if __name__ == "__main__":
    unittest.main()
