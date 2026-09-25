"""Unit regressions for the typed CREATE judge (no engine needed).

Drives the pure discovery/normalization helpers directly: Money and
LineItem shape recognition, entry-contract matching, frozen spec
parsing, driver-case construction, and submitted-test boundary
coverage. Every malformed shape must fail closed (harness failure for
judge-side config, rejection with the distinguishing code for
candidate shapes), and only exact frozen coverage may pass.
Integration proofs (genuine invoice program accepted end to end;
wrong dataflow, swallowed overflow, missing types/tests, and wrong
test expectations rejected with distinguishing codes) run via
succ_witness_create variants.
"""

from __future__ import annotations

import unittest

from bench.fixtures import sley2_live_judge as judge

MONEY = "aa" * 32
LINE = "bb" * 32
OTHER = "cc" * 32
CENTS = "c1" * 32
QTY = "c2" * 32
UNIT = "c3" * 32
SINT64 = {"variant": "SInt", "value": 64}
I64MAX = 9223372036854775807


def record_body(members: list) -> dict:
    return {"form": {"variant": "Record", "value": [
        {"member_id": member, "value_type": dict(SINT64)}
        for member in members]}}


def named(definition: str) -> dict:
    return {"variant": "Named",
            "value": {"definition": definition, "arguments": []}}


def entry_body() -> dict:
    return {
        "parameters": ["P0", "P1"],
        "result_type": {
            "variant": "Result",
            "value": {"ok": named(MONEY),
                      "error": {"variant": "BuiltinFailure",
                                "value": "ArithmeticError"}}},
    }


def param_types() -> dict:
    return {
        "P0": {"variant": "Vector", "value": named(LINE)},
        "P1": dict(SINT64),
    }


def spec_judge() -> dict:
    return {"flow": "create", "cases": [
        {"name": "empty", "lines": [], "tax_bp": 725,
         "expect": {"Ok": {"cents": 0}}},
        {"name": "one-line", "lines": [{"quantity": 2,
                                        "unit_cents": 1250}],
         "tax_bp": 725, "expect": {"Ok": {"cents": 2681}}},
        {"name": "overflow",
         "lines": [{"quantity": I64MAX, "unit_cents": 2}],
         "tax_bp": 725,
         "expect": {"Err": {"kind": "Arithmetic", "code": 1}}},
    ]}


def test_entity(lines: list, rate: int, expected: dict) -> dict:
    return {"target": "EN", "inputs": [
        {"value_type": {"variant": "Vector", "value": named(LINE)},
         "data": {"variant": "Sequence", "value": [
             {"value_type": named(LINE),
              "data": {"variant": "Record", "value": {
                  "definition": LINE,
                  "fields": [{"member_id": QTY,
                              "value": {"value_type": dict(SINT64),
                                        "data": {"variant": "SInt",
                                                 "value": quantity}}},
                             {"member_id": UNIT,
                              "value": {"value_type": dict(SINT64),
                                        "data": {"variant": "SInt",
                                                 "value": unit}}}]}}}
             for quantity, unit in lines]}},
        {"value_type": dict(SINT64),
         "data": {"variant": "SInt", "value": rate}}],
        "expected": expected}


def ok_expected(cents: int) -> dict:
    return {"variant": "Value", "value": {
        "value_type": {"variant": "Result", "value": {
            "ok": named(MONEY),
            "error": {"variant": "BuiltinFailure",
                      "value": "ArithmeticError"}}},
        "data": {"variant": "Result", "value": {
            "variant": "Ok", "value": {
                "value_type": named(MONEY),
                "data": {"variant": "Record", "value": {
                    "definition": MONEY,
                    "fields": [{"member_id": CENTS,
                                "value": {
                                    "value_type": dict(SINT64),
                                    "data": {"variant": "SInt",
                                             "value": cents}}}]}}}}}}}


