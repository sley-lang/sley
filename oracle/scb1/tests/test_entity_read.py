"""Focused grammar and semantic rejection tests for the entity-read oracle.

Stage A scope: meaningful rejection behavior only (malformed varints and
records, digest integrity, declaration-order and relationship checks, work
arithmetic, negotiation refusals). These tests construct their own tiny
in-memory fixtures through the oracle module; they never read the
hand-authored corpus inputs, never execute refresh, and claim no corpus or
runtime coverage.

Root execution contract (never executed by the author of this file):
  uv run --project oracle/scb1 --frozen python -m unittest discover -s oracle/scb1/tests -v
"""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

import blake3

from sley2_scb1_oracle import entity_read
from sley2_scb1_oracle.codec import encode_record, encode_sized, encode_uvar
from sley2_scb1_oracle.errors import ScbError


EPOCH = bytes([0xC3]) * 32
ID_A = bytes([0xA0]) * 32
ID_B = bytes([0xB0]) * 32


def constant_entity(entity_id: bytes, text: str) -> dict:
    return {
        "id": entity_id.hex(),
        "kind": 9,
        "kind_name": "Constant",
        "body_type": "ConstantBody",
        "body": {
            "value": {
                "value_type": {"variant": "Text"},
                "data": {"variant": "Text", "value": text},
            }
        },
    }


class GrammarRejectionCases(unittest.TestCase):
    def test_nonminimal_uvar_refused(self) -> None:
        reader = entity_read.Reader(b"\x80\x00")
        with self.assertRaisesRegex(ScbError, "SCB_VARINT_NON_MINIMAL"):
            reader.uvar(64)

    def test_truncated_uvar_refused(self) -> None:
        reader = entity_read.Reader(b"\x80")
        with self.assertRaisesRegex(ScbError, "SCB_LENGTH_OVERFLOW"):
            reader.uvar(64)

    def test_uvar_width_overflow_refused(self) -> None:
        reader = entity_read.Reader(b"\x80\x80\x80\x80\x80\x80\x80\x80\x80\x80\x01")
        with self.assertRaisesRegex(ScbError, "SCB_INTEGER_OVERFLOW"):
            reader.uvar(64)

    def test_record_duplicate_refused(self) -> None:
        raw = encode_record([(1, b"x"), (1, b"y")])
        with self.assertRaisesRegex(ScbError, "SCB_FIELD_DUPLICATE"):
            entity_read.parse_record(raw)

    def test_record_reorder_refused(self) -> None:
        raw = encode_record([(2, b"x"), (1, b"y")])
        with self.assertRaisesRegex(ScbError, "SCB_FIELD_ORDER"):
            entity_read.parse_record(raw)

    def test_record_trailing_refused(self) -> None:
        with self.assertRaisesRegex(ScbError, "SCB_TRAILING_BYTES"):
            entity_read.parse_record(encode_record([(1, b"x")]) + b"\x00")

    def test_bounds_flag_enum_refused(self) -> None:
        reader = entity_read.Reader(encode_uvar(9) + encode_sized(b""))
        tag, _payload = reader.union()
        self.assertNotIn(tag, (0, 1))
        with self.assertRaisesRegex(ScbError, "SCB_UNION_INVALID"):
            entity_read.decode_bounds(
                encode_record(
                    [
                        (1, entity_read.build_limits({k: 1 for k in ("max_frame_bytes", "max_entities", "max_edges", "max_depth", "max_response_bytes", "max_work", "max_inflight", "max_sessions")})),
                        (2, encode_uvar(0)),
                        (3, encode_uvar(0)),
                        (4, encode_uvar(0)),
                        (5, encode_uvar(0)),
                        (6, encode_uvar(0)),
                        (7, encode_uvar(9)),
                        (8, encode_uvar(1)),
                    ]
                )
            )

    def test_unsorted_entity_id_set_refused(self) -> None:
        with self.assertRaisesRegex(ScbError, "SCB_MAP_ORDER"):
            entity_read.encode_mutation_value("Set<EntityId>", [ID_B.hex(), ID_A.hex()])


