"""Unit regressions for the TYPE (S2B-TYPE-001) live judge (no engine).

Drives the pure graph half (`_type_structure`) over decoded-body
fixtures shaped like the served decode_object bodies, and the
execution half (`_type_execute`) with the frozen case driver stubbed.
One case per rejection path (ORACLE_TYPE_NOT_MIGRATED,
ORACLE_SWITCH_NOT_MIGRATED, ORACLE_MISSING_CASE, ORACLE_FAILED_CODE,
ORACLE_TRAP_ARM, ORACLE_BOOL_COMPAT_FIELD, ORACLE_NONDETERMINISTIC) and
one per retired witness-shape pin that must now accept (non-SInt
result, arithmetic arms, a Failed arm mapping the code, a join block,
a shared arm block, a dispatch outside the entry block, any code
literal / status member / integer code type). Integration proofs run
through bench/live/succ_witness_type_full.py.
"""

from __future__ import annotations

import copy
import unittest
from pathlib import Path
from unittest import mock

from bench.fixtures import sley2_live_judge as judge

STATUS = "65" * 32
SWITCH = "6b" * 32
PARAM = "6c" * 32
ENTRY = "6d" * 32
LEAF = "6e" * 32
TYPEDEF = "a0" * 32
QUEUED, RUNNING, SUCCEEDED, FAILED = ("51" * 32, "52" * 32, "53" * 32, "54" * 32)
BLOCK_A, BLOCK_B, BLOCK_C, JOIN = "b1" * 32, "b2" * 32, "b3" * 32, "b4" * 32
P_CODE, P_JOIN = "c1" * 32, "c2" * 32
K0, K1, K2 = "d0" * 32, "d1" * 32, "d2" * 32
OP_6E, OP_A, OP_B = "e0" * 32, "e1" * 32, "e2" * 32
SINT = {"variant": "SInt", "value": 64}
UINT = {"variant": "UInt", "value": 64}
BOOL = {"variant": "Bool"}
ARITH = {"variant": "BuiltinFailure", "value": "ArithmeticError"}

MANIFEST = {
    "entities": {"status": STATUS, "switch": SWITCH, "switch_param": PARAM,
                 "switch_entry": ENTRY, "switch_leaf": LEAF},
    "targets": [STATUS, SWITCH, PARAM, ENTRY, LEAF],
    "judge": {"flow": "type-variant", "status": "status", "switch": "switch",
              "variant_cases": 4, "exhaustive": True},
}


def named(definition: str = TYPEDEF) -> dict:
    return {"variant": "Named", "value": {"definition": definition, "arguments": []}}


def par(ref: str) -> dict:
    return {"variant": "Parameter", "value": ref}


def res(ref: str) -> dict:
    return {"variant": "OperationResult", "value": {"operation": ref, "result_index": 0}}


def ret(value: dict) -> dict:
    return {"variant": "Return", "value": {"value": value}}


def branch(target: str, value: dict) -> dict:
    return {"variant": "Branch", "value": {"edge": {"target": target, "arguments": [value]}}}


def block(params: list, ops: list, terminator: dict, reach: str = "Required") -> dict:
    return {"kind": 7, "body": {"function": SWITCH, "parameters": params,
                                "operations": ops, "terminator": terminator,
                                "reachability": reach}}


def case(member: str, target: str, payload: bool = False) -> dict:
    return {"case_key": {"variant": "Member", "value": member},
            "edge": {"target": target,
                     "arguments": [{"variant": "CasePayload"}] if payload else []}}


def variant_switch(cases: list, selector: str = PARAM) -> dict:
    return {"variant": "VariantSwitch", "value": {"value": par(selector), "cases": cases}}


def int_value(value_type: dict, value: int) -> dict:
    return {"value_type": value_type, "data": {"variant": value_type["variant"], "value": value}}