def err_expected(code: int) -> dict:
    return {"variant": "Value", "value": {
        "value_type": {"variant": "Result", "value": {
            "ok": named(MONEY),
            "error": {"variant": "BuiltinFailure",
                      "value": "ArithmeticError"}}},
        "data": {"variant": "Result", "value": {
            "variant": "Err", "value": {
                "value_type": {"variant": "BuiltinFailure",
                               "value": "ArithmeticError"},
                "data": {"variant": "BuiltinFailure", "value": {
                    "kind": "ArithmeticError", "code": code}}}}}}}


class SpecCase(unittest.TestCase):
    def test_valid_spec_parses(self) -> None:
        cases = judge._invoice_spec_cases(spec_judge())
        self.assertEqual([case["name"] for case in cases],
                         ["empty", "one-line", "overflow"])
        self.assertEqual(cases[0]["want"], ("Ok", 0))
        self.assertEqual(cases[1]["want"], ("Ok", 2681))
        self.assertEqual(cases[2]["want"], ("Err", 1))

    def test_missing_cases_is_harness_failure(self) -> None:
        with self.assertRaises(judge.JudgeHarnessError):
            judge._invoice_spec_cases({"flow": "create"})

    def test_wrong_case_names_is_harness_failure(self) -> None:
        spec = spec_judge()
        spec["cases"][0]["name"] = "nil"
        with self.assertRaises(judge.JudgeHarnessError):
            judge._invoice_spec_cases(spec)

    def test_bool_quantity_is_harness_failure(self) -> None:
        spec = spec_judge()
        spec["cases"][1]["lines"] = [{"quantity": True,
                                      "unit_cents": 1250}]
        with self.assertRaises(judge.JudgeHarnessError):
            judge._invoice_spec_cases(spec)


class ShapeCase(unittest.TestCase):
    def test_money_and_line_shapes(self) -> None:
        self.assertTrue(judge._sint_record_shape(
            record_body([CENTS]), 1))
        self.assertTrue(judge._sint_record_shape(
            record_body([QTY, UNIT]), 2))
        self.assertFalse(judge._sint_record_shape(
            record_body([QTY, UNIT]), 1))
        self.assertFalse(judge._sint_record_shape(
            {"form": {"variant": "Variant", "value": []}}, 1))
        self.assertFalse(judge._sint_record_shape({}, 1))

    def test_wrong_field_type_rejected(self) -> None:
        body = {"form": {"variant": "Record", "value": [
            {"member_id": CENTS,
             "value_type": {"variant": "Bool"}}]}}
        self.assertFalse(judge._sint_record_shape(body, 1))

    def test_member_order_preserved(self) -> None:
        self.assertEqual(judge._record_members(record_body([QTY, UNIT])),
                         [QTY, UNIT])

    def test_entry_contract_matches(self) -> None:
        self.assertTrue(judge._entry_shape(
            entry_body(), param_types(), LINE, MONEY))

    def test_entry_wrong_arity_rejected(self) -> None:
        body = entry_body()
        body["parameters"] = ["P0"]
        self.assertFalse(judge._entry_shape(
            body, param_types(), LINE, MONEY))

    def test_entry_bare_result_rejected(self) -> None:
        body = entry_body()
        body["result_type"] = dict(SINT64)
        self.assertFalse(judge._entry_shape(
            body, param_types(), LINE, MONEY))

    def test_entry_wrong_typedef_rejected(self) -> None:
        self.assertFalse(judge._entry_shape(
            entry_body(), param_types(), OTHER, MONEY))
        self.assertFalse(judge._entry_shape(
            entry_body(), param_types(), LINE, OTHER))

    def test_entry_bits_mismatch_rejected(self) -> None:
        params = param_types()
        params["P1"] = {"variant": "SInt", "value": 32}
        self.assertFalse(judge._entry_shape(
            entry_body(), params, LINE, MONEY))