class IntegrityCases(unittest.TestCase):
    def test_changed_body_with_stale_trailer_refused(self) -> None:
        item = entity_read.build_object(constant_entity(ID_A, "sealed"), EPOCH)
        stored = bytearray(bytes.fromhex(item["stored_hex"]))
        stored[60] ^= 1
        with self.assertRaisesRegex(ScbError, "SCB_DIGEST_MISMATCH"):
            entity_read.check_stored_object(bytes(stored), 9, ID_A, EPOCH)

    def test_object_epoch_mismatch_refused(self) -> None:
        item = entity_read.build_object(constant_entity(ID_A, "sealed"), EPOCH)
        with self.assertRaisesRegex(ScbError, "SCB_EPOCH_MISMATCH"):
            entity_read.check_stored_object(bytes.fromhex(item["stored_hex"]), 9, ID_A, bytes([0xE5]) * 32)

    def test_label_round_trip(self) -> None:
        labeled = constant_entity(ID_A, "sealed")
        labeled["label"] = "er2/test/label"
        item = entity_read.build_object(labeled, EPOCH)
        decoded = entity_read.check_stored_object(bytes.fromhex(item["stored_hex"]), 9, ID_A, EPOCH)
        self.assertEqual(decoded["label"], "er2/test/label")


class SignatureCheckCases(unittest.TestCase):
    def _assembly(self) -> tuple:
        func_id = bytes([0xF0]) * 32
        high_id = bytes([0xF1]) * 32
        low_id = bytes([0xF2]) * 32
        func = {
            "id": func_id.hex(),
            "kind": 5,
            "kind_name": "Function",
            "body_type": "FunctionBody",
            "body": {
                "type_parameters": [],
                "parameters": [high_id.hex(), low_id.hex()],
                "result_type": {"variant": "Bool"},
                "effects": [],
                "entry_block": ID_A.hex(),
                "blocks": [ID_A.hex()],
                "contracts": [],
                "visibility": "Private",
            },
        }
        high_type = {"variant": "Option", "value": {"variant": "Text"}}
        low_type = {"variant": "Tuple", "value": [{"variant": "Bool"}, {"variant": "Text"}]}
        high = {
            "id": high_id.hex(),
            "kind": 6,
            "kind_name": "Parameter",
            "body_type": "ParameterBody",
            "body": {"owner": func_id.hex(), "role": "Function", "ordinal": 0, "value_type": high_type},
        }
        low = {
            "id": low_id.hex(),
            "kind": 6,
            "kind_name": "Parameter",
            "body_type": "ParameterBody",
            "body": {"owner": func_id.hex(), "role": "Function", "ordinal": 1, "value_type": low_type},
        }
        objects = [entity_read.build_object(entity, EPOCH) for entity in (func, high, low)]
        metas = [
            {"entity": func_id, "kind": 5},
            {"entity": high_id, "kind": 6},
            {"entity": low_id, "kind": 6},
        ]
        bodies = []
        func_body = entity_read.parse_record(bytes.fromhex(objects[0]["record_hex"]))
        func_union = entity_read.Reader(entity_read.single_field(func_body, 2))
        _tag, func_inner = func_union.union()
        bodies = [func_inner]
        for item in objects[1:]:
            record = entity_read.parse_record(bytes.fromhex(item["record_hex"]))
            union = entity_read.Reader(entity_read.single_field(record, 2))
            _tag, inner = union.union()
            bodies.append(inner)
        return func_id, high_id, low_id, metas, bodies

    def test_declaration_order_accepted(self) -> None:
        func_id, high_id, low_id, metas, bodies = self._assembly()
        high_type = entity_read.encode_mutation_value("TypeExpr", {"variant": "Option", "value": {"variant": "Text"}})
        low_type = entity_read.encode_mutation_value("TypeExpr", {"variant": "Tuple", "value": [{"variant": "Bool"}, {"variant": "Text"}]})
        entity_read.check_signature(metas, bodies, func_id, [high_id, low_id], [high_type, low_type])

    def test_swapped_order_refused(self) -> None:
        func_id, high_id, low_id, metas, bodies = self._assembly()
        high_type = entity_read.encode_mutation_value("TypeExpr", {"variant": "Option", "value": {"variant": "Text"}})
        low_type = entity_read.encode_mutation_value("TypeExpr", {"variant": "Tuple", "value": [{"variant": "Bool"}, {"variant": "Text"}]})
        with self.assertRaisesRegex(entity_read.CheckFailed, "signature"):
            entity_read.check_signature([metas[0], metas[2], metas[1]], [bodies[0], bodies[2], bodies[1]], func_id, [high_id, low_id], [high_type, low_type])

    def test_exact_type_bytes_required(self) -> None:
        func_id, high_id, low_id, metas, bodies = self._assembly()
        wrong = entity_read.encode_mutation_value("TypeExpr", {"variant": "Bool"})
        low_type = entity_read.encode_mutation_value("TypeExpr", {"variant": "Tuple", "value": [{"variant": "Bool"}, {"variant": "Text"}]})
        with self.assertRaisesRegex(entity_read.CheckFailed, "signature"):
            entity_read.check_signature(metas, bodies, func_id, [high_id, low_id], [wrong, low_type])