def status_value(member: str = FAILED, code: dict | None = None) -> dict:
    payload = ({"variant": "Some", "value": code} if code is not None
               else {"variant": "None"})
    return {"kind": 9, "body": {"value": {"value_type": named(), "data": {
        "variant": "Variant", "value": {"definition": TYPEDEF, "member_id": member,
                                        "payload": payload}}}}}


def positive(code_type: dict = SINT) -> tuple[dict, set[str]]:
    """The const design: unit arms return constants, Failed returns
    its code (the historical positive witness shape)."""

    bodies = {
        STATUS: status_value(FAILED, int_value(code_type, 7)),
        TYPEDEF: {"kind": 4, "body": {"type_parameters": [], "form": {
            "variant": "Variant", "value": [
                {"member_id": QUEUED, "payload_type": {"variant": "None"}},
                {"member_id": RUNNING, "payload_type": {"variant": "None"}},
                {"member_id": SUCCEEDED, "payload_type": {"variant": "None"}},
                {"member_id": FAILED, "payload_type": {"variant": "Some",
                                                       "value": code_type}}]},
            "invariants": [], "visibility": "Private"}},
        PARAM: {"kind": 6, "body": {"owner": SWITCH, "role": "Function", "ordinal": 0,
                                    "value_type": named()}},
        SWITCH: {"kind": 5, "body": {"type_parameters": [], "parameters": [PARAM],
                                     "result_type": dict(SINT), "effects": [],
                                     "entry_block": ENTRY,
                                     "blocks": [ENTRY, LEAF, BLOCK_A, BLOCK_B, BLOCK_C],
                                     "contracts": [], "visibility": "Private"}},
        ENTRY: block([], [], variant_switch([
            case(QUEUED, LEAF), case(RUNNING, BLOCK_A), case(SUCCEEDED, BLOCK_B),
            case(FAILED, BLOCK_C, payload=True)])),
        LEAF: block([], [OP_6E], ret(res(OP_6E))),
        BLOCK_A: block([], [OP_A], ret(res(OP_A))),
        BLOCK_B: block([], [OP_B], ret(res(OP_B))),
        BLOCK_C: block([P_CODE], [], ret(par(P_CODE))),
        P_CODE: {"kind": 6, "body": {"owner": BLOCK_C, "role": "Block", "ordinal": 0,
                                     "value_type": dict(code_type)}},
        K0: {"kind": 9, "body": {"value": int_value(SINT, 0)}},
        K1: {"kind": 9, "body": {"value": int_value(SINT, 1)}},
        K2: {"kind": 9, "body": {"value": int_value(SINT, 2)}},
    }
    for op_id, blk, const in ((OP_6E, LEAF, K0), (OP_A, BLOCK_A, K1), (OP_B, BLOCK_B, K2)):
        bodies[op_id] = {"kind": 8, "body": {
            "block": blk, "ordinal": 0, "opcode": 1, "operands": [],
            "result_types": [dict(SINT)], "immediate": {"variant": "Entity", "value": const}}}
    fresh = {TYPEDEF, BLOCK_A, BLOCK_B, BLOCK_C, P_CODE, K0, K1, K2, OP_6E, OP_A, OP_B}
    return bodies, fresh


def structure(bodies: dict, fresh: set[str], manifest: dict | None = None) -> dict:
    return judge._type_structure(manifest or MANIFEST, bodies, fresh)


class TypeJudgeTestCase(unittest.TestCase):
    def assertRejects(self, code: str, bodies: dict, fresh: set[str]) -> str:
        with self.assertRaises(judge.JudgeRejection) as caught:
            structure(bodies, fresh)
        self.assertEqual(caught.exception.code, code, caught.exception.detail)
        return caught.exception.detail

    def assertAccepts(self, bodies: dict, fresh: set[str]) -> dict:
        plan = structure(bodies, fresh)
        self.assertEqual(plan["members"], [QUEUED, RUNNING, SUCCEEDED, FAILED])
        self.assertEqual(plan["failed"], FAILED)
        self.assertEqual(plan["switch"], SWITCH)
        return plan