class DriverCase(unittest.TestCase):
    def test_driver_case_shape(self) -> None:
        case = judge._invoice_driver_case([(2, 1250)], 725, QTY, UNIT)
        self.assertEqual(case, {"inputs": [
            {"values": [{"fields": {QTY: {"value": 2},
                                    UNIT: {"value": 1250}}}]},
            {"value": 725}]})

    def test_want_shapes(self) -> None:
        self.assertEqual(
            judge._invoice_want_ok(MONEY, CENTS, 2681),
            {"Result": {"Ok": {"Record": {
                "definition": MONEY,
                "fields": {CENTS: {"SInt": "2681"}}}}}})
        self.assertEqual(
            judge._invoice_want_err(1),
            {"Result": {"Err": {"BuiltinFailure": {
                "kind": "Arithmetic", "code": 1}}}})

    def test_overflow_value_is_unchecked(self) -> None:
        value = {"ok": True, "value": {"Result": {"Ok": {"SInt": "5"}}}}
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._check_overflow_result(
                value, {"code": 1}, "ORACLE_UNCHECKED_ARITHMETIC")
        self.assertEqual(raised.exception.code,
                         "ORACLE_UNCHECKED_ARITHMETIC")


class BoundaryCase(unittest.TestCase):
    def frozen_tests(self) -> list:
        return [
            test_entity([], 725, ok_expected(0)),
            test_entity([(2, 1250)], 725, ok_expected(2681)),
            test_entity([(I64MAX, 2)], 725, err_expected(1)),
        ]

    def test_frozen_coverage_passes(self) -> None:
        required = judge._require_invoice_boundaries(
            self.frozen_tests(), MONEY, CENTS, LINE, QTY, UNIT)
        self.assertEqual(required[((), 725)], ("Ok", 0))
        self.assertEqual(required[(((2, 1250),), 725)], ("Ok", 2681))
        self.assertEqual(required[(((I64MAX, 2),), 725)], ("Err", 1))

    def test_missing_boundary_rejects(self) -> None:
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._require_invoice_boundaries(
                self.frozen_tests()[:2], MONEY, CENTS, LINE, QTY, UNIT)
        self.assertEqual(raised.exception.code, "ORACLE_CASE_MISSING")

    def test_wrong_expectation_rejects(self) -> None:
        tests = self.frozen_tests()
        tests[1] = test_entity([(2, 1250)], 725, ok_expected(2682))
        with self.assertRaises(judge.JudgeRejection) as raised:
            judge._require_invoice_boundaries(
                tests, MONEY, CENTS, LINE, QTY, UNIT)
        self.assertEqual(raised.exception.code, "ORACLE_TEST_MISMATCH")

    def test_failure_code_norm_accepted(self) -> None:
        tests = self.frozen_tests()
        tests[2] = test_entity([(I64MAX, 2)], 725,
                               {"variant": "FailureCode", "value": 1})
        required = judge._require_invoice_boundaries(
            tests, MONEY, CENTS, LINE, QTY, UNIT)
        self.assertEqual(required[(((I64MAX, 2),), 725)], ("Err", 1))

    def test_bool_input_undecodable(self) -> None:
        broken = test_entity([(2, 1250)], 725, ok_expected(2681))
        fields = broken["inputs"][0]["data"]["value"][0]["data"][
            "value"]["fields"]
        fields[0]["value"]["data"]["value"] = True
        self.assertIsNone(judge._invoice_test_inputs(
            broken, LINE, QTY, UNIT))

    def test_wrong_definition_undecodable(self) -> None:
        broken = test_entity([(2, 1250)], 725, ok_expected(2681))
        broken["inputs"][0]["data"]["value"][0]["data"][
            "value"]["definition"] = OTHER
        self.assertIsNone(judge._invoice_test_inputs(
            broken, LINE, QTY, UNIT))

    def test_wrong_money_rejected(self) -> None:
        self.assertIsNone(judge._invoice_test_expected(
            test_entity([(2, 1250)], 725, ok_expected(2681)),
            OTHER, CENTS))

    def test_duplicate_inputs_first_wins(self) -> None:
        tests = self.frozen_tests()
        tests.append(test_entity([(2, 1250)], 725, ok_expected(2682)))
        required = judge._require_invoice_boundaries(
            tests, MONEY, CENTS, LINE, QTY, UNIT)
        self.assertEqual(required[(((2, 1250),), 725)], ("Ok", 2681))


if __name__ == "__main__":
    unittest.main()