class AccountingCases(unittest.TestCase):
    def test_work_formula_exact(self) -> None:
        self.assertEqual(entity_read.compute_work(3, 9, 1000, 200000), 1 + 27 + 2000 + 200000)

    def test_work_overflow_refused(self) -> None:
        with self.assertRaises(entity_read.Overflow):
            entity_read.compute_work(1 << 62, 2, 0, (1 << 64) - 1)

    def test_zero_ceiling_refused(self) -> None:
        with self.assertRaisesRegex(entity_read.CheckFailed, "request_range"):
            entity_read.build_request_body(ID_A, ID_B, 0, 200000, 5000000)

    def test_context_mismatch_refused(self) -> None:
        response = {"workspace": ID_A, "root": ID_A, "epoch": ID_A, "session": ID_A, "requested": ID_B}
        context = {"workspace": ID_A.hex(), "root": ID_A.hex(), "content_epoch": ID_A.hex(), "session": (bytes([0x5A]) * 32).hex()}
        with self.assertRaisesRegex(entity_read.CheckFailed, "context"):
            entity_read.check_context(response, context, ID_B)


class NegotiationCases(unittest.TestCase):
    def _hello(self, versions: list, methods: list) -> dict:
        return {
            "protocol_versions": versions,
            "schema_epochs": [EPOCH.hex()],
            "limits": {"max_frame_bytes": 8388608, "max_entities": 1024, "max_edges": 10000, "max_depth": 64, "max_response_bytes": 4194304, "max_work": 10000000, "max_inflight": 16, "max_sessions": 16},
            "methods": methods,
            "features": 1,
            "adapters": [],
            "effects": [],
        }

    def test_reserved_offer_refused(self) -> None:
        with self.assertRaisesRegex(entity_read.CheckFailed, "hello"):
            entity_read.build_hello(self._hello([1, 2], [100, 305]))

    def test_greatest_common_three_refused(self) -> None:
        client = self._hello([1, 3], [100, 200])
        server = self._hello([1, 3], [100, 200])
        with self.assertRaisesRegex(entity_read.CheckFailed, "selection"):
            entity_read.negotiate_versioned(client, server)

    def test_v1_selection_filters_new_tags(self) -> None:
        client = self._hello([1, 2], [100, 306, 307, 900])
        server = self._hello([1], [100, 306, 307, 900])
        selection = entity_read.negotiate_versioned(client, server)
        self.assertEqual(selection["protocol_version"], 1)
        self.assertNotIn(306, selection["methods"])
        self.assertNotIn(307, selection["methods"])
        self.assertIn(900, selection["methods"])

    def test_descriptor_pins_frozen(self) -> None:
        self.assertEqual(entity_read.PROTOCOL_FIELD_SCHEMA_HASH, "d36cce861ab70cbbd021ff3d8020e71eb9e9684773f82734b6f2aa484937df0e")
        self.assertEqual(entity_read.PROTOCOL_DECODER_LIMITS_HASH, "212b293d90646621c2e83caf095c2a21b6864888a3cc98878b7f6ab81ab98fe7")
        self.assertEqual(len(entity_read.protocol_epoch_id()), 32)

    def test_retryability_mapping(self) -> None:
        self.assertEqual(entity_read.retryability("PROTOCOL_LIMIT_EXCEEDED"), 4)
        self.assertEqual(entity_read.retryability("PROTOCOL_PAYLOAD_INVALID"), 1)
        self.assertEqual(entity_read.retryability("QUERY_ROOT_MISMATCH"), 1)