class RejectionPaths(TypeJudgeTestCase):
    def test_positive_fixture_accepts(self) -> None:
        self.assertAccepts(*positive())

    def test_status_not_named_is_type_not_migrated(self) -> None:
        bodies, fresh = positive()
        bodies[STATUS] = {"kind": 9, "body": {"value": int_value(SINT, 1)}}
        self.assertRejects("ORACLE_TYPE_NOT_MIGRATED", bodies, fresh)

    def test_three_member_typedef_is_type_not_migrated(self) -> None:
        bodies, fresh = positive()
        bodies[TYPEDEF]["body"]["form"]["value"].pop(0)
        self.assertRejects("ORACLE_TYPE_NOT_MIGRATED", bodies, fresh)

    def test_two_coded_members_is_type_not_migrated(self) -> None:
        bodies, fresh = positive()
        bodies[TYPEDEF]["body"]["form"]["value"][0]["payload_type"] = {
            "variant": "Some", "value": dict(SINT)}
        self.assertRejects("ORACLE_TYPE_NOT_MIGRATED", bodies, fresh)

    def test_record_status_type_is_type_not_migrated(self) -> None:
        bodies, fresh = positive()
        bodies[TYPEDEF]["body"]["form"] = {"variant": "Record", "value": []}
        self.assertRejects("ORACLE_TYPE_NOT_MIGRATED", bodies, fresh)

    def test_param_not_jobstate_is_type_not_migrated(self) -> None:
        bodies, fresh = positive()
        bodies[PARAM]["body"]["value_type"] = dict(SINT)
        self.assertRejects("ORACLE_TYPE_NOT_MIGRATED", bodies, fresh)

    def test_no_variant_switch_is_switch_not_migrated(self) -> None:
        bodies, fresh = positive()
        bodies[ENTRY] = block([], [], branch(LEAF, par(PARAM)))
        self.assertRejects("ORACLE_SWITCH_NOT_MIGRATED", bodies, fresh)

    def test_switch_on_other_value_is_switch_not_migrated(self) -> None:
        bodies, fresh = positive()
        bodies[ENTRY]["body"]["terminator"] = variant_switch(
            bodies[ENTRY]["body"]["terminator"]["value"]["cases"], selector=P_CODE)
        self.assertRejects("ORACLE_SWITCH_NOT_MIGRATED", bodies, fresh)

    def test_extra_non_bool_parameter_is_switch_not_migrated(self) -> None:
        bodies, fresh = positive()
        extra = "f1" * 32
        bodies[SWITCH]["body"]["parameters"] = [PARAM, extra]
        bodies[extra] = {"kind": 6, "body": {"owner": SWITCH, "role": "Function",
                                             "ordinal": 1, "value_type": dict(SINT)}}
        self.assertRejects("ORACLE_SWITCH_NOT_MIGRATED", bodies, fresh | {extra})

    def test_three_cases_is_missing_case(self) -> None:
        bodies, fresh = positive()
        bodies[ENTRY]["body"]["terminator"]["value"]["cases"].pop(0)
        self.assertRejects("ORACLE_MISSING_CASE", bodies, fresh)

    def test_builtin_case_key_is_missing_case(self) -> None:
        bodies, fresh = positive()
        bodies[ENTRY]["body"]["terminator"]["value"]["cases"][0]["case_key"] = {
            "variant": "Builtin", "value": "None"}
        self.assertRejects("ORACLE_MISSING_CASE", bodies, fresh)

    def test_unsorted_cases_is_missing_case(self) -> None:
        bodies, fresh = positive()
        cases = bodies[ENTRY]["body"]["terminator"]["value"]["cases"]
        cases[0], cases[1] = cases[1], cases[0]
        self.assertRejects("ORACLE_MISSING_CASE", bodies, fresh)

    def test_reachable_non_required_arm_is_missing_case(self) -> None:
        bodies, fresh = positive()
        bodies[BLOCK_A]["body"]["reachability"] = "ExplicitlyUnreachable"
        self.assertRejects("ORACLE_MISSING_CASE", bodies, fresh)

    def test_edge_outside_function_is_missing_case(self) -> None:
        bodies, fresh = positive()
        bodies[SWITCH]["body"]["blocks"].remove(BLOCK_B)
        self.assertRejects("ORACLE_MISSING_CASE", bodies, fresh)

    def test_failed_edge_dropping_code_is_failed_code(self) -> None:
        bodies, fresh = positive()
        bodies[ENTRY]["body"]["terminator"]["value"]["cases"][3]["edge"]["arguments"] = []
        bodies[BLOCK_C] = block([], [OP_6E], ret(res(OP_6E)))
        self.assertRejects("ORACLE_FAILED_CODE", bodies, fresh)

    def test_failed_status_null_payload_is_failed_code(self) -> None:
        bodies, fresh = positive()
        bodies[STATUS] = status_value(FAILED, None)
        self.assertRejects("ORACLE_FAILED_CODE", bodies, fresh)

    def test_no_coded_member_is_failed_code(self) -> None:
        bodies, fresh = positive()
        bodies[TYPEDEF]["body"]["form"]["value"][3]["payload_type"] = {"variant": "None"}
        bodies[STATUS] = status_value(QUEUED)
        self.assertRejects("ORACLE_FAILED_CODE", bodies, fresh)

    def test_non_integer_code_is_failed_code(self) -> None:
        bodies, fresh = positive()
        bodies[TYPEDEF]["body"]["form"]["value"][3]["payload_type"] = {
            "variant": "Some", "value": {"variant": "Unit"}}
        self.assertRejects("ORACLE_FAILED_CODE", bodies, fresh)

    def test_trap_arm_is_trap_arm(self) -> None:
        bodies, fresh = positive()
        bodies[BLOCK_C] = block([P_CODE], [], {"variant": "Trap", "value": {
            "code": "Unreachable", "payload": {"variant": "None"}}})
        self.assertRejects("ORACLE_TRAP_ARM", bodies, fresh)

    def test_status_still_bool_is_bool_compat(self) -> None:
        bodies, fresh = positive()
        bodies[STATUS] = {"kind": 9, "body": {"value": {
            "value_type": dict(BOOL), "data": {"variant": "Bool", "value": True}}}}
        self.assertRejects("ORACLE_BOOL_COMPAT_FIELD", bodies, fresh)

    def test_param_still_bool_is_bool_compat(self) -> None:
        bodies, fresh = positive()
        bodies[PARAM]["body"]["value_type"] = dict(BOOL)
        self.assertRejects("ORACLE_BOOL_COMPAT_FIELD", bodies, fresh)

    def test_switch_result_bool_is_bool_compat(self) -> None:
        bodies, fresh = positive()
        bodies[SWITCH]["body"]["result_type"] = {"variant": "Option", "value": dict(BOOL)}
        self.assertRejects("ORACLE_BOOL_COMPAT_FIELD", bodies, fresh)

    def test_fresh_bool_constant_is_bool_compat(self) -> None:
        bodies, fresh = positive()
        flag = "f2" * 32
        bodies[flag] = {"kind": 9, "body": {"value": {
            "value_type": dict(BOOL), "data": {"variant": "Bool", "value": False}}}}
        self.assertIn(flag[:16], self.assertRejects(
            "ORACLE_BOOL_COMPAT_FIELD", bodies, fresh | {flag}))

    def test_second_bool_switch_parameter_is_bool_compat(self) -> None:
        bodies, fresh = positive()
        flag = "f3" * 32
        bodies[SWITCH]["body"]["parameters"] = [PARAM, flag]
        bodies[flag] = {"kind": 6, "body": {"owner": SWITCH, "role": "Function",
                                            "ordinal": 1, "value_type": dict(BOOL)}}
        self.assertRejects("ORACLE_BOOL_COMPAT_FIELD", bodies, fresh | {flag})

    def test_bool_threaded_into_block_parameter_is_bool_compat(self) -> None:
        bodies, fresh = positive()
        flag = "f4" * 32
        bodies[BLOCK_A]["body"]["parameters"] = [flag]
        bodies[flag] = {"kind": 6, "body": {"owner": BLOCK_A, "role": "Block",
                                            "ordinal": 0, "value_type": dict(BOOL)}}
        self.assertRejects("ORACLE_BOOL_COMPAT_FIELD", bodies, fresh | {flag})

    def test_bool_field_in_fresh_record_is_bool_compat(self) -> None:
        bodies, fresh = positive()
        record = "f5" * 32
        bodies[record] = {"kind": 4, "body": {"type_parameters": [], "form": {
            "variant": "Record", "value": [
                {"member_id": "a1" * 32, "value_type": named(), "visibility": "Private"},
                {"member_id": "a2" * 32, "value_type": dict(BOOL), "visibility": "Private"},
            ]}, "invariants": [], "visibility": "Private"}}
        self.assertRejects("ORACLE_BOOL_COMPAT_FIELD", bodies, fresh | {record})

    def test_bool_global_is_bool_compat(self) -> None:
        bodies, fresh = positive()
        glob = "f6" * 32
        bodies[glob] = {"kind": 10, "body": {"value_type": dict(BOOL),
                                             "initializer": K0, "visibility": "Private"}}
        self.assertRejects("ORACLE_BOOL_COMPAT_FIELD", bodies, fresh | {glob})

    def test_unrelated_base_bool_function_is_not_scanned(self) -> None:
        # The base comparator (0x66, Bool result, untouched) is neither
        # fresh nor a target: not a status binding.
        bodies, fresh = positive()
        bodies["66" * 32] = {"kind": 5, "body": {"parameters": [], "result_type": dict(BOOL)}}
        self.assertAccepts(bodies, fresh)

    def test_param_role_is_scanned_without_targets(self) -> None:
        bodies, fresh = positive()
        bodies[PARAM]["body"]["value_type"] = dict(BOOL)
        manifest = copy.deepcopy(MANIFEST)
        manifest["targets"] = [STATUS, SWITCH]
        with self.assertRaises(judge.JudgeRejection) as caught:
            structure(bodies, fresh, manifest)
        self.assertEqual(caught.exception.code, "ORACLE_BOOL_COMPAT_FIELD")

    def test_missing_param_role_is_harness_error(self) -> None:
        bodies, fresh = positive()
        manifest = copy.deepcopy(MANIFEST)
        del manifest["entities"]["switch_param"]
        with self.assertRaises(judge.JudgeHarnessError):
            structure(bodies, fresh, manifest)


