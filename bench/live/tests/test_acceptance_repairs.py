"""Deterministic regressions for the acceptance-correctness repair pass.

No live model calls. Binary-gated integration (three-phase compose)
runs only with SLEY2_SLEY_BINARY; all other tests are pure unit tests
over evidence parsing, transition linkage, verdict decoding, and
submitted-test coverage.
"""

from __future__ import annotations

import hashlib
import json
import unittest
from unittest import mock

from bench.fixtures import sley2_live_judge as judge
from bench.live import sley2_tool


def _h(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def _mk_test(a: int, b: int, kind: str, detail: int) -> dict:
    inputs = [{"data": {"variant": "SInt", "value": a}},
              {"data": {"variant": "SInt", "value": b}}]
    if kind == "Ok":
        inner_value = {"data": {"variant": "SInt", "value": detail}}
        result_value = {"variant": "Ok", "value": inner_value}
    else:
        failure = {"kind": "ArithmeticError", "code": detail}
        inner_value = {"data": {"variant": "BuiltinFailure", "value": failure}}
        result_value = {"variant": "Err", "value": inner_value}
    expected = {"variant": "Value", "value": {"data": {
        "variant": "Result", "value": result_value}}}
    return {"target": "f" * 64, "inputs": inputs, "expected": expected}


class ComposeEvidenceTests(unittest.TestCase):
    def test_propose_binds_out_and_ops(self) -> None:
        ops = [{"class": "CreateEntity", "target": None}]
        args, records = sley2_tool._command_evidence(
            ["propose", json.dumps(ops)], {"record": "ab" * 32})
        self.assertEqual(records["out"], _h("ab" * 32))
        self.assertIsNone(records["in"])
        self.assertEqual(args["op_count"], 1)

    def test_compose_binds_in_out_and_ops(self) -> None:
        base = "cd" * 32
        ops = [{"class": "CreateEntity", "target": None},
               {"class": "CreateEntity", "target": None}]
        args, records = sley2_tool._command_evidence(
            ["compose", base, json.dumps(ops)], {"record": "ab" * 32})
        # Compose takes two arguments: the base record and the full op
        # list. Both transitions must be bound.
        self.assertEqual(records["in"], _h(base))
        self.assertEqual(records["out"], _h("ab" * 32))
        self.assertEqual(args["op_count"], 2)
        self.assertEqual(len(args["targets"]), 2)

    def test_compose_single_arg_shape_does_not_bind_as_compose(self) -> None:
        # Regression for grouping compose with propose under len(rest)==1:
        # a single-arg compose shape must not produce compose bindings.
        args, records = sley2_tool._command_evidence(
            ["compose", "ab" * 32], {"record": "ab" * 32})
        self.assertIsNone(records["in"])
        self.assertIsNone(records["out"])

    def test_compose_evidence_carries_classes_not_nonce_claim(self) -> None:
        # Identity preservation is established by resupplied ops and
        # target equality, never by nonce reuse alone: evidence must
        # carry the op list (classes/targets/count), not a nonce claim.
        ops = [{"class": "ReplaceEntityVersion", "target": "aa" * 32}]
        args, _ = sley2_tool._command_evidence(
            ["compose", "bb" * 32, json.dumps(ops)], {"record": "cc" * 32})
        self.assertEqual(args["classes"], ["ReplaceEntityVersion"])
        self.assertNotIn("nonce", args)


class TransitionLinkageTests(unittest.TestCase):
    def _entry(self, command: str, records: dict, op_count: int = 1,
               ok: bool = True) -> dict:
        args: dict = {}
        if command in ("propose", "compose", "append"):
            args = {"op_count": op_count, "targets": [None] * op_count,
                    "classes": ["CreateEntity"] * op_count}
        return {"command": command, "records": records, "args": args,
                "ok": ok, "tool_version": sley2_tool.TOOL_VERSION,
                "pack_sha256": "x", "summary": {}, "session": []}

    def test_three_phase_chain_accepts(self) -> None:
        out0, out1, out2 = _h("r0"), _h("r1"), _h("r2")
        entries = [
            self._entry("propose", {"in": None, "out": out0}),
            self._entry("compose", {"in": out0, "out": out1}, op_count=2),
            self._entry("compose", {"in": out1, "out": out2}, op_count=3),
            self._entry("finish", {"in": out2, "out": None}),
        ]
        # Must not raise.
        judge._verify_candidate_transitions(entries)

    def test_missing_out_rejects(self) -> None:
        entries = [self._entry("propose", {"in": None, "out": None})]
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._verify_candidate_transitions(entries)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_compose_mismatch_rejects(self) -> None:
        entries = [
            self._entry("propose", {"in": None, "out": _h("r0")}),
            self._entry("compose", {"in": _h("wrong"), "out": _h("r1")}, op_count=2),
        ]
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._verify_candidate_transitions(entries)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_finish_mismatch_rejects(self) -> None:
        entries = [
            self._entry("propose", {"in": None, "out": _h("r0")}),
            self._entry("finish", {"in": _h("other"), "out": None}),
        ]
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._verify_candidate_transitions(entries)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")


class ValidDecisionTests(unittest.TestCase):
    def test_invalid_decision_cannot_accept(self) -> None:
        # A successful protocol envelope carrying Invalid must not
        # produce acceptance, even with failed=false.
        with mock.patch.object(
            judge.sley2_codecs, "run_batch",
            return_value=[{"decision_tag": 0, "failed_phase": 5}],
        ):
            with self.assertRaises(judge.JudgeRejection) as raised:
                judge._require_valid_decision("00" * 32, code="ORACLE_REBASE_INVALID")
            self.assertEqual(raised.exception.code, "ORACLE_REBASE_INVALID")

    def test_valid_decision_passes(self) -> None:
        with mock.patch.object(
            judge.sley2_codecs, "run_batch",
            return_value=[{"decision_tag": 1, "failed_phase": 0}],
        ):
            decoded = judge._require_valid_decision("00" * 32)
            self.assertEqual(decoded["decision_tag"], 1)

    def test_validate_encoder_is_distinct_from_commit(self) -> None:
        # Same layout, distinct functions: the validate path must not
        # assume the commit encoder is interchangeable.
        self.assertIsNot(judge._encode_validate_body, judge._encode_commit_body)
        tx, principal, stored = "11" * 32, "22" * 32, "33" * 64
        self.assertEqual(judge._encode_validate_body(tx, principal, stored),
                         judge._encode_commit_body(tx, principal, stored))


class TestBoundaryTests(unittest.TestCase):
    def test_correct_three_boundaries_accept(self) -> None:
        tests = [_mk_test(7, 2, "Ok", 3), _mk_test(7, 0, "Err", 2),
                 _mk_test(-9223372036854775808, -1, "Err", 1)]
        required = judge._require_test_boundaries(tests)
        self.assertEqual(required[(7, 2)], ("Ok", 3))

    def test_missing_boundary_rejects(self) -> None:
        tests = [_mk_test(7, 2, "Ok", 3), _mk_test(7, 0, "Err", 2),
                 _mk_test(1, 1, "Ok", 1)]
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._require_test_boundaries(tests)
        self.assertEqual(raised.exception.code, "ORACLE_CASE_MISSING")

    def test_duplicated_in_place_of_another_rejects(self) -> None:
        tests = [_mk_test(7, 2, "Ok", 3), _mk_test(7, 2, "Ok", 3),
                 _mk_test(7, 0, "Err", 2), _mk_test(7, 2, "Ok", 3)]
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._require_test_boundaries(tests)
        self.assertEqual(raised.exception.code, "ORACLE_CASE_MISSING")

    def test_wrong_expected_failure_rejects(self) -> None:
        tests = [_mk_test(7, 2, "Ok", 3), _mk_test(7, 0, "Err", 1),
                 _mk_test(-9223372036854775808, -1, "Err", 1)]
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._require_test_boundaries(tests)
        self.assertEqual(raised.exception.code, "ORACLE_TEST_MISMATCH")

    def test_wrong_success_value_rejects(self) -> None:
        tests = [_mk_test(7, 2, "Ok", 4), _mk_test(7, 0, "Err", 2),
                 _mk_test(-9223372036854775808, -1, "Err", 1)]
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._require_test_boundaries(tests)
        self.assertEqual(raised.exception.code, "ORACLE_TEST_MISMATCH")


class ComposeThreePhaseIntegrationTests(unittest.TestCase):
    """Real three-phase compose trace with transition bindings.

    Binary-gated; no model calls. Each phase must carry non-null
    in/out bindings, the chain must verify, and tampering must fail.
    Preservation holds only when earlier creates are resupplied
    verbatim: a replacement list under the same nonce does not preserve
    dropped creates.
    """

    def setUp(self) -> None:
        import os
        import tempfile
        from pathlib import Path

        self.sley = Path(os.environ.get(
            "SLEY2_SLEY_BINARY", "/home/gfarch/Work/target-sley2-succ/debug/sley"))
        if not (self.sley.is_file() and os.access(self.sley, os.X_OK)):
            self.skipTest("sley binary unavailable")
        from bench.live.taskpacks import stage_initial
        from bench.live.tooling import stage_tooling

        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        from pathlib import Path as _Path

        self.ws = _Path(self.temp.name) / "ws"
        stage_initial("sley_2_0", "S2B-TEST-001", self.ws)
        stage_tooling("sley_2_0", self.ws)
        manifest = json.loads(
            (_Path(__file__).resolve().parents[3]
             / "bench/fixtures/sley2/S2B-TEST-001/task_manifest.json").read_text())
        self.func = manifest["entities"]["func"]
        self._env = mock.patch.dict(os.environ, {"SLEY2_SLEY_BINARY": str(self.sley)})
        self._env.start()
        self.addCleanup(self._env.stop)

    def run_tool(self, *argv: str) -> tuple[int, dict]:
        import io
        import os

        out = io.StringIO()
        saved = os.getcwd()
        os.chdir(self.ws)
        try:
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
        finally:
            os.chdir(saved)
        return code, json.loads(out.getvalue())

    def _ok_case(self, first: int, second: int, exp: int) -> dict:
        head = {"value_type": {"variant": "SInt", "value": 64},
                "data": {"variant": "SInt", "value": first}}
        typed = {"value_type": {"variant": "SInt", "value": 64},
                 "data": {"variant": "SInt", "value": second}}
        out = {"value_type": {"variant": "SInt", "value": 64},
               "data": {"variant": "SInt", "value": exp}}
        err_t = {"variant": "BuiltinFailure", "value": "ArithmeticError"}
        expected = {"value_type": {"variant": "Result", "value": {
            "ok": {"variant": "SInt", "value": 64}, "error": err_t}},
            "data": {"variant": "Result", "value": {"variant": "Ok", "value": out}}}
        return {"class": "CreateEntity", "kind": 14, "target": None,
                "field_tag": None,
                "payload": {"target": self.func, "inputs": [head, typed],
                            "effect_environment": {"variant": "Replay", "value": []},
                            "expected": {"variant": "Value", "value": expected},
                            "observations": [],
                            "resource_limits": {"fuel": 1000000, "memory_bytes": 1000000,
                                                "output_bytes": 1000000, "effect_count": 1000,
                                                "call_depth": 64, "wall_timeout_millis": 60000}}}

    def _err_case(self, first: int, second: int, code: int) -> dict:
        head = {"value_type": {"variant": "SInt", "value": 64},
                "data": {"variant": "SInt", "value": first}}
        typed = {"value_type": {"variant": "SInt", "value": 64},
                 "data": {"variant": "SInt", "value": second}}
        err_t = {"variant": "BuiltinFailure", "value": "ArithmeticError"}
        err_const = {"value_type": {"variant": "BuiltinFailure", "value": "ArithmeticError"},
                     "data": {"variant": "BuiltinFailure", "value": {
                         "kind": "ArithmeticError", "code": code}}}
        expected = {"value_type": {"variant": "Result", "value": {
            "ok": {"variant": "SInt", "value": 64}, "error": err_t}},
            "data": {"variant": "Result", "value": {"variant": "Err", "value": err_const}}}
        return {"class": "CreateEntity", "kind": 14, "target": None,
                "field_tag": None,
                "payload": {"target": self.func, "inputs": [head, typed],
                            "effect_environment": {"variant": "Replay", "value": []},
                            "expected": {"variant": "Value", "value": expected},
                            "observations": [],
                            "resource_limits": {"fuel": 1000000, "memory_bytes": 1000000,
                                                "output_bytes": 1000000, "effect_count": 1000,
                                                "call_depth": 64, "wall_timeout_millis": 60000}}}

    def test_three_phase_compose_carries_transitions(self) -> None:
        from bench.live import sley2_codecs

        first = self._ok_case(7, 2, 3)
        code, p1 = self.run_tool("propose", json.dumps([first]))
        self.assertEqual(code, 0)
        self.assertTrue(p1["report"].get("valid"))
        base = p1["report"]["record"]
        second = self._err_case(7, 0, 2)
        code, p2 = self.run_tool("compose", base, json.dumps([first, second]))
        self.assertEqual(code, 0)
        self.assertTrue(p2["report"].get("valid"), json.dumps(p2)[:400])
        base2 = p2["report"]["record"]
        third = self._err_case(-9223372036854775808, -1, 1)
        code, p3 = self.run_tool(
            "compose", base2, json.dumps([first, second, third]))
        self.assertEqual(code, 0)
        self.assertTrue(p3["report"].get("valid"), json.dumps(p3)[:400])
        # Nonce preserved across the full chain; op counts grow 1->2->3.
        [d0] = sley2_codecs.run_batch([{"op": "describe_record", "record": base}])
        [d1] = sley2_codecs.run_batch([{"op": "describe_record", "record": base2}])
        [d2] = sley2_codecs.run_batch(
            [{"op": "describe_record", "record": p3["report"]["record"]}])
        self.assertEqual(d0["nonce"], d1["nonce"])
        self.assertEqual(d1["nonce"], d2["nonce"])
        self.assertEqual((d0["op_count"], d1["op_count"], d2["op_count"]), (1, 2, 3))
        # First identity preserved byte-for-byte across resupply.
        self.assertEqual(d1["targets"][0], d0["targets"][0])
        self.assertEqual(d2["targets"][0], d0["targets"][0])
        self.assertEqual(d2["targets"][1], d1["targets"][1])
        # Transcript carries non-null transitions for every phase, and
        # the linkage verifies.
        chain = self.ws / sley2_tool.CHAIN_NAME
        entries, reason = sley2_tool.verify_transcript_chain(chain)
        self.assertIsNone(reason, reason)
        compose_entries = [e for e in entries if e.get("command") == "compose"]
        self.assertEqual(len(compose_entries), 2)
        for entry in compose_entries:
            self.assertTrue(judge._is_hash64(entry["records"].get("in")))
            self.assertTrue(judge._is_hash64(entry["records"].get("out")))
            self.assertGreater(entry["args"].get("op_count", 0), 0)
        judge._verify_candidate_transitions(entries)

    def test_nonce_reuse_with_replacement_list_does_not_preserve(self) -> None:
        from bench.live import sley2_codecs

        first = self._ok_case(7, 2, 3)
        other = self._err_case(7, 0, 2)
        code, p1 = self.run_tool(
            "propose", json.dumps([first, self._ok_case(7, 2, 3)]))
        # Two creates with distinct ordinals: distinct identities.
        if code != 0 or not p1["report"].get("valid"):
            code, p1 = self.run_tool("propose", json.dumps([first]))
            self.assertEqual(code, 0)
            self.assertTrue(p1["report"].get("valid"))
            base = p1["report"]["record"]
            [origin] = sley2_codecs.run_batch(
                [{"op": "describe_record", "record": base}])
            code, replaced = self.run_tool("compose", base, json.dumps([other]))
            self.assertEqual(code, 0)
            [full] = sley2_codecs.run_batch(
                [{"op": "describe_record", "record": replaced["report"]["record"]}])
            # Same nonce (compose contract) but the supplied list did not
            # resupply the original payload: treating nonce reuse as
            # preservation evidence would be wrong. The narrowed promise
            # is that only verbatim resupply preserves identities.
            self.assertEqual(full["nonce"], origin["nonce"])
            self.assertEqual(full["op_count"], 1)
            return
        base = p1["report"]["record"]
        [origin] = sley2_codecs.run_batch(
            [{"op": "describe_record", "record": base}])
        self.assertEqual(len(origin["targets"]), 2)
        code, replaced = self.run_tool("compose", base, json.dumps([other]))
        self.assertEqual(code, 0)
        [full] = sley2_codecs.run_batch(
            [{"op": "describe_record", "record": replaced["report"]["record"]}])
        self.assertEqual(full["nonce"], origin["nonce"])
        # The dropped second create is gone: nonce reuse did not preserve
        # the earlier set.
        self.assertNotIn(origin["targets"][1], full["targets"])


class TrustedEvidenceTests(unittest.TestCase):
    """Task-2 boundaries: provider confinement, whole-store only for
    inventory/side, bounded query paging with explicit continuations,
    hidden-truncation and summary-mismatch rejection,
    binary-identity binding. No model calls; chains sealed synthetically.
    """

    def _task_dir(self):
        from pathlib import Path

        return Path(__file__).resolve().parents[3] / "bench" / "fixtures" / "sley2" / "S2B-TEST-001"

    def _binary_digest(self) -> str:
        import os
        from pathlib import Path

        raw = os.environ.get("SLEY2_SLEY_BINARY",
                             "/home/gfarch/Work/target-sley2-succ/debug/sley")
        try:
            return hashlib.sha256(Path(raw).read_bytes()).hexdigest()
        except OSError:
            return "ab" * 32

    def _write_chain(self, trial_ws, entries: list[dict]) -> None:
        from pathlib import Path

        trial_ws = Path(trial_ws)
        previous = "0" * 64
        lines = []
        for seq, body in enumerate(entries):
            body = dict(body)
            body["seq"] = seq
            body["prev"] = previous
            sealed = sley2_tool._chain_entry(body, previous)
            previous = sealed["hash"]
            lines.append(json.dumps(sealed, sort_keys=True))
        (trial_ws / sley2_tool.CHAIN_NAME).write_text("\n".join(lines) + "\n")

    def _base_entry(self, command: str, session: list[dict] | None = None,
                    summary: dict | None = None, **over) -> dict:
        import hashlib as _hl

        task_dir = self._task_dir()
        pack_digest = _hl.sha256((task_dir / "base.pack").read_bytes()).hexdigest()
        entry: dict = {
            "tool_version": sley2_tool.TOOL_VERSION,
            "sley_binary_sha256": self._binary_digest(),
            "pack_sha256": pack_digest,
            "command": command,
            "args": {"entities": ["aa" * 32]},
            "ok": True,
            "wall_ms": 5,
            "report_bytes": 100,
            "records": {"in": None, "out": None},
            "summary": {"requests": 1, "failed": 0, "returned_bytes": 100,
                        "continuations": 0, "omitted": 0, "truncated": 0},
            "session": [],
        }
        if session is not None:
            entry["session"] = session
        if summary is not None:
            entry["summary"].update(summary)
        entry.update(over)
        return entry

    def test_provider_launch_is_confined(self) -> None:
        from bench.live.provider import CodexExecAdapter

        adapter = CodexExecAdapter(executable="/usr/bin/codex", model="gpt-5.2",
                                   reasoning_effort="high")
        command = adapter.command("/tmp/trial-ws")
        # Actual provider surfaces, not just the sley2 CLI allowlist: the
        # agent runs ephemeral, workspace-write sandboxed, without user
        # config/rules and without inherited environment.
        self.assertIn("--ephemeral", command)
        self.assertIn("workspace-write", command)
        self.assertIn("--ignore-user-config", command)
        self.assertIn("--ignore-rules", command)
        self.assertIn('shell_environment_policy.inherit="none"', command)
        self.assertIn("/tmp/trial-ws", command)

    def test_bounded_query_route_is_not_whole_store(self) -> None:
        # Bounded paging routes are not whole-store by method name: a
        # complete single-page query.root derives whole_store_reads=0.
        import tempfile
        from pathlib import Path

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        trial_ws = Path(tmp.name)
        session = [{"direction": "request", "method": "query.root",
                    "body_sha256": "0" * 64},
                   {"direction": "response", "failed": False,
                    "body_sha256": "1" * 64, "omitted": 0,
                    "truncated": False, "returned_bytes": 100}]
        entry = self._base_entry("raw", session=session,
                                 summary={"continuations": 1, "returned_bytes": 100})
        self._write_chain(trial_ws, [entry])
        access = judge._audit_agent_access(self._task_dir(), trial_ws)
        self.assertEqual(access["whole_store_reads"], 0)
        self.assertGreaterEqual(access["bounded_reads"], 1)

    def test_refs_list_is_bounded_not_whole_store(self) -> None:
        import tempfile
        from pathlib import Path

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        trial_ws = Path(tmp.name)
        session = [{"direction": "request", "method": "refs.list",
                    "body_sha256": "0" * 64},
                   {"direction": "response", "failed": False,
                    "body_sha256": "1" * 64, "omitted": 0,
                    "truncated": False, "returned_bytes": 100}]
        entry = self._base_entry("raw", session=session,
                                 summary={"returned_bytes": 100})
        self._write_chain(trial_ws, [entry])
        access = judge._audit_agent_access(self._task_dir(), trial_ws)
        self.assertEqual(access["whole_store_reads"], 0)

    def test_hidden_truncation_prevents_acceptance(self) -> None:
        # A truncated bounded page with no following query.continue is
        # hidden truncation, not a complete bounded read.
        import tempfile
        from pathlib import Path

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        trial_ws = Path(tmp.name)
        session = [{"direction": "request", "method": "query.root",
                    "body_sha256": "0" * 64},
                   {"direction": "response", "failed": False,
                    "body_sha256": "1" * 64, "omitted": 4,
                    "truncated": True, "returned_bytes": 100}]
        entry = self._base_entry("raw", session=session,
                                 summary={"continuations": 1, "omitted": 4,
                                          "truncated": True,
                                          "returned_bytes": 100})
        self._write_chain(trial_ws, [entry])
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_agent_access(self._task_dir(), trial_ws)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("continuation", raised.exception.detail)

    def test_omitted_response_prevents_acceptance(self) -> None:
        import tempfile
        from pathlib import Path

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        trial_ws = Path(tmp.name)
        session = [{"direction": "request", "method": "entity.version"},
                   {"direction": "response", "failed": False}]
        entry = self._base_entry("read", session=session,
                                 summary={"omitted": 2, "returned_bytes": 100})
        self._write_chain(trial_ws, [entry])
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_agent_access(self._task_dir(), trial_ws)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")
        self.assertIn("omitted", raised.exception.detail)

    def test_binary_identity_mismatch_prevents_acceptance(self) -> None:
        import tempfile
        from pathlib import Path

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        trial_ws = Path(tmp.name)
        first = self._base_entry("revision")
        second = self._base_entry("revision")
        second["sley_binary_sha256"] = "00" * 32
        self._write_chain(trial_ws, [first, second])
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_agent_access(self._task_dir(), trial_ws)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")

    def test_missing_binary_binding_prevents_acceptance(self) -> None:
        import tempfile
        from pathlib import Path

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        trial_ws = Path(tmp.name)
        entry = self._base_entry("revision")
        del entry["sley_binary_sha256"]
        self._write_chain(trial_ws, [entry])
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._audit_agent_access(self._task_dir(), trial_ws)
        self.assertEqual(raised.exception.code, "QUERY_REQUIRED_FACT_OMITTED")


class RootBindingTests(unittest.TestCase):
    """Owner-derived EntityId -> ObjectId resolution (task 3).

    Acceptance-critical reads resolve currency through live
    entity.version bindings under the accepted head, never through
    first-matching object files. These run against the real binary and
    frozen TEST pack (no model calls).
    """

    def setUp(self) -> None:
        import os
        import tempfile
        from pathlib import Path

        raw = os.environ.get("SLEY2_SLEY_BINARY",
                             "/home/gfarch/Work/target-sley2-succ/debug/sley")
        self.sley = Path(raw)
        if not (self.sley.is_file() and os.access(self.sley, os.X_OK)):
            self.skipTest("sley binary unavailable")
        from bench.live.taskpacks import stage_initial
        from bench.live.tooling import stage_tooling

        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.ws = Path(self.temp.name) / "ws"
        stage_initial("sley_2_0", "S2B-TEST-001", self.ws)
        stage_tooling("sley_2_0", self.ws)
        task_dir = (Path(__file__).resolve().parents[3] / "bench" / "fixtures"
                    / "sley2" / "S2B-TEST-001")
        manifest = json.loads((task_dir / "task_manifest.json").read_text())
        self.func = manifest["entities"]["func"]
        self._env = mock.patch.dict(os.environ, {"SLEY2_SLEY_BINARY": str(self.sley)})
        self._env.start()
        self.addCleanup(self._env.stop)
        from bench.live.sley2_tool import Session

        self.session = Session(self.sley, self.ws, [], seed_pack=True)
        self.addCleanup(self.session.close)
        self.repo = self.ws / sley2_tool.REPO_DIR

    def test_bound_bytes_follow_live_binding(self) -> None:
        live_id = judge._live_object_id(self.session, self.func)
        self.assertTrue(judge._is_hash64(live_id))
        bound = judge._entity_bound_bytes(self.session, self.repo, self.func)
        self.assertIsNotNone(bound)
        # Exact object at the live binding, decoding to the entity.
        exact = judge._bound_object_bytes(self.repo, live_id)
        self.assertEqual(bound, exact)
        path = judge._object_path(self.repo, live_id)
        self.assertIsNotNone(path)
        [decoded] = sley2_tool.sley2_codecs.run_batch([{
            "op": "decode_object", "stored": bound.hex(),
            "epoch": self.session.head.get("epoch", ""),
        }])
        self.assertEqual(decoded["decoded"].get("entity_id"), self.func)

    def live_set(self) -> set[str]:
        from bench.live.sley2_tool import Session

        def connect() -> Session:
            return Session(self.sley, self.ws, [], seed_pack=False)

        return judge._live_entity_set(
            connect, self.ws, self.repo,
            self.session.head.get("epoch", ""))

    def test_filename_order_decoy_cannot_shadow_binding(self) -> None:
        before = judge._entity_bound_bytes(self.session, self.repo, self.func)
        live_before = self.live_set()
        self.assertIn(self.func, live_before)
        # Decoy file sorting before every real object: valid hex stem,
        # content decoding to an already-live entity (copied object
        # bytes). File scans see it; the live binding must not move.
        live_id = judge._live_object_id(self.session, self.func)
        live_path = judge._object_path(self.repo, live_id)
        assert live_path is not None
        decoy = self.repo / "objects" / "scb1" / ("00" * 32 + ".scb1")
        decoy.write_bytes(live_path.read_bytes())
        try:
            self.assertEqual(
                judge._entity_bound_bytes(self.session, self.repo, self.func),
                before)
            self.assertEqual(self.live_set(), live_before)
        finally:
            decoy.unlink()

    def test_absent_entity_has_no_binding(self) -> None:
        # Deleted/absent entities fail entity.version: no binding, not
        # present — history files can never resurrect them here.
        self.assertIsNone(judge._live_object_id(self.session, "ff" * 32))
        self.assertFalse(judge._entity_present(self.session, "ff" * 32))
        self.assertIsNone(
            judge._entity_bound_bytes(self.session, self.repo, "ff" * 32))


class CreateClassifierTests(unittest.TestCase):
    """CREATE overflow/ceiling rules (pure classifier level): the frozen
    negative codes are specified judge behavior. Live reachability
    differs (checked-only ISA), so these pin the mapping directly."""

    def err(self, code: int) -> dict:
        return {"ok": True, "value": {"Result": {"Err": {"BuiltinFailure": {
            "kind": "Arithmetic", "code": code}}}}}

    def ok(self, value: int) -> dict:
        return {"ok": True, "value": {"Result": {"Ok": {"SInt": str(value)}}}}

    def test_overflow_err_passes(self) -> None:
        judge._check_overflow_result(self.err(1), {"code": 1},
                                     "ORACLE_UNCHECKED_ARITHMETIC")

    def test_overflow_value_is_unchecked(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._check_overflow_result(self.ok(0), {"code": 1},
                                         "ORACLE_UNCHECKED_ARITHMETIC")
        self.assertEqual(raised.exception.code, "ORACLE_UNCHECKED_ARITHMETIC")

    def test_overflow_wrong_code_mismatches(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._check_overflow_result(self.err(2), {"code": 1},
                                         "ORACLE_UNCHECKED_ARITHMETIC")
        self.assertEqual(raised.exception.code, "ORACLE_CREATE_MISMATCH")

    def test_overflow_driver_failure_mismatches(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._check_overflow_result({"ok": False, "code": "X"},
                                         {"code": 1},
                                         "ORACLE_UNCHECKED_ARITHMETIC")
        self.assertEqual(raised.exception.code, "ORACLE_CREATE_MISMATCH")

    def checks(self) -> list:
        return [[1812500, 10000, 181], [0, 10000, 0], [1, 10000, 0]]

    def test_ceiling_triple_is_wrong_cents(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._reject_ceil_near_miss([("e", [182, 0, 1])], self.checks())
        self.assertEqual(raised.exception.code, "ORACLE_WRONG_CENTS")

    def test_floor_triple_passes_silently(self) -> None:
        judge._reject_ceil_near_miss([("e", [181, 0, 0])], self.checks())

    def test_unrelated_triple_passes_silently(self) -> None:
        judge._reject_ceil_near_miss([("e", [5, 5, 5])], self.checks())


if __name__ == "__main__":
    unittest.main()