def load_authored_inputs():
    path = Path(__file__).resolve().parents[3] / "conformance" / "entity-read" / "v2" / "inputs.json"
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)


def resplice_stored(stored, mutate):
    preimage = stored[:-32]
    cursor = entity_read.Reader(preimage)
    cursor.take(8)
    cursor.uvar(64)
    cursor.uvar(32)
    cursor.take(32)
    record = cursor.sized(entity_read.MAX_STANDALONE_BYTES)
    cursor.finish()
    head = preimage[: len(preimage) - len(encode_sized(record))]
    new_record = entity_read.encode_fields(mutate(entity_read.parse_record(record)))
    new_preimage = head + encode_sized(new_record)
    return new_preimage + blake3.blake3(entity_read.OBJECT_DOMAIN + new_preimage).digest()


class AuthoredObjectConstructionCases(unittest.TestCase):
    def test_all_authored_objects_build_and_round_trip(self) -> None:
        inputs = load_authored_inputs()
        epoch = bytes.fromhex(inputs["context"]["content_epoch"])
        self.assertEqual(len(inputs["entities"]), 25)
        for ref, ent in inputs["entities"].items():
            with self.subTest(ref=ref):
                try:
                    item = entity_read.build_object(ent, epoch)
                except Exception as error:
                    self.fail(f"build {ref}: {error!r}")
                decoded = entity_read.check_stored_object(bytes.fromhex(item["stored_hex"]), ent["kind"], bytes.fromhex(ent["id"]), epoch)
                self.assertEqual(decoded["kind"], ent["kind"])
                self.assertEqual(decoded["entity_id"], bytes.fromhex(ent["id"]))


class AuthoredSignatureSemanticCases(unittest.TestCase):
    def test_zero_and_multi_signatures_valid_with_derived_lookup(self) -> None:
        inputs = load_authored_inputs()
        normalized = copy.deepcopy(inputs)
        normalized["context"]["lookup_l"] = int(normalized["context"]["root_bindings"]).bit_length() + 1
        self.assertEqual(normalized["context"]["lookup_l"], 10)
        for case_id in ("sig_zero", "sig_multi"):
            with self.subTest(case=case_id):
                case = normalized["cases"][case_id]
                built = entity_read.build_success_case(normalized, case)
                response_body = bytes.fromhex(built["response_body_hex"])
                response = entity_read.decode_response_body(response_body)
                bounds = entity_read.decode_bounds(entity_read.build_bounds(normalized["selected_limits"], len(response_body), built["count_k"]))
                entity_read.check_context(response, normalized["context"], bytes.fromhex(normalized["entities"][case["entity"]]["id"]))
                entity_read.check_counts(response, built["count_k"], len(response_body), bounds)
                entity_read.check_work(response, built["count_k"], int(normalized["context"]["lookup_l"]), built["stored_b"], int(case["request"]["max_response_bytes"]))
                entries = [entity_read.decode_response_entry(raw) for raw in response["entries"]]
                epoch = bytes.fromhex(normalized["context"]["content_epoch"])
                bodies = [entity_read.check_stored_object(entry["stored"], entry["kind"], entry["entity"], epoch)["body"] for entry in entries]
                sig = case["signature"]
                expected_types = [entity_read.encode_mutation_value("TypeExpr", normalized["signature_types"][ref]) for ref in sig["parameters"]]
                entity_read.check_signature(
                    entries,
                    bodies,
                    bytes.fromhex(normalized["entities"][sig["function"]]["id"]),
                    [bytes.fromhex(normalized["entities"][ref]["id"]) for ref in sig["parameters"]],
                    expected_types,
                )
                problems = []
                entity_read.semantic_check(normalized, case, built, problems)
                self.assertEqual(problems, [])


