"""Unit regressions for CREATE entry fail-closed classification.

Drives ``_judge_create_entry`` directly with a stubbed body decoder
and stubbed native observations (no engine needed): every
non-accepting shape must classify explicitly, and only a correctly
decoded expected value may pass. Integration proofs (real entry
executing Ok(2681) through the repaired driver; valid structural
alternative accepting) run via succ_witness_create pos/alt_order.
"""

from __future__ import annotations

import unittest
from pathlib import Path
from unittest import mock

from bench.fixtures import sley2_live_judge as judge

ROLES = {"subtotal": "E1", "tax_mul": "E2", "tax_div": "E3",
         "total": "E4"}
COMPOSITION = {"subtotal": [25, 100], "tax_mul": [50, 35],
               "tax_div": [70, 4], "total": [2500, 181]}
WANT_TOTAL = 2681
WANT = {"Result": {"Ok": {"SInt": "2681"}}}
ENTRY = "EN"
PARAMS8 = ["P1", "P2", "P3", "P4", "P5", "P6", "P7", "P8"]


def param(pid: str) -> dict:
    return {"variant": "Parameter", "value": pid}


def call_op(callee: str, left: str, right: str) -> dict:
    return {"opcode": 112,
            "immediate": {"variant": "Function",
                          "value": {"function": callee,
                                    "type_arguments": []}},
            "operands": [param(left), param(right)]}


CONSISTENT_CALLS = [call_op("E1", "P1", "P2"), call_op("E2", "P3", "P4"),
                    call_op("E3", "P5", "P6"), call_op("E4", "P7", "P8")]
CONSISTENT_VECTOR = [(25, 100, 50, 35, 70, 4, 2500, 181)]


class EntryCase(unittest.TestCase):
    def run_entry(self, calls, observations, *, roles=None,
                  composition=None, ties=None, params=None,
                  blocks=None):
        ops = {f"O{i}": call for i, call in enumerate(calls)}
        bodies = {"B1": {"operations": list(ops)}}
        bodies.update(ops)
        body = {"parameters": PARAMS8 if params is None else params,
                "blocks": ["B1"] if blocks is None else blocks}

        def fake_decode(session, scratch_ws, entity, current):
            return bodies[entity]

        with mock.patch.object(judge, "_decode_body",
                               side_effect=fake_decode):
            return judge._judge_create_entry(
                ENTRY, body, roles or dict(ROLES), ties or [],
                composition or dict(COMPOSITION), WANT_TOTAL,
                dict(WANT), object(), Path("/nonexistent"),
                set(), observations)

    def test_consistent_wiring_correct_value_passes(self) -> None:
        seen = {}

        def observations(entry, vectors):
            seen["vector"] = vectors
            return [{"ok": True, "value": dict(WANT)}]

        self.run_entry(CONSISTENT_CALLS, observations)
        self.assertEqual(seen["vector"], CONSISTENT_VECTOR)

    def test_wrong_value_rejects_mismatch(self) -> None:
        wrong = {"Result": {"Ok": {"SInt": "9999"}}}
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.run_entry(
                CONSISTENT_CALLS,
                lambda e, v: [{"ok": True, "value": wrong}])
        self.assertEqual(raised.exception.code, "ORACLE_CREATE_MISMATCH")

    def test_failed_execution_is_unexecutable(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.run_entry(
                CONSISTENT_CALLS,
                lambda e, v: [{"ok": False, "code": "VM_X"}])
        self.assertEqual(raised.exception.code,
                         "ORACLE_CREATE_UNEXECUTABLE")

    def test_driver_error_is_unexecutable(self) -> None:
        def boom(entry, vectors):
            raise RuntimeError("driver exploded")

        with self.assertRaises(judge.JudgeRejection) as raised:
            self.run_entry(CONSISTENT_CALLS, boom)
        self.assertEqual(raised.exception.code,
                         "ORACLE_CREATE_UNEXECUTABLE")

    def test_malformed_envelope_is_harness_failure(self) -> None:
        with self.assertRaises(judge.JudgeHarnessError):
            self.run_entry(
                CONSISTENT_CALLS,
                lambda e, v: [{"ok": True, "value": "not-a-dict"}])

    def test_no_params_is_unmapped(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.run_entry(CONSISTENT_CALLS, lambda e, v: [], params=[])
        self.assertEqual(raised.exception.code, "ORACLE_CREATE_UNMAPPED")

    def test_non_parameter_operands_unmapped(self) -> None:
        bad = {"opcode": 112,
               "immediate": {"variant": "Function",
                             "value": {"function": "E1",
                                       "type_arguments": []}},
               "operands": [{"variant": "Entity", "value": "C1"},
                            param("P2")]}
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.run_entry([bad], lambda e, v: [])
        self.assertEqual(raised.exception.code, "ORACLE_CREATE_UNMAPPED")

    def test_call_outside_role_set_unmapped(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.run_entry([call_op("E9", "P1", "P2")], lambda e, v: [])
        self.assertEqual(raised.exception.code, "ORACLE_CREATE_UNMAPPED")

    def test_no_mapped_calls_unmapped(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.run_entry(
                [{"opcode": 66, "immediate": {"variant": "None"},
                  "operands": []}], lambda e, v: [])
        self.assertEqual(raised.exception.code, "ORACLE_CREATE_UNMAPPED")

    def test_conflicting_shared_params_reject(self) -> None:
        # P1 demanded 25 by subtotal and 50 by tax_mul: the wiring
        # cannot compute the corpus total under any labeling.
        calls = [call_op("E1", "P1", "P2"), call_op("E2", "P1", "P4"),
                 call_op("E3", "P5", "P6"), call_op("E4", "P7", "P8")]
        with self.assertRaises(judge.JudgeRejection) as raised:
            self.run_entry(calls, lambda e, v: [])
        self.assertEqual(raised.exception.code, "ORACLE_CREATE_MISMATCH")

    def test_tie_labeling_result_decides_not_order(self) -> None:
        # Behaviorally tied roles admit two consistent labelings with
        # different input vectors. Only the labeling whose execution
        # decodes to the expected value may pass — never scan order.
        calls = [call_op("E1", "P1", "P2"), call_op("E2", "P3", "P4"),
                 call_op("E3", "P5", "P6"), call_op("E4", "P7", "P8")]
        seen = []

        def observations(entry, vectors):
            seen.append(vectors[0])
            if vectors[0][:4] == (50, 35, 25, 100):
                return [{"ok": True, "value": dict(WANT)}]
            return [{"ok": True,
                     "value": {"Result": {"Ok": {"SInt": "0"}}}}]

        self.run_entry(calls, observations,
                       ties=[("subtotal", "tax_mul")])
        # Both labelings executed; the swapped one decided.
        self.assertEqual(seen, [(25, 100, 50, 35, 70, 4, 2500, 181),
                                (50, 35, 25, 100, 70, 4, 2500, 181)])

    def test_malformed_composition_is_harness_failure(self) -> None:
        with self.assertRaises(judge.JudgeHarnessError):
            self.run_entry(CONSISTENT_CALLS, lambda e, v: [],
                           composition={"subtotal": "nope"})


if __name__ == "__main__":
    unittest.main()