class RetiredPinsAccept(TypeJudgeTestCase):
    """Correct migrations the 0b26c39c/2c97c32f fixture-tier pins rejected."""

    def test_non_sint_result_accepts(self) -> None:
        bodies, fresh = positive(code_type=UINT)
        bodies[SWITCH]["body"]["result_type"] = dict(UINT)
        self.assertAccepts(bodies, fresh)

    def test_result_typed_switch_accepts(self) -> None:
        bodies, fresh = positive()
        bodies[SWITCH]["body"]["result_type"] = {
            "variant": "Result", "value": {"ok": dict(SINT), "error": ARITH}}
        self.assertAccepts(bodies, fresh)

    def test_arithmetic_arms_accept(self) -> None:
        bodies, fresh = positive()
        add = "e5" * 32
        bodies[add] = {"kind": 8, "body": {
            "block": BLOCK_A, "ordinal": 1, "opcode": 64,
            "operands": [res(OP_A), res(OP_A)], "result_types": [dict(SINT)],
            "immediate": {"variant": "None"}}}
        bodies[BLOCK_A] = block([], [OP_A, add], ret(res(add)))
        self.assertAccepts(bodies, fresh | {add})

    def test_failed_arm_mapping_code_accepts(self) -> None:
        bodies, fresh = positive()
        mapped = "e6" * 32
        bodies[mapped] = {"kind": 8, "body": {
            "block": BLOCK_C, "ordinal": 0, "opcode": 64,
            "operands": [par(P_CODE), par(P_CODE)], "result_types": [dict(SINT)],
            "immediate": {"variant": "None"}}}
        bodies[BLOCK_C] = block([P_CODE], [mapped], ret(res(mapped)))
        self.assertAccepts(bodies, fresh | {mapped})

    def test_failed_arm_fixed_value_accepts(self) -> None:
        bodies, fresh = positive()
        bodies[BLOCK_C] = block([P_CODE], [OP_B], ret(res(OP_B)))
        self.assertAccepts(bodies, fresh)

    def test_join_block_accepts(self) -> None:
        bodies, fresh = positive()
        bodies[SWITCH]["body"]["blocks"].append(JOIN)
        bodies[LEAF] = block([], [OP_6E], branch(JOIN, res(OP_6E)))
        bodies[BLOCK_A] = block([], [OP_A], branch(JOIN, res(OP_A)))
        bodies[BLOCK_B] = block([], [OP_B], branch(JOIN, res(OP_B)))
        bodies[BLOCK_C] = block([P_CODE], [], branch(JOIN, par(P_CODE)))
        bodies[JOIN] = block([P_JOIN], [], ret(par(P_JOIN)))
        bodies[P_JOIN] = {"kind": 6, "body": {"owner": JOIN, "role": "Block",
                                              "ordinal": 0, "value_type": dict(SINT)}}
        self.assertAccepts(bodies, fresh | {JOIN, P_JOIN})

    def test_shared_unit_arm_block_accepts(self) -> None:
        bodies, fresh = positive()
        bodies[ENTRY]["body"]["terminator"] = variant_switch([
            case(QUEUED, LEAF), case(RUNNING, LEAF), case(SUCCEEDED, LEAF),
            case(FAILED, BLOCK_C, payload=True)])
        bodies[SWITCH]["body"]["blocks"] = [ENTRY, LEAF, BLOCK_C]
        self.assertAccepts(bodies, fresh)

    def test_dispatch_outside_entry_accepts(self) -> None:
        bodies, fresh = positive()
        pre = "b5" * 32
        dispatch = copy.deepcopy(bodies[ENTRY])
        bodies[pre] = dispatch
        bodies[ENTRY] = block([], [], {"variant": "Branch", "value": {"edge": {
            "target": pre, "arguments": []}}})
        bodies[SWITCH]["body"]["blocks"].append(pre)
        self.assertAccepts(bodies, fresh | {pre})

    def test_unreachable_dead_block_is_ignored(self) -> None:
        bodies, fresh = positive()
        dead = "b6" * 32
        bodies[dead] = block([], [], ret(par(PARAM)), reach="ExplicitlyUnreachable")
        bodies[SWITCH]["body"]["blocks"].append(dead)
        self.assertAccepts(bodies, fresh | {dead})

    def test_any_code_literal_accepts(self) -> None:
        bodies, fresh = positive()
        bodies[STATUS] = status_value(FAILED, int_value(SINT, 8))
        self.assertAccepts(bodies, fresh)

    def test_any_status_member_accepts(self) -> None:
        for member in (QUEUED, RUNNING, SUCCEEDED):
            bodies, fresh = positive()
            bodies[STATUS] = status_value(member)
            self.assertAccepts(bodies, fresh)

    def test_unsigned_code_accepts(self) -> None:
        self.assertAccepts(*positive(code_type=UINT))

    def test_unit_member_with_payload_rejects(self) -> None:
        bodies, fresh = positive()
        bodies[STATUS] = status_value(QUEUED, int_value(SINT, 1))
        self.assertRejects("ORACLE_TYPE_NOT_MIGRATED", bodies, fresh)