class WorkDerivationCases(unittest.TestCase):
    def test_single_and_multi_work_uses_derived_lookup(self) -> None:
        inputs = load_authored_inputs()
        for case_id, want_k in (("ver_ws", 1), ("sig_multi", 3)):
            with self.subTest(case=case_id):
                case = inputs["cases"][case_id]
                built = entity_read.build_success_case(inputs, case)
                derived_l = int(inputs["context"]["root_bindings"]).bit_length() + 1
                self.assertEqual(derived_l, 10)
                self.assertEqual(built["count_k"], want_k)
                expected = entity_read.compute_work(built["count_k"], derived_l, built["stored_b"], int(case["request"]["max_response_bytes"]))
                self.assertEqual(built["work"], expected)

    def test_redundant_lookup_mismatch_refused_from_valid_baseline(self) -> None:
        inputs = load_authored_inputs()
        baseline = copy.deepcopy(inputs)
        baseline["context"]["lookup_l"] = 10
        built = entity_read.build_success_case(baseline, baseline["cases"]["ver_ws"])
        self.assertEqual(built["work"], entity_read.compute_work(built["count_k"], 10, built["stored_b"], int(baseline["cases"]["ver_ws"]["request"]["max_response_bytes"])))
        mutated = copy.deepcopy(baseline)
        mutated["context"]["lookup_l"] = 7
        with self.assertRaises(entity_read.CheckFailed) as raised:
            entity_read.build_success_case(mutated, mutated["cases"]["ver_ws"])
        self.assertEqual(raised.exception.layer, "work")

    def test_redundant_count_mismatch_refused_from_valid_baseline(self) -> None:
        inputs = load_authored_inputs()
        baseline = copy.deepcopy(inputs)
        baseline["context"]["lookup_l"] = 10
        built = entity_read.build_success_case(baseline, baseline["cases"]["ver_ws"])
        self.assertEqual(built["count_k"], 1)
        mutated = copy.deepcopy(baseline)
        mutated["bindings"]["count"] = 255
        with self.assertRaises(entity_read.CheckFailed) as raised:
            entity_read.build_success_case(mutated, mutated["cases"]["ver_ws"])
        self.assertEqual(raised.exception.layer, "count")

    def test_declared_counts_derive_lookup_in_builder(self) -> None:
        inputs = load_authored_inputs()
        for declared_n in (0, 1, 255, 256):
            with self.subTest(n=declared_n):
                scoped = copy.deepcopy(inputs)
                scoped["context"]["root_bindings"] = declared_n
                scoped["bindings"]["count"] = declared_n
                del scoped["context"]["lookup_l"]
                derived_l = declared_n.bit_length() + 1
                built = entity_read.build_success_case(scoped, scoped["cases"]["ver_ws"])
                expected = entity_read.compute_work(built["count_k"], derived_l, built["stored_b"], int(scoped["cases"]["ver_ws"]["request"]["max_response_bytes"]))
                self.assertEqual(built["work"], expected)

    def test_declared_counts_charge(self) -> None:
        for declared_n, want_l in ((0, 1), (1, 2), (255, 9), (256, 10)):
            with self.subTest(n=declared_n):
                derived = declared_n.bit_length() + 1
                self.assertEqual(derived, want_l)
                self.assertEqual(entity_read.compute_work(1, derived, 0, 0), 1 + derived)


