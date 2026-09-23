"""CONTEXT campaign-path integration proofs for confined mediated sley_2_0.

Every test enters through ``execute_attempt`` with the REAL fixture
oracle (``run_fixture_oracle``) and finishes through the normal
record-verification path (``verify_attempts``). The only stand-in is
the agent command (a deterministic confined adapter sequence,
``mediated_client.py context MODE``); confinement, gateway, endpoint,
capture, oracle, append, and verification are the real production
path. These runs are NOT model trials and are never counted toward the
preregistered live-model campaign.

Bounded discovery through the interface (REQ-10, trial-runner contract
revision 5): the stand-in receives no entity identity, member literal,
or manifest value as an argument — its only argument is the mode. It
opens the accepted head (`open`, the afforded `workspace.open`),
materializes and binds the head's index snapshot identity, finds the
record typedef by a bounded class-4 listing, walks the class-14 reverse
impact closure in 3-entity pages with explicit `query.continue` (each
continue bound by the judge to the page it continues, by query and
cursor),
probes kinds (class 2), reads the impacted constants, and authors the
repair. The judge derives access/budget evidence from the runner-owned
reconciled capture (``SLEY2_MEDIATED_CAPTURE_DIR``).
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
GATED = unittest.skipUnless(
    NEEDS_BINARY and NEEDS_JUDGE_BINARY and NEEDS_CONFINEMENT,
    "needs SLEY2_SLEY_BINARY + SUCC_JUDGE_TEST_BINARY + bwrap")


class MediatedContextTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.run = self.root / "run"
        self.run.mkdir()
        write_manifest_once(self.run / "run_manifest.json", manifest())
        self.store = ArtifactStore(self.run / "artifacts")
        self.verdicts: list[dict] = []

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def attempt(self, mode: str, oracle_runner=None) -> dict:
        from bench.live.oracle import run_fixture_oracle

        adapter = StandInAdapter("context", [mode])
        return execute_attempt(
            run_directory=self.run,
            store=self.store,
            adapter=adapter,
            task_id=TASK_ID,
            arm_id="sley_2_0",
            seed=17,
            workspace_parent=self.root / "workspaces",
            provider_runner=run_provider_process,
            oracle_runner=oracle_runner or run_fixture_oracle,
            utc_now=lambda: "2026-09-17T12:01:00Z",
        )

    def summary(self, record: dict) -> dict:
        stderr = self.store.read(record["artifacts"]["provider_stderr_sha256"])
        line = stderr.split(b"SUMMARY ", 1)[1].split(b"\n")[0]
        return json.loads(line)

    def exchanges(self, record: dict) -> list[dict]:
        capture = (self.run / "captures" / record["attempt_id"]
                   / "exchanges.jsonl")
        return [json.loads(line) for line in
                capture.read_text(encoding="utf-8").splitlines()]

    def judge_detail(self, record: dict) -> str:
        stdout = self.store.read(record["artifacts"]["oracle_stdout_sha256"])
        return stdout.decode("utf-8", "replace")

    @GATED
    def test_context_discovery_and_repair_accepted_end_to_end(self) -> None:
        """Positive: interface discovery with actual continuation, agent
        mutation, production admission (propose/validate + finish), the
        real oracle, append, and verification."""

        record = self.attempt("pos")
        self.assertEqual(record["status"], "accepted", self.judge_detail(record))
        self.assertIsNone(record["failure_code"])
        verified = verify_attempts(self.run, self.store)
        self.assertEqual(len(verified), 1)
        self.assertEqual(verified[0]["evidence_status"],
                         "VERIFIED_LIVE_EVIDENCE")
        summary = self.summary(record)
        discovery = summary["discovery"]
        self.assertTrue(summary["finished"])
        # Cold head snapshot: materialized on the query path, then bound.
        self.assertFalse(discovery["opened_with_snapshot"])
        self.assertTrue(discovery["warm_refused"])
        self.assertEqual(len(discovery["snapshot"]), 64)
        self.assertEqual(discovery["record_typedefs"], 1)
        # Ten-entity closure in 3-entity pages: three real continuations.
        self.assertEqual(discovery["impact_total"], 10)
        self.assertEqual(discovery["impact_seen"], 10)
        self.assertEqual(discovery["impact_continuations"], 3)
        self.assertFalse(discovery["impact_left_truncated"])
        self.assertEqual(discovery["impacted_constants"], 3)
        methods = [item["method"] for item in self.exchanges(record)
                   if item["kind"] == "request"]
        self.assertEqual(methods[:2], ["open", "revision"])
        self.assertEqual(methods.count("raw:query.continue"), 3)
        self.assertIn("raw:query.root", methods)
        self.assertNotIn("inventory", methods)
        self.assertNotIn("side", methods)
        self.assertEqual(methods[-2:], ["propose", "finish"])
        # Every answered root-query page carries its continuation binding,
        # and each continue's cursor is the previous page's next cursor.
        chains = [item.get("chain") for item in self.exchanges(record)
                  if item["kind"] == "response" and not item["failed"]
                  and item["method"] in ("raw:query.root", "raw:query.continue")]
        self.assertTrue(chains and all(isinstance(c, dict) for c in chains))
        impact = [c for c in chains if c["truncated"] or c["after"]]
        self.assertEqual(len(impact), 4)
        for page, following in zip(impact, impact[1:]):
            self.assertEqual(following["query"], page["query"])
            self.assertEqual(following["after"], page["next"])

    @GATED
    def test_context_incomplete_discovery_rejects(self) -> None:
        """The agent stops after the first truncated impact page. Its
        repair happens to be complete (the first page already holds
        every constant) and finishes, but discovery never followed the
        truncation: the judge rejects the attempt."""

        record = self.attempt("stop_early")
        summary = self.summary(record)
        self.assertTrue(summary["finished"])
        self.assertTrue(summary["discovery"]["impact_left_truncated"])
        self.assertEqual(summary["discovery"]["impacted_constants"], 3)
        self.assertEqual(record["status"], "rejected")
        self.assertEqual(record["failure_code"], "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("truncated page without continuation",
                      self.judge_detail(record))

    @GATED
    def test_context_fake_discharge_rejects(self) -> None:
        """Vulcan r5 P2: after a truncated impact page, a refused continue,
        a frame whose command imitates a routed continue, and a continue at
        a past-the-end cursor (answered, empty) close nothing. The repair
        is complete and finishes, yet the judge rejects."""

        record = self.attempt("fake_discharge")
        summary = self.summary(record)
        self.assertTrue(summary["finished"])
        fake = summary["discovery"]["fake"]
        self.assertTrue(fake["refused"])
        self.assertFalse(fake["forged_ok"])
        self.assertEqual(fake["past_end_returned"], 0)
        labels = [item["method"] for item in self.exchanges(record)
                  if item["kind"] == "request"]
        self.assertIn("denied", labels)
        self.assertNotIn("raw:query.continue:forged", labels)
        self.assertEqual(record["status"], "rejected")
        self.assertEqual(record["failure_code"], "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("inconsistent continuation", self.judge_detail(record))

    @GATED
    def test_context_incomplete_impact_cannot_finish(self) -> None:
        """Complete discovery, typedef-only repair: production validation
        refuses the incomplete closure, so no finishable candidate forms
        and the attempt fails closed with no final."""

        record = self.attempt("incomplete")
        self.assertFalse(self.summary(record).get("finished"))
        self.assertEqual(record["status"], "harness_failure")
        self.assertEqual(record["failure_code"], "CAPTURE_GATE_NO_FINAL")

    @GATED
    def test_context_inconsistent_continuation_rejects(self) -> None:
        """A query.continue after the impact chain completed (no
        truncated page pending) is an inconsistent continuation; the
        judge rejects although the repair itself is complete."""

        record = self.attempt("extra_continue")
        self.assertTrue(self.summary(record)["finished"])
        self.assertEqual(record["status"], "rejected")
        self.assertEqual(record["failure_code"], "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("inconsistent continuation", self.judge_detail(record))

    @GATED
    def test_context_exceeded_budget_rejects(self) -> None:
        """Complete discovery and repair plus seven full constant
        listings: each response fits the per-response bound, but the
        cumulative agent-visible bytes exceed the trial budget."""

        record = self.attempt("overbudget")
        self.assertTrue(self.summary(record)["finished"])
        self.assertEqual(record["status"], "rejected")
        self.assertEqual(record["failure_code"], "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("over budget", self.judge_detail(record))

    @GATED
    def test_context_missing_evidence_never_accepts(self) -> None:
        """The positive flow, with the authoritative capture exchanges
        lost before the oracle runs: the judge rejects the missing
        evidence and reconciliation fails closed, so the attempt is
        never accepted."""

        from bench.live.oracle import run_fixture_oracle

        def lossy_oracle(*, arm_id, task_id, candidate):
            capture = Path(os.environ["SLEY2_MEDIATED_CAPTURE_DIR"])
            (capture / "exchanges.jsonl").rename(capture / "exchanges.lost")
            verdict = run_fixture_oracle(arm_id=arm_id, task_id=task_id,
                                         candidate=candidate)
            self.verdicts.append(dict(verdict[0]))
            return verdict

        record = self.attempt("pos", oracle_runner=lossy_oracle)
        self.assertTrue(self.summary(record)["finished"])
        self.assertEqual(len(self.verdicts), 1)
        self.assertEqual(self.verdicts[0]["status"], "rejected")
        self.assertEqual(self.verdicts[0]["code"], "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("mediated exchanges missing", self.judge_detail(record))
        self.assertNotEqual(record["status"], "accepted")
        self.assertEqual(record["status"], "harness_failure")
        self.assertTrue(str(record["failure_code"]).startswith("CAPTURE_"),
                        record["failure_code"])

    @GATED
    def test_context_no_argument_revision_is_refused_and_counted(self) -> None:
        """`revision` without a transaction id has no harness head to
        fall back on: the refusal is a captured, counted failed
        exchange, and the agent proceeds through `open` + `revision
        <tx>` to an accepted repair."""

        record = self.attempt("noarg_revision")
        summary = self.summary(record)
        self.assertTrue(summary["discovery"]["noarg_revision_refused"])
        exchanges = self.exchanges(record)
        self.assertEqual(exchanges[0]["method"], "revision")
        self.assertEqual(exchanges[1]["kind"], "response")
        self.assertTrue(exchanges[1]["failed"])
        self.assertEqual(record["status"], "accepted", self.judge_detail(record))


if __name__ == "__main__":
    unittest.main()