def driver_reply(values: list) -> dict:
    return {"ok": True, "cases": [
        {"ok": True, "value": value, "instructions": 3, "fuel": 3}
        for value in values]}


class ExecutionHalf(unittest.TestCase):
    PLAN = {"switch": SWITCH, "typedef": TYPEDEF,
            "members": [QUEUED, RUNNING, SUCCEEDED, FAILED], "failed": FAILED}

    def test_all_four_members_execute_twice(self) -> None:
        values = [{"SInt": "0"}, {"SInt": "1"}, {"SInt": "2"}, {"SInt": "7"}]
        with mock.patch.object(judge, "_run_driver",
                               return_value=driver_reply(values)) as run:
            self.assertEqual(judge._type_execute(Path("/nonexistent"), self.PLAN), values)
        self.assertEqual(run.call_count, 2)
        _, function, cases = run.call_args.args
        self.assertEqual(function, SWITCH)
        self.assertEqual(cases, [
            {"inputs": [{"member": QUEUED}]}, {"inputs": [{"member": RUNNING}]},
            {"inputs": [{"member": SUCCEEDED}]},
            {"inputs": [{"member": FAILED,
                         "payload": {"value": judge.TYPE_EXEC_FAILED_CODE}}]}])

    def test_trapping_member_is_missing_case(self) -> None:
        reply = driver_reply([{"SInt": "0"}] * 4)
        reply["cases"][3] = {"ok": True, "value": {"non_success": "Trap(Unreachable)"}}
        with mock.patch.object(judge, "_run_driver", return_value=reply):
            with self.assertRaises(judge.JudgeRejection) as caught:
                judge._type_execute(Path("/nonexistent"), self.PLAN)
        self.assertEqual(caught.exception.code, "ORACLE_MISSING_CASE")

    def test_engine_error_is_missing_case(self) -> None:
        reply = driver_reply([{"SInt": "0"}] * 4)
        reply["cases"][1] = {"ok": False, "code": "VM_FUEL_EXHAUSTED"}
        with mock.patch.object(judge, "_run_driver", return_value=reply):
            with self.assertRaises(judge.JudgeRejection) as caught:
                judge._type_execute(Path("/nonexistent"), self.PLAN)
        self.assertEqual(caught.exception.code, "ORACLE_MISSING_CASE")

    def test_differing_runs_are_nondeterministic(self) -> None:
        first = driver_reply([{"SInt": "0"}] * 4)
        second = driver_reply([{"SInt": "0"}] * 3 + [{"SInt": "8"}])
        with mock.patch.object(judge, "_run_driver", side_effect=[first, second]):
            with self.assertRaises(judge.JudgeRejection) as caught:
                judge._type_execute(Path("/nonexistent"), self.PLAN)
        self.assertEqual(caught.exception.code, "ORACLE_NONDETERMINISTIC")

    def test_short_driver_reply_is_harness_error(self) -> None:
        with mock.patch.object(judge, "_run_driver",
                               return_value=driver_reply([{"SInt": "0"}])):
            with self.assertRaises(judge.JudgeHarnessError):
                judge._type_execute(Path("/nonexistent"), self.PLAN)

    def test_full_flow_reports_executed_values(self) -> None:
        bodies, fresh = positive()
        pre = {STATUS: "o1", SWITCH: "o2", PARAM: "o3", ENTRY: "o4", LEAF: "o5"}
        post = dict(pre, **{entity: "n-" + entity[:4] for entity in fresh})
        values = [{"SInt": "0"}, {"SInt": "1"}, {"SInt": "2"}, {"SInt": "7"}]
        with mock.patch.object(judge, "_type_current_bodies", return_value=bodies), \
                mock.patch.object(judge, "_run_driver", return_value=driver_reply(values)):
            suffix = judge._judge_type_variant(object(), MANIFEST, Path("/nonexistent"),
                                               pre, post)
        self.assertTrue(suffix.startswith("type exec 5151="), suffix)
        self.assertIn('5454={"SInt":"7"}', suffix)


if __name__ == "__main__":
    unittest.main()