class RequestRecordLayerCases(unittest.TestCase):
    def test_valid_request_control(self) -> None:
        inputs = load_authored_inputs()
        root = bytes.fromhex(inputs["context"]["root"])
        entity = bytes.fromhex(inputs["entities"]["ws"]["id"])
        body = entity_read.build_request_body(root, entity, 16, 200000, 5000000)
        decoded = entity_read.decode_request_body(body, {"limits": inputs["selected_limits"]})
        self.assertEqual(decoded["root"], root)
        self.assertEqual(decoded["entity"], entity)
        self.assertEqual((decoded["max_objects"], decoded["ceiling_m"], decoded["max_work"]), (16, 200000, 5000000))

    def test_missing_field_reaches_record_layer_with_explicit_body(self) -> None:
        inputs = load_authored_inputs()
        authored = next(case for case in inputs["rejected"] if case["id"] == "req_missing_field")
        recipe = dict(authored["recipe"])
        recipe["level"] = "body"
        data = entity_read.build_rejected_bytes(inputs, inputs["cases"]["ver_ws"], recipe)
        layer, code = entity_read.validate_rejected(inputs, {"recipe": {"target": "request"}}, data)
        self.assertEqual(layer, "request_record")
        self.assertEqual(code, "SCB_FIELD_MISSING")

    def test_missing_field_omitted_level_reaches_record_layer(self) -> None:
        inputs = load_authored_inputs()
        authored = next(case for case in inputs["rejected"] if case["id"] == "req_missing_field")
        data = entity_read.build_rejected_bytes(inputs, inputs["cases"]["ver_ws"], authored["recipe"])
        layer, code = entity_read.validate_rejected(inputs, {"recipe": {"target": "request"}}, data)
        self.assertEqual((layer, code), ("request_record", "SCB_FIELD_MISSING"))

    def test_duplicate_reaches_duplicate_not_order(self) -> None:
        inputs = load_authored_inputs()
        authored = next(case for case in inputs["rejected"] if case["id"] == "req_duplicate_field")
        recipe = dict(authored["recipe"])
        recipe["level"] = "body"
        data = entity_read.build_rejected_bytes(inputs, inputs["cases"]["ver_ws"], recipe)
        layer, code = entity_read.validate_rejected(inputs, {"recipe": {"target": "request"}}, data)
        self.assertEqual((layer, code), ("request_record", "SCB_FIELD_DUPLICATE"))


class FailureEnvelopeCases(unittest.TestCase):
    def test_authored_failure_responses_reconstruct(self) -> None:
        inputs = load_authored_inputs()
        cases = [case for case in inputs["rejected"] if case.get("kind") == "failure_response"]
        self.assertEqual(len(cases), 6)
        for case in cases:
            with self.subTest(id=case["id"]):
                wire = entity_read.build_failure_wire(inputs, case)
                layer, _code = entity_read.validate_rejected(inputs, {"recipe": {"target": "failure"}}, wire)
                self.assertEqual(layer, "failure_accepted")
                stored, _prefix = entity_read.split_wire(wire, int(inputs["selected_limits"]["max_frame_bytes"]))
                payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
                frame = entity_read.decode_frame_payload(payload)
                failure = entity_read.decode_failure(frame["body"])
                self.assertEqual(failure["code"], case["expected_code"])
                self.assertEqual(failure["symbol"], case["expected_symbol"])
                self.assertEqual(failure["retryability"], case["expected_retryability"])

    def test_authored_failure_response_dispatches_through_checker(self) -> None:
        inputs = load_authored_inputs()
        authored = next(case for case in inputs["rejected"] if case["id"] == "fail_root_mismatch")
        scoped = dict(authored)
        scoped["input_hex"] = entity_read.build_failure_wire(inputs, authored).hex()
        self.assertEqual(scoped["kind"], "failure_response")
        problems = entity_read.check_rejected(inputs, {"cases": [scoped]})
        self.assertEqual(problems, [])


class SignatureMutationLayerCases(unittest.TestCase):
    def test_mutations_preserve_prior_object_entry_frame_integrity(self) -> None:
        inputs = load_authored_inputs()
        normalized = copy.deepcopy(inputs)
        normalized["context"]["lookup_l"] = 10
        epoch = bytes.fromhex(normalized["context"]["content_epoch"])
        for case_id in ("sig_owner", "sig_role", "sig_ordinal", "sig_type"):
            with self.subTest(id=case_id):
                case = next(item for item in normalized["rejected"] if item["id"] == case_id)
                base = normalized["cases"][case["base"]]
                recipe = dict(case["recipe"])
                recipe["level"] = "body"
                data = entity_read.build_rejected_bytes(normalized, base, recipe)
                stored, _prefix = entity_read.split_wire(data, int(normalized["selected_limits"]["max_frame_bytes"]))
                payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
                frame = entity_read.decode_frame_payload(payload)
                response = entity_read.decode_response_body(frame["body"])
                entry = entity_read.decode_response_entry(response["entries"][int(case["recipe"].get("index", 0))])
                object_record, trailer = entity_read.decode_stored_envelope(entry["stored"], epoch)
                entity_read.decode_stored_record(object_record)
                self.assertEqual(entry["object_id"], trailer)

    def test_mutations_reach_signature_layer(self) -> None:
        inputs = load_authored_inputs()
        normalized = copy.deepcopy(inputs)
        normalized["context"]["lookup_l"] = 10
        for case_id in ("sig_owner", "sig_role", "sig_ordinal", "sig_type"):
            with self.subTest(id=case_id):
                case = next(item for item in normalized["rejected"] if item["id"] == case_id)
                base = normalized["cases"][case["base"]]
                recipe = dict(case["recipe"])
                recipe["level"] = "body"
                data = entity_read.build_rejected_bytes(normalized, base, recipe)
                layer, _code = entity_read.validate_rejected(normalized, case, data)
                self.assertEqual(layer, "signature")


class HelloPreferenceCases(unittest.TestCase):
    def test_sorted_hello_control(self) -> None:
        inputs = load_authored_inputs()
        client = inputs["hellos"]["hello_v2_client"]
        server = inputs["hellos"]["hello_v1_server"]
        entity_read.build_hello(client)
        entity_read.build_hello(server)
        selection = entity_read.negotiate_versioned(client, server)
        self.assertEqual(selection["protocol_version"], 1)
        self.assertNotIn(306, selection["methods"])
        self.assertNotIn(307, selection["methods"])
        self.assertIn(900, selection["methods"])

    def test_descending_negotiation_selects_server_first_common(self) -> None:
        inputs = load_authored_inputs()
        client = inputs["hellos"]["hello_v2_client"]
        server = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
        server["schema_epochs"] = ["d4" * 32, "c3" * 32]
        selection = entity_read.negotiate_versioned(client, server)
        self.assertEqual(selection["protocol_version"], 2)
        self.assertEqual(selection["schema_epoch"], "d4" * 32)

    def test_descending_epochs_build_valid(self) -> None:
        inputs = load_authored_inputs()
        server = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
        server["schema_epochs"] = ["d4" * 32, "c3" * 32]
        entity_read.build_hello(server)

    def test_invalid_limits_refused_by_build(self) -> None:
        inputs = load_authored_inputs()
        for name, key, value in (("zero_work", "max_work", 0), ("over_frame", "max_frame_bytes", 67108865)):
            with self.subTest(offer=name):
                offer = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
                offer["limits"] = dict(offer["limits"])
                offer["limits"][key] = value
                with self.assertRaises(entity_read.CheckFailed):
                    entity_read.build_hello(offer)

    def test_invalid_limits_refused_by_negotiation(self) -> None:
        inputs = load_authored_inputs()
        server = inputs["hellos"]["hello_v2_server"]
        for name, key, value in (("zero_work", "max_work", 0), ("over_frame", "max_frame_bytes", 67108865)):
            with self.subTest(offer=name):
                offer = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
                offer["limits"] = dict(offer["limits"])
                offer["limits"][key] = value
                with self.assertRaises(entity_read.CheckFailed):
                    entity_read.negotiate_versioned(offer, server)


class StoredMetadataExactnessCases(unittest.TestCase):
    def test_valid_label_fingerprint_control(self) -> None:
        inputs = load_authored_inputs()
        epoch = bytes.fromhex(inputs["context"]["content_epoch"])
        ent = inputs["entities"]["ws"]
        item = entity_read.build_object(ent, epoch)
        stored = bytes.fromhex(item["stored_hex"])
        decoded = entity_read.check_stored_object(stored, ent["kind"], bytes.fromhex(ent["id"]), epoch)
        self.assertEqual(decoded["label"], "er2/k01/s00")
        self.assertEqual(decoded["fingerprint"], bytes.fromhex(ent["fingerprint"]))
        unbound = entity_read.decode_stored_unbound(stored, epoch)
        self.assertEqual(unbound["entity_id"], bytes.fromhex(ent["id"]))
        self.assertEqual(unbound["kind"], ent["kind"])

    def test_label_trailing_bytes_refused_bound(self) -> None:
        inputs = load_authored_inputs()
        epoch = bytes.fromhex(inputs["context"]["content_epoch"])
        ent = inputs["entities"]["ws"]
        stored = bytes.fromhex(entity_read.build_object(ent, epoch)["stored_hex"])

        def append_label_byte(fields):
            return [(tag, payload + b"\x00" if tag == 3 else payload) for tag, payload in fields]

        mutated = resplice_stored(stored, append_label_byte)
        entity_read.decode_stored_envelope(mutated, epoch)
        with self.assertRaisesRegex(ScbError, "SCB_TRAILING_BYTES"):
            entity_read.check_stored_object(mutated, ent["kind"], bytes.fromhex(ent["id"]), epoch)

    def test_label_trailing_bytes_refused_unbound(self) -> None:
        inputs = load_authored_inputs()
        epoch = bytes.fromhex(inputs["context"]["content_epoch"])
        ent = inputs["entities"]["ws"]
        stored = bytes.fromhex(entity_read.build_object(ent, epoch)["stored_hex"])

        def append_label_byte(fields):
            return [(tag, payload + b"\x00" if tag == 3 else payload) for tag, payload in fields]

        mutated = resplice_stored(stored, append_label_byte)
        entity_read.decode_stored_envelope(mutated, epoch)
        with self.assertRaisesRegex(ScbError, "SCB_TRAILING_BYTES"):
            entity_read.decode_stored_unbound(mutated, epoch)

    def test_fingerprint_width_refused_bound(self) -> None:
        inputs = load_authored_inputs()
        epoch = bytes.fromhex(inputs["context"]["content_epoch"])
        ent = inputs["entities"]["ws"]
        stored = bytes.fromhex(entity_read.build_object(ent, epoch)["stored_hex"])
        for width, code, adjust in ((31, "SCB_LENGTH_OVERFLOW", lambda raw: raw[:31]), (33, "SCB_TRAILING_BYTES", lambda raw: raw + b"\x00")):
            with self.subTest(width=width):
                mutated = resplice_stored(stored, lambda fields, fix=adjust: [(tag, fix(raw) if tag == 4 else raw) for tag, raw in fields])
                entity_read.decode_stored_envelope(mutated, epoch)
                with self.assertRaisesRegex(ScbError, code):
                    entity_read.check_stored_object(mutated, ent["kind"], bytes.fromhex(ent["id"]), epoch)

    def test_fingerprint_width_refused_unbound(self) -> None:
        inputs = load_authored_inputs()
        epoch = bytes.fromhex(inputs["context"]["content_epoch"])
        ent = inputs["entities"]["ws"]
        stored = bytes.fromhex(entity_read.build_object(ent, epoch)["stored_hex"])
        for width, code, adjust in ((31, "SCB_LENGTH_OVERFLOW", lambda raw: raw[:31]), (33, "SCB_TRAILING_BYTES", lambda raw: raw + b"\x00")):
            with self.subTest(width=width):
                mutated = resplice_stored(stored, lambda fields, fix=adjust: [(tag, fix(raw) if tag == 4 else raw) for tag, raw in fields])
                entity_read.decode_stored_envelope(mutated, epoch)
                with self.assertRaisesRegex(ScbError, code):
                    entity_read.decode_stored_unbound(mutated, epoch)


if __name__ == "__main__":
    unittest.main()
