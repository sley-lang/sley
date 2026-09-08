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
import hashlib
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
        full = load_authored_inputs()
        authored = next(case for case in full["rejected"] if case["id"] == "fail_root_mismatch")
        scoped_inputs = copy.deepcopy(full)
        scoped_inputs["rejected"] = [copy.deepcopy(authored)]
        scoped = copy.deepcopy(authored)
        scoped["input_hex"] = entity_read.build_failure_wire(scoped_inputs, authored).hex()
        self.assertEqual(scoped["kind"], "failure_response")
        inputs_sha = hashlib.sha256(json.dumps(scoped_inputs, sort_keys=True).encode()).hexdigest()
        rejected = {
            "contract": "sley2-entity-read-v2-rejected",
            "claim": "independent-expected",
            "manifest": {"inputs_sha256": inputs_sha},
            "cases": [scoped],
        }
        problems = entity_read.check_rejected(scoped_inputs, rejected)
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


_STRUCTURAL_ROWS = (
    "reserved_method",
    "unordered_methods",
    "unordered_versions",
    "unknown_feature",
    "unordered_adapters",
    "unordered_effects",
    "empty_versions",
    "empty_methods",
    "empty_epochs",
)


def _structural_mutation(offer, row):
    mutated = copy.deepcopy(offer)
    if row == "reserved_method":
        methods = mutated["methods"]
        at = methods.index(306)
        mutated["methods"] = methods[:at] + [305] + methods[at:]
    elif row == "unordered_methods":
        methods = mutated["methods"]
        mutated["methods"] = [methods[1], methods[0]] + methods[2:]
    elif row == "unordered_versions":
        mutated["protocol_versions"] = [2, 1]
    elif row == "unknown_feature":
        mutated["features"] = mutated["features"] | 0x20
    elif row == "unordered_adapters":
        mutated["adapters"] = ["22" * 32, "11" * 32]
    elif row == "unordered_effects":
        mutated["effects"] = ["22" * 32, "11" * 32]
    elif row == "empty_versions":
        mutated["protocol_versions"] = []
    elif row == "empty_methods":
        mutated["methods"] = []
    elif row == "empty_epochs":
        mutated["schema_epochs"] = []
    else:
        raise AssertionError(row)
    return mutated


def _ceiling_values(list_name, n):
    if list_name == "protocol_versions":
        return list(range(1, n + 1))
    if list_name == "methods":
        return [100] + list(range(10000, 10000 + n - 1))
    if list_name == "schema_epochs":
        return ["c3" * 32] + [i.to_bytes(32, "big").hex() for i in range(1, n)]
    if list_name in ("adapters", "effects"):
        return [i.to_bytes(32, "big").hex() for i in range(1, n + 1)]
    raise AssertionError(list_name)


class HelloAdmissionConsistencyCases(unittest.TestCase):
    def test_duplicate_epoch_preference_build_preserves_raw_list(self) -> None:
        inputs = load_authored_inputs()
        offer = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
        offer["schema_epochs"] = ["d4" * 32, "c3" * 32, "d4" * 32]
        raw = entity_read.build_hello(offer)
        fields = entity_read.parse_record(raw)
        epoch_list_raw = entity_read.single_field(fields, 2)
        reader = entity_read.Reader(epoch_list_raw)
        count = reader.uvar(64)
        entries = [reader.sized(entity_read.MAX_STANDALONE_BYTES) for _ in range(count)]
        reader.finish()
        self.assertEqual(count, 3)
        self.assertEqual([entry.hex() for entry in entries], ["d4" * 32, "c3" * 32, "d4" * 32])

    def test_duplicate_epoch_preference_negotiation_selects_first_common(self) -> None:
        inputs = load_authored_inputs()
        with self.subTest(side="server_duplicate"):
            client = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
            server = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
            server["schema_epochs"] = ["d4" * 32, "c3" * 32, "d4" * 32]
            selection = entity_read.negotiate_versioned(client, server)
            self.assertEqual(selection["protocol_version"], 2)
            self.assertEqual(selection["schema_epoch"], "d4" * 32)
        with self.subTest(side="client_duplicate"):
            client = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
            client["schema_epochs"] = ["d4" * 32, "c3" * 32, "d4" * 32]
            server = inputs["hellos"]["hello_v2_server"]
            selection = entity_read.negotiate_versioned(client, server)
            self.assertEqual(selection["protocol_version"], 2)
            self.assertEqual(selection["schema_epoch"], "c3" * 32)

    def test_zero_depth_offer_build_is_valid(self) -> None:
        inputs = load_authored_inputs()
        ordinary = inputs["hellos"]["hello_v2_client"]
        offer = copy.deepcopy(ordinary)
        offer["limits"] = dict(offer["limits"])
        offer["limits"]["max_depth"] = 0
        raw = entity_read.build_hello(offer)
        fields = entity_read.parse_record(raw)
        limit_fields = entity_read.parse_record(entity_read.single_field(fields, 3))
        self.assertEqual(entity_read.decode_uvar_exact(entity_read.single_field(limit_fields, 4), 32), 0)
        self.assertEqual(
            entity_read.decode_uvar_exact(entity_read.single_field(limit_fields, 1), 64),
            ordinary["limits"]["max_frame_bytes"],
        )

    def test_zero_depth_offer_negotiation_is_valid(self) -> None:
        inputs = load_authored_inputs()
        limit_keys = ("max_frame_bytes", "max_entities", "max_edges", "max_depth", "max_response_bytes", "max_work", "max_inflight", "max_sessions")
        for role in ("client", "server"):
            with self.subTest(role=role):
                client = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
                server = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
                target = client if role == "client" else server
                target["limits"] = dict(target["limits"])
                target["limits"]["max_depth"] = 0
                selection = entity_read.negotiate_versioned(client, server)
                self.assertEqual(selection["limits"]["max_depth"], 0)
                for key in limit_keys:
                    with self.subTest(field=key):
                        self.assertEqual(selection["limits"][key], min(client["limits"][key], server["limits"][key]))

    def test_invalid_offer_structure_refused_by_build(self) -> None:
        inputs = load_authored_inputs()
        for role in ("client", "server"):
            base_name = "hello_v2_client" if role == "client" else "hello_v2_server"
            for row in _STRUCTURAL_ROWS:
                with self.subTest(role=role, row=row):
                    offer = _structural_mutation(inputs["hellos"][base_name], row)
                    with self.assertRaises(entity_read.CheckFailed) as raised:
                        entity_read.build_hello(offer)
                    self.assertEqual(raised.exception.layer, "hello")

    def test_invalid_offer_structure_refused_by_negotiation(self) -> None:
        inputs = load_authored_inputs()
        for row in _STRUCTURAL_ROWS:
            for role in ("client", "server"):
                with self.subTest(role=role, row=row):
                    client = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
                    server = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
                    if role == "client":
                        client = _structural_mutation(client, row)
                    else:
                        server = _structural_mutation(server, row)
                    with self.assertRaises(entity_read.CheckFailed) as raised:
                        entity_read.negotiate_versioned(client, server)
                    self.assertEqual(raised.exception.layer, "hello")

    def test_hello_list_ceiling_build_exact_and_over(self) -> None:
        inputs = load_authored_inputs()
        for list_name in ("protocol_versions", "methods", "schema_epochs", "adapters", "effects"):
            with self.subTest(list=list_name):
                exact = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
                exact[list_name] = _ceiling_values(list_name, 4096)
                entity_read.build_hello(exact)
                over = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
                over[list_name] = _ceiling_values(list_name, 4097)
                with self.assertRaises(entity_read.CheckFailed) as raised:
                    entity_read.build_hello(over)
                self.assertEqual(raised.exception.layer, "hello")

    def test_hello_list_ceiling_negotiation_exact_and_over(self) -> None:
        inputs = load_authored_inputs()
        for list_name in ("protocol_versions", "methods", "schema_epochs", "adapters", "effects"):
            for role in ("client", "server"):
                with self.subTest(list=list_name, role=role):
                    client = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
                    server = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
                    if role == "client":
                        client[list_name] = _ceiling_values(list_name, 4096)
                    else:
                        server[list_name] = _ceiling_values(list_name, 4096)
                    selection = entity_read.negotiate_versioned(client, server)
                    self.assertEqual(selection["protocol_version"], 2)
                    self.assertIn(100, selection["methods"])
                    client = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
                    server = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
                    if role == "client":
                        client[list_name] = _ceiling_values(list_name, 4097)
                    else:
                        server[list_name] = _ceiling_values(list_name, 4097)
                    with self.assertRaises(entity_read.CheckFailed) as raised:
                        entity_read.negotiate_versioned(client, server)
                    self.assertEqual(raised.exception.layer, "hello")


class RecipeSelectorAdmissionCases(unittest.TestCase):
    def test_known_recipe_selector_controls(self) -> None:
        inputs = load_authored_inputs()
        base = inputs["cases"]["ver_ws"]
        with self.subTest(route="named_record_omitted_level"):
            authored = next(case for case in inputs["rejected"] if case["id"] == "req_missing_field")
            data = entity_read.build_rejected_bytes(inputs, base, authored["recipe"])
            self.assertEqual(
                entity_read.validate_rejected(inputs, {"recipe": {"target": "request"}}, data),
                ("request_record", "SCB_FIELD_MISSING"),
            )
        with self.subTest(route="named_record_explicit_body"):
            authored = next(case for case in inputs["rejected"] if case["id"] == "req_missing_field")
            recipe = dict(authored["recipe"])
            recipe["level"] = "body"
            data = entity_read.build_rejected_bytes(inputs, base, recipe)
            self.assertEqual(
                entity_read.validate_rejected(inputs, {"recipe": {"target": "request"}}, data),
                ("request_record", "SCB_FIELD_MISSING"),
            )
        with self.subTest(route="explicit_body_append"):
            recipe = {"target": "response", "level": "body", "op": "append", "hex": "00"}
            data = entity_read.build_rejected_bytes(inputs, base, recipe)
            self.assertEqual(
                entity_read.validate_rejected(inputs, {"recipe": {"target": "response"}, "base": "ver_ws"}, data),
                ("response_record", "SCB_TRAILING_BYTES"),
            )
        with self.subTest(route="wire_truncate"):
            recipe = {"target": "response", "level": "wire", "op": "truncate", "n": 1}
            data = entity_read.build_rejected_bytes(inputs, base, recipe)
            self.assertEqual(
                entity_read.validate_rejected(inputs, {"recipe": {"target": "response"}, "base": "ver_ws"}, data),
                ("wire_prefix", None),
            )

    def test_unknown_target_selector_refused(self) -> None:
        inputs = load_authored_inputs()
        base = inputs["cases"]["ver_ws"]
        good = {"target": "request", "record": "request", "op": "identity"}
        wire = entity_read.build_rejected_bytes(inputs, base, good)
        stored, _prefix = entity_read.split_wire(wire, int(inputs["selected_limits"]["max_frame_bytes"]))
        entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
        self.assertEqual(
            entity_read.validate_rejected(inputs, {"recipe": {"target": "request"}, "base": "ver_ws"}, wire),
            ("request_accepted", None),
        )
        bad = dict(good)
        bad["target"] = "unknown-target"
        with self.assertRaises(ValueError) as raised:
            entity_read.build_rejected_bytes(inputs, base, bad)
        message = str(raised.exception)
        self.assertIn("target", message)
        self.assertIn("unknown-target", message)

    def test_unknown_record_selector_refused(self) -> None:
        inputs = load_authored_inputs()
        base = inputs["cases"]["ver_ws"]
        good = {"target": "response", "record": "response", "op": "identity"}
        wire = entity_read.build_rejected_bytes(inputs, base, good)
        stored, _prefix = entity_read.split_wire(wire, int(inputs["selected_limits"]["max_frame_bytes"]))
        payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
        entity_read.decode_response_body(entity_read.decode_frame_payload(payload)["body"])
        bad = dict(good)
        bad["record"] = "unknown-record"
        with self.assertRaises(ValueError) as raised:
            entity_read.build_rejected_bytes(inputs, base, bad)
        message = str(raised.exception)
        self.assertIn("record", message)
        self.assertIn("unknown-record", message)

    def test_unknown_level_selector_refused(self) -> None:
        inputs = load_authored_inputs()
        base = inputs["cases"]["ver_ws"]
        good = {"target": "response", "level": "body", "op": "identity"}
        wire = entity_read.build_rejected_bytes(inputs, base, good)
        stored, _prefix = entity_read.split_wire(wire, int(inputs["selected_limits"]["max_frame_bytes"]))
        payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
        entity_read.decode_response_body(entity_read.decode_frame_payload(payload)["body"])
        bad = dict(good)
        bad["level"] = "unknown-level"
        with self.assertRaises(ValueError) as raised:
            entity_read.build_rejected_bytes(inputs, base, bad)
        message = str(raised.exception)
        self.assertIn("level", message)
        self.assertIn("unknown-level", message)


def _sig_owner_control(testcase):
    inputs = load_authored_inputs()
    authored = next(item for item in inputs["rejected"] if item["id"] == "sig_owner")
    base = inputs["cases"][authored["base"]]
    wire = entity_read.build_rejected_bytes(inputs, base, authored["recipe"])
    max_frame = int(inputs["selected_limits"]["max_frame_bytes"])
    stored, _prefix = entity_read.split_wire(wire, max_frame)
    payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
    frame = entity_read.decode_frame_payload(payload)
    response = entity_read.decode_response_body(frame["body"])
    bounds = entity_read.decode_bounds(frame["bounds"])
    epoch = bytes.fromhex(inputs["context"]["content_epoch"])
    entries = [entity_read.decode_response_entry(raw) for raw in response["entries"]]
    for entry in entries:
        with testcase.subTest(entry=entry["entity"].hex()):
            checked = entity_read.check_stored_object(entry["stored"], entry["kind"], entry["entity"], epoch)
            testcase.assertEqual(entry["object_id"], checked["object_id"])
    requested = bytes.fromhex(inputs["entities"][base["entity"]]["id"])
    entity_read.check_context(response, inputs["context"], requested)
    count_k = len(entries)
    stored_b = sum(len(entry["stored"]) for entry in entries)
    lookup_l = int(inputs["context"]["root_bindings"]).bit_length() + 1
    ceiling_m = int(base["request"]["max_response_bytes"])
    testcase.assertEqual(response["work"], 1 + count_k * lookup_l + 2 * stored_b + ceiling_m)
    testcase.assertEqual(bounds["returned_entities"], count_k)
    testcase.assertEqual(bounds["returned_bytes"], len(frame["body"]))
    testcase.assertEqual((bounds["returned_edges"], bounds["reached_depth"], bounds["omitted"]), (0, 0, 0))
    testcase.assertFalse(bounds["truncated"])
    testcase.assertFalse(bounds["continuation"])
    testcase.assertEqual(entity_read.validate_rejected(inputs, authored, wire), ("signature", None))
    return inputs, authored, base, wire, frame, response, bounds, entries


class SignaturePriorCountCases(unittest.TestCase):
    def test_wrong_owner_with_valid_counts_control(self) -> None:
        _sig_owner_control(self)

    def test_wrong_owner_with_wrong_returned_bytes_refuses_count_first(self) -> None:
        inputs, authored, _base, _wire, frame, response, bounds, entries = _sig_owner_control(self)
        actual_bytes = len(frame["body"])
        actual_k = len(entries)
        mutated_fields = [(tag, encode_uvar(actual_bytes + 1) if tag == 2 else payload) for tag, payload in entity_read.parse_record(frame["bounds"])]
        mutated_payload = entity_read.build_frame_payload(
            frame["version"],
            frame["session"],
            frame["request_id"],
            frame["kind"],
            frame["method"],
            frame["flags"],
            entity_read.encode_fields(mutated_fields),
            frame["body"],
        )
        mutated_wire, _pre, _fid = entity_read.build_envelope(entity_read.protocol_epoch_id(), mutated_payload)
        max_frame = int(inputs["selected_limits"]["max_frame_bytes"])
        stored, _prefix = entity_read.split_wire(mutated_wire, max_frame)
        payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
        mutated_frame = entity_read.decode_frame_payload(payload)
        self.assertEqual(mutated_frame["body"], frame["body"])
        self.assertEqual(entity_read.decode_response_body(mutated_frame["body"])["work"], response["work"])
        mutated_bounds = entity_read.decode_bounds(mutated_frame["bounds"])
        self.assertEqual(mutated_bounds["returned_bytes"], actual_bytes + 1)
        self.assertEqual(mutated_bounds["returned_entities"], actual_k)
        self.assertEqual(entity_read.validate_rejected(inputs, authored, mutated_wire), ("count", None))

    def test_wrong_owner_with_wrong_returned_entities_refuses_count_first(self) -> None:
        inputs, authored, _base, _wire, frame, response, bounds, entries = _sig_owner_control(self)
        actual_bytes = len(frame["body"])
        actual_k = len(entries)
        mutated_fields = [(tag, encode_uvar(actual_k + 1) if tag == 3 else payload) for tag, payload in entity_read.parse_record(frame["bounds"])]
        mutated_payload = entity_read.build_frame_payload(
            frame["version"],
            frame["session"],
            frame["request_id"],
            frame["kind"],
            frame["method"],
            frame["flags"],
            entity_read.encode_fields(mutated_fields),
            frame["body"],
        )
        mutated_wire, _pre, _fid = entity_read.build_envelope(entity_read.protocol_epoch_id(), mutated_payload)
        max_frame = int(inputs["selected_limits"]["max_frame_bytes"])
        stored, _prefix = entity_read.split_wire(mutated_wire, max_frame)
        payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
        mutated_frame = entity_read.decode_frame_payload(payload)
        self.assertEqual(mutated_frame["body"], frame["body"])
        self.assertEqual(entity_read.decode_response_body(mutated_frame["body"])["work"], response["work"])
        mutated_bounds = entity_read.decode_bounds(mutated_frame["bounds"])
        self.assertEqual(mutated_bounds["returned_entities"], actual_k + 1)
        self.assertEqual(mutated_bounds["returned_bytes"], actual_bytes)
        self.assertEqual(entity_read.validate_rejected(inputs, authored, mutated_wire), ("count", None))


class HelloIdentityEqualityCases(unittest.TestCase):
    def test_epoch_equivalent_spellings_select_same_identity(self) -> None:
        inputs = load_authored_inputs()
        canonical = "ab" * 32
        upper = "AB" * 32
        spaced = " ".join(["AB"] * 32)
        distinct = "cd" * 32
        client_base = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
        server_base = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
        client_base["schema_epochs"] = [canonical]
        server_base["schema_epochs"] = [canonical]
        entity_read.build_hello(client_base)
        entity_read.build_hello(server_base)
        self.assertEqual(entity_read.negotiate_versioned(client_base, server_base)["schema_epoch"], canonical)
        for role in ("client", "server"):
            base = client_base if role == "client" else server_base
            for variant, spelling in (("upper", upper), ("spaced", spaced)):
                with self.subTest(role=role, variant=variant):
                    canonical_raw = entity_read.build_hello(base)
                    canonical_fields = entity_read.parse_record(canonical_raw)
                    canonical_list = entity_read.single_field(canonical_fields, 2)
                    canonical_reader = entity_read.Reader(canonical_list)
                    canonical_count = canonical_reader.uvar(64)
                    canonical_entries = [canonical_reader.sized(entity_read.MAX_STANDALONE_BYTES) for _ in range(canonical_count)]
                    canonical_reader.finish()
                    self.assertEqual(canonical_entries, [bytes.fromhex(canonical)])
                    variant_offer = copy.deepcopy(base)
                    variant_offer["schema_epochs"] = [spelling]
                    variant_raw = entity_read.build_hello(variant_offer)
                    variant_fields = entity_read.parse_record(variant_raw)
                    variant_list = entity_read.single_field(variant_fields, 2)
                    variant_reader = entity_read.Reader(variant_list)
                    variant_count = variant_reader.uvar(64)
                    variant_entries = [variant_reader.sized(entity_read.MAX_STANDALONE_BYTES) for _ in range(variant_count)]
                    variant_reader.finish()
                    self.assertEqual(variant_entries, canonical_entries)
                    client = copy.deepcopy(client_base)
                    server = copy.deepcopy(server_base)
                    if role == "client":
                        client["schema_epochs"] = [spelling]
                    else:
                        server["schema_epochs"] = [spelling]
                    self.assertEqual(entity_read.negotiate_versioned(client, server)["schema_epoch"], canonical)
        with self.subTest(control="distinct_bytes"):
            client = copy.deepcopy(client_base)
            server = copy.deepcopy(server_base)
            server["schema_epochs"] = [distinct]
            with self.assertRaisesRegex(entity_read.CheckFailed, "selection"):
                entity_read.negotiate_versioned(client, server)

    def test_adapter_equivalent_spellings_intersect(self) -> None:
        inputs = load_authored_inputs()
        canonical = "ab" * 32
        upper = "AB" * 32
        spaced = " ".join(["AB"] * 32)
        distinct = "cd" * 32
        client_base = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
        server_base = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
        client_base["adapters"] = [canonical]
        server_base["adapters"] = [canonical]
        entity_read.build_hello(client_base)
        entity_read.build_hello(server_base)
        self.assertEqual(entity_read.negotiate_versioned(client_base, server_base)["adapters"], [canonical])
        for role in ("client", "server"):
            base = client_base if role == "client" else server_base
            for variant, spelling in (("upper", upper), ("spaced", spaced)):
                with self.subTest(role=role, variant=variant):
                    canonical_raw = entity_read.build_hello(base)
                    canonical_fields = entity_read.parse_record(canonical_raw)
                    canonical_list = entity_read.single_field(canonical_fields, 6)
                    canonical_reader = entity_read.Reader(canonical_list)
                    canonical_count = canonical_reader.uvar(64)
                    canonical_entries = [canonical_reader.sized(entity_read.MAX_STANDALONE_BYTES) for _ in range(canonical_count)]
                    canonical_reader.finish()
                    self.assertEqual(canonical_entries, [bytes.fromhex(canonical)])
                    variant_offer = copy.deepcopy(base)
                    variant_offer["adapters"] = [spelling]
                    variant_raw = entity_read.build_hello(variant_offer)
                    variant_fields = entity_read.parse_record(variant_raw)
                    variant_list = entity_read.single_field(variant_fields, 6)
                    variant_reader = entity_read.Reader(variant_list)
                    variant_count = variant_reader.uvar(64)
                    variant_entries = [variant_reader.sized(entity_read.MAX_STANDALONE_BYTES) for _ in range(variant_count)]
                    variant_reader.finish()
                    self.assertEqual(variant_entries, canonical_entries)
                    client = copy.deepcopy(client_base)
                    server = copy.deepcopy(server_base)
                    if role == "client":
                        client["adapters"] = [spelling]
                    else:
                        server["adapters"] = [spelling]
                    self.assertEqual(entity_read.negotiate_versioned(client, server)["adapters"], [canonical])
        with self.subTest(control="distinct_bytes"):
            client = copy.deepcopy(client_base)
            server = copy.deepcopy(server_base)
            server["adapters"] = [distinct]
            self.assertEqual(entity_read.negotiate_versioned(client, server)["adapters"], [])

    def test_effect_equivalent_spellings_intersect(self) -> None:
        inputs = load_authored_inputs()
        canonical = "ab" * 32
        upper = "AB" * 32
        spaced = " ".join(["AB"] * 32)
        distinct = "cd" * 32
        client_base = copy.deepcopy(inputs["hellos"]["hello_v2_client"])
        server_base = copy.deepcopy(inputs["hellos"]["hello_v2_server"])
        client_base["effects"] = [canonical]
        server_base["effects"] = [canonical]
        entity_read.build_hello(client_base)
        entity_read.build_hello(server_base)
        self.assertEqual(entity_read.negotiate_versioned(client_base, server_base)["effects"], [canonical])
        for role in ("client", "server"):
            base = client_base if role == "client" else server_base
            for variant, spelling in (("upper", upper), ("spaced", spaced)):
                with self.subTest(role=role, variant=variant):
                    canonical_raw = entity_read.build_hello(base)
                    canonical_fields = entity_read.parse_record(canonical_raw)
                    canonical_list = entity_read.single_field(canonical_fields, 7)
                    canonical_reader = entity_read.Reader(canonical_list)
                    canonical_count = canonical_reader.uvar(64)
                    canonical_entries = [canonical_reader.sized(entity_read.MAX_STANDALONE_BYTES) for _ in range(canonical_count)]
                    canonical_reader.finish()
                    self.assertEqual(canonical_entries, [bytes.fromhex(canonical)])
                    variant_offer = copy.deepcopy(base)
                    variant_offer["effects"] = [spelling]
                    variant_raw = entity_read.build_hello(variant_offer)
                    variant_fields = entity_read.parse_record(variant_raw)
                    variant_list = entity_read.single_field(variant_fields, 7)
                    variant_reader = entity_read.Reader(variant_list)
                    variant_count = variant_reader.uvar(64)
                    variant_entries = [variant_reader.sized(entity_read.MAX_STANDALONE_BYTES) for _ in range(variant_count)]
                    variant_reader.finish()
                    self.assertEqual(variant_entries, canonical_entries)
                    client = copy.deepcopy(client_base)
                    server = copy.deepcopy(server_base)
                    if role == "client":
                        client["effects"] = [spelling]
                    else:
                        server["effects"] = [spelling]
                    self.assertEqual(entity_read.negotiate_versioned(client, server)["effects"], [canonical])
        with self.subTest(control="distinct_bytes"):
            client = copy.deepcopy(client_base)
            server = copy.deepcopy(server_base)
            server["effects"] = [distinct]
            self.assertEqual(entity_read.negotiate_versioned(client, server)["effects"], [])


def _b1_scoped_inputs():
    full = load_authored_inputs()
    scoped = copy.deepcopy(full)
    scoped["cases"] = {key: copy.deepcopy(full["cases"][key]) for key in ("ver_ws", "sig_multi")}
    scoped["hellos"] = {
        key: copy.deepcopy(full["hellos"][key])
        for key in ("hello_v2_client", "hello_v1_server", "hello_v2_server", "hello_v3_client", "hello_v3_server")
    }
    scoped["selection_scenarios"] = {
        key: copy.deepcopy(full["selection_scenarios"][key]) for key in ("v1_select", "v2_select", "v3_refuse")
    }
    wanted = ("req_missing_field", "req_duplicate_field", "fail_root_mismatch", "bound_k_exact", "seq_wrong_session")
    by_id = {row["id"]: row for row in full["rejected"]}
    scoped["rejected"] = [copy.deepcopy(by_id[row_id]) for row_id in wanted]
    return scoped


def _b1_build_accepted(scoped):
    cases = {}
    for case_id in ("ver_ws", "sig_multi"):
        built = entity_read.build_success_case(scoped, scoped["cases"][case_id])
        cases[case_id] = {"id": case_id, **built}
    hellos = {name: {"body_hex": entity_read.build_hello(offer).hex()} for name, offer in scoped["hellos"].items()}
    selections = {}
    for name, scenario in scoped["selection_scenarios"].items():
        if scenario.get("expect") == "fail":
            try:
                entity_read.negotiate_versioned(scoped["hellos"][scenario["client"]], scoped["hellos"][scenario["server"]])
            except entity_read.CheckFailed as error:
                assert error.layer == "selection"
                selections[name] = {"expected_failure": "selection"}
            else:
                raise AssertionError(f"{name} was accepted")
            continue
        selection = entity_read.negotiate_versioned(scoped["hellos"][scenario["client"]], scoped["hellos"][scenario["server"]])
        preimage = entity_read.build_selected(selection)
        client_body = bytes.fromhex(hellos[scenario["client"]]["body_hex"])
        server_body = bytes.fromhex(hellos[scenario["server"]]["body_hex"])
        transcript = client_body + server_body + preimage
        selections[name] = {
            **selection,
            "preimage_hex": preimage.hex(),
            "transcript_hex": transcript.hex(),
            "handshake_id": entity_read.handshake_id(client_body, server_body, preimage).hex(),
        }
    inputs_sha = hashlib.sha256(json.dumps(scoped, sort_keys=True).encode()).hexdigest()
    return {
        "contract": "sley2-entity-read-v2",
        "claim": "independent-expected",
        "manifest": {"inputs_sha256": inputs_sha},
        "cases": cases,
        "hellos": hellos,
        "selections": selections,
    }


def _b1_build_rejected(scoped):
    cases = []
    for authored in scoped["rejected"]:
        row = copy.deepcopy(authored)
        if row["id"] in ("req_missing_field", "req_duplicate_field"):
            base = scoped["cases"][row["base"]]
            row["input_hex"] = entity_read.build_rejected_bytes(scoped, base, row["recipe"]).hex()
        elif row["id"] == "fail_root_mismatch":
            row["input_hex"] = entity_read.build_failure_wire(scoped, row).hex()
        elif row["id"] == "bound_k_exact":
            row = entity_read.derive_relation(scoped, row)
        cases.append(row)
    inputs_sha = hashlib.sha256(json.dumps(scoped, sort_keys=True).encode()).hexdigest()
    return {
        "contract": "sley2-entity-read-v2-rejected",
        "claim": "independent-expected",
        "manifest": {"inputs_sha256": inputs_sha},
        "cases": cases,
    }


def _b1_control():
    scoped = _b1_scoped_inputs()
    return scoped, _b1_build_accepted(scoped), _b1_build_rejected(scoped)


def _b1_flip_hex(value):
    assert len(value) >= 1
    first = value[0]
    replacement = "0" if first != "0" else "1"
    return replacement + value[1:]


def _b1_case_by_id(supplied_cases, row_id):
    return next(case for case in supplied_cases if case["id"] == row_id)


class CorpusContentClosureCases(unittest.TestCase):
    def assertB1Problems(self, problems, *expected):
        self.assertIsInstance(problems, list)
        for entry in problems:
            self.assertIsInstance(entry, str)
        for diagnostic in expected:
            self.assertIn(diagnostic, problems)

    def test_small_authored_content_control(self) -> None:
        scoped, accepted, rejected = _b1_control()
        self.assertEqual(accepted["contract"], "sley2-entity-read-v2")
        self.assertEqual(accepted["claim"], "independent-expected")
        self.assertEqual(rejected["contract"], "sley2-entity-read-v2-rejected")
        self.assertEqual(rejected["claim"], "independent-expected")
        self.assertEqual(
            scoped["rejected"] and [row["id"] for row in scoped["rejected"]],
            ["req_missing_field", "req_duplicate_field", "fail_root_mismatch", "bound_k_exact", "seq_wrong_session"],
        )
        self.assertEqual(sorted(accepted["cases"].keys()), ["sig_multi", "ver_ws"])
        self.assertEqual(len(rejected["cases"]), 5)
        self.assertEqual(entity_read.check_accepted(scoped, accepted), [])
        self.assertEqual(entity_read.check_rejected(scoped, rejected), [])
        missing = _b1_case_by_id(scoped["rejected"], "req_missing_field")
        duplicate = _b1_case_by_id(scoped["rejected"], "req_duplicate_field")
        base = scoped["cases"]["ver_ws"]
        missing_bytes = entity_read.build_rejected_bytes(scoped, base, missing["recipe"])
        duplicate_bytes = entity_read.build_rejected_bytes(scoped, base, duplicate["recipe"])
        self.assertEqual(
            entity_read.validate_rejected(scoped, {"recipe": {"target": "request"}}, missing_bytes),
            ("request_record", "SCB_FIELD_MISSING"),
        )
        self.assertEqual(
            entity_read.validate_rejected(scoped, {"recipe": {"target": "request"}}, duplicate_bytes),
            ("request_record", "SCB_FIELD_DUPLICATE"),
        )
        failure_row = _b1_case_by_id(scoped["rejected"], "fail_root_mismatch")
        failure_bytes = entity_read.build_failure_wire(scoped, failure_row)
        layer, _code = entity_read.validate_rejected(scoped, {"recipe": {"target": "failure"}}, failure_bytes)
        self.assertEqual(layer, "failure_accepted")
        relation_row = _b1_case_by_id(rejected["cases"], "bound_k_exact")
        self.assertEqual(relation_row["max_objects"], 3)
        sequence_row = _b1_case_by_id(rejected["cases"], "seq_wrong_session")
        self.assertEqual(sequence_row["status"], "pending_runtime_comparison")

    def test_accepted_inventories_are_exact(self) -> None:
        scoped, accepted, _rejected = _b1_control()
        self.assertEqual(entity_read.check_accepted(scoped, accepted), [])
        with self.subTest(mutation="remove-ver_ws"):
            mutated = copy.deepcopy(accepted)
            del mutated["cases"]["ver_ws"]
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "ver_ws:missing-expected")
        with self.subTest(mutation="clear-accepted-cases"):
            mutated = copy.deepcopy(accepted)
            mutated["cases"] = {}
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "ver_ws:missing-expected", "sig_multi:missing-expected")
        with self.subTest(mutation="extra-accepted-case"):
            mutated = copy.deepcopy(accepted)
            mutated["cases"]["extra_case"] = copy.deepcopy(mutated["cases"]["ver_ws"])
            mutated["cases"]["extra_case"]["id"] = "extra_case"
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "accepted:cases:inventory")
        with self.subTest(mutation="remove-sig_multi-object"):
            mutated = copy.deepcopy(accepted)
            del mutated["cases"]["sig_multi"]["objects"]["sig_p_low"]
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "sig_multi:object:sig_p_low:stored_hex")
        with self.subTest(mutation="extra-sig_multi-object"):
            mutated = copy.deepcopy(accepted)
            mutated["cases"]["sig_multi"]["objects"]["extra_obj"] = copy.deepcopy(
                mutated["cases"]["sig_multi"]["objects"]["sig_p_low"]
            )
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "sig_multi:objects:inventory")
        with self.subTest(mutation="remove-hello"):
            mutated = copy.deepcopy(accepted)
            del mutated["hellos"]["hello_v2_client"]
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "hello:hello_v2_client:body")
        with self.subTest(mutation="extra-hello"):
            mutated = copy.deepcopy(accepted)
            mutated["hellos"]["extra_hello"] = copy.deepcopy(mutated["hellos"]["hello_v2_client"])
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "accepted:hellos:inventory")
        with self.subTest(mutation="remove-success-selection"):
            mutated = copy.deepcopy(accepted)
            del mutated["selections"]["v2_select"]
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v2_select:version")
        with self.subTest(mutation="extra-success-selection"):
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["extra_select"] = copy.deepcopy(mutated["selections"]["v2_select"])
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "accepted:selections:inventory")
        with self.subTest(mutation="remove-failed-selection"):
            mutated = copy.deepcopy(accepted)
            del mutated["selections"]["v3_refuse"]
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v3_refuse:missing-expected-failure")
        with self.subTest(mutation="extra-failed-selection"):
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["extra_refuse"] = {"expected_failure": "selection"}
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "accepted:selections:inventory")

    def test_rejected_inventory_is_exact(self) -> None:
        scoped, _accepted, rejected = _b1_control()
        self.assertEqual(entity_read.check_rejected(scoped, rejected), [])
        with self.subTest(mutation="empty"):
            mutated = copy.deepcopy(rejected)
            mutated["cases"] = []
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:cases:inventory")
        with self.subTest(mutation="drop-one"):
            mutated = copy.deepcopy(rejected)
            mutated["cases"] = [case for case in mutated["cases"] if case["id"] != "bound_k_exact"]
            self.assertEqual(len(mutated["cases"]), 4)
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:cases:inventory")
        with self.subTest(mutation="duplicate-id"):
            mutated = copy.deepcopy(rejected)
            mutated["cases"].append(copy.deepcopy(_b1_case_by_id(mutated["cases"], "req_missing_field")))
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:cases:inventory")
        with self.subTest(mutation="extra-valid-row"):
            mutated = copy.deepcopy(rejected)
            extra = copy.deepcopy(_b1_case_by_id(mutated["cases"], "req_missing_field"))
            extra["id"] = "req_missing_field_extra"
            mutated["cases"].append(extra)
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:cases:inventory")
        with self.subTest(mutation="reorder"):
            mutated = copy.deepcopy(rejected)
            mutated["cases"][0], mutated["cases"][1] = mutated["cases"][1], mutated["cases"][0]
            self.assertNotEqual(mutated["cases"][0]["id"], rejected["cases"][0]["id"])
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:cases:inventory")
        with self.subTest(mutation="duplicate-inputs-ids"):
            mutated_inputs = copy.deepcopy(scoped)
            duplicated = copy.deepcopy(mutated_inputs["rejected"][0])
            mutated_inputs["rejected"].append(duplicated)
            rebuilt_rejected = _b1_build_rejected(mutated_inputs)
            problems = entity_read.check_rejected(mutated_inputs, rebuilt_rejected)
            self.assertB1Problems(problems, "inputs:rejected:ids")

    def test_all_success_and_object_fields_are_bound(self) -> None:
        scoped, accepted, _rejected = _b1_control()
        self.assertEqual(entity_read.check_accepted(scoped, accepted), [])
        hex_fields = (
            "request_body_hex",
            "response_body_hex",
            "request_wire_hex",
            "response_wire_hex",
            "request_preimage_hex",
            "response_preimage_hex",
            "request_frame_id",
            "response_frame_id",
        )
        for field in hex_fields:
            with self.subTest(case="ver_ws", path=field):
                mutated = copy.deepcopy(accepted)
                original = mutated["cases"]["ver_ws"][field]
                mutated["cases"]["ver_ws"][field] = _b1_flip_hex(original)
                self.assertNotEqual(mutated["cases"]["ver_ws"][field], original)
                problems = entity_read.check_accepted(scoped, mutated)
                self.assertB1Problems(problems, f"ver_ws:{field}")
        for field in ("response_wire_len", "work", "count_k", "stored_b"):
            with self.subTest(case="ver_ws", path=field):
                mutated = copy.deepcopy(accepted)
                mutated["cases"]["ver_ws"][field] += 1
                problems = entity_read.check_accepted(scoped, mutated)
                self.assertB1Problems(problems, f"ver_ws:{field}")
        with self.subTest(case="ver_ws", path="id"):
            mutated = copy.deepcopy(accepted)
            mutated["cases"]["ver_ws"]["id"] = "ver_ws_other"
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "ver_ws:id")
        for field in ("record_hex", "preimage_hex", "stored_hex", "object_id"):
            with self.subTest(case="ver_ws", path=f"object:ws:{field}"):
                mutated = copy.deepcopy(accepted)
                original = mutated["cases"]["ver_ws"]["objects"]["ws"][field]
                mutated["cases"]["ver_ws"]["objects"]["ws"][field] = _b1_flip_hex(original)
                self.assertNotEqual(mutated["cases"]["ver_ws"]["objects"]["ws"][field], original)
                problems = entity_read.check_accepted(scoped, mutated)
                self.assertB1Problems(problems, f"ver_ws:object:ws:{field}")
        for field in ("id", "request_preimage_hex", "response_preimage_hex", "response_wire_len"):
            with self.subTest(case="ver_ws", path=f"remove-{field}"):
                mutated = copy.deepcopy(accepted)
                del mutated["cases"]["ver_ws"][field]
                problems = entity_read.check_accepted(scoped, mutated)
                self.assertB1Problems(problems, f"ver_ws:{field}")
        with self.subTest(case="ver_ws", path="remove-object-record_hex"):
            mutated = copy.deepcopy(accepted)
            del mutated["cases"]["ver_ws"]["objects"]["ws"]["record_hex"]
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "ver_ws:object:ws:record_hex")
        with self.subTest(case="ver_ws", path="extra-case-field"):
            mutated = copy.deepcopy(accepted)
            mutated["cases"]["ver_ws"]["unexpected_field"] = 1
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "ver_ws:fields")
        with self.subTest(case="ver_ws", path="extra-object-field"):
            mutated = copy.deepcopy(accepted)
            mutated["cases"]["ver_ws"]["objects"]["ws"]["unexpected_field"] = 1
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "ver_ws:object:ws:fields")
        with self.subTest(case="ver_ws", path="count_k-bool"):
            mutated = copy.deepcopy(accepted)
            self.assertEqual(mutated["cases"]["ver_ws"]["count_k"], 1)
            mutated["cases"]["ver_ws"]["count_k"] = True
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "ver_ws:count_k")

    def test_hello_and_selection_fields_are_bound(self) -> None:
        scoped, accepted, _rejected = _b1_control()
        self.assertEqual(entity_read.check_accepted(scoped, accepted), [])
        with self.subTest(path="hello:hello_v2_client:body"):
            mutated = copy.deepcopy(accepted)
            original = mutated["hellos"]["hello_v2_client"]["body_hex"]
            mutated["hellos"]["hello_v2_client"]["body_hex"] = _b1_flip_hex(original)
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "hello:hello_v2_client:body")
        with self.subTest(path="hello:hello_v2_client:fields"):
            mutated = copy.deepcopy(accepted)
            mutated["hellos"]["hello_v2_client"]["unexpected_field"] = 1
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "hello:hello_v2_client:fields")
        with self.subTest(path="selection:v2_select:version"):
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["v2_select"]["protocol_version"] = 1
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v2_select:version")
        with self.subTest(path="selection:v2_select:methods"):
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["v2_select"]["methods"] = [100]
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v2_select:methods")
        with self.subTest(path="selection:v2_select:epoch"):
            mutated = copy.deepcopy(accepted)
            original = mutated["selections"]["v2_select"]["schema_epoch"]
            mutated["selections"]["v2_select"]["schema_epoch"] = _b1_flip_hex(original)
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v2_select:epoch")
        for field in ("preimage_hex", "transcript_hex", "handshake_id"):
            suffix = {"preimage_hex": "preimage", "transcript_hex": "transcript", "handshake_id": "handshake"}[field]
            with self.subTest(path=f"selection:v2_select:{suffix}"):
                mutated = copy.deepcopy(accepted)
                mutated["selections"]["v2_select"][field] = _b1_flip_hex(mutated["selections"]["v2_select"][field])
                problems = entity_read.check_accepted(scoped, mutated)
                self.assertB1Problems(problems, f"selection:v2_select:{suffix}")
        limit_keys = (
            "max_frame_bytes",
            "max_entities",
            "max_edges",
            "max_depth",
            "max_response_bytes",
            "max_work",
            "max_inflight",
            "max_sessions",
        )
        for key in limit_keys:
            with self.subTest(path="selection:v2_select:limits", limit=key):
                mutated = copy.deepcopy(accepted)
                mutated["selections"]["v2_select"]["limits"][key] += 1
                problems = entity_read.check_accepted(scoped, mutated)
                self.assertB1Problems(problems, "selection:v2_select:limits")
        with self.subTest(path="selection:v2_select:features"):
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["v2_select"]["features"] ^= 1
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v2_select:features")
        with self.subTest(path="selection:v2_select:adapters"):
            self.assertNotEqual(accepted["selections"]["v2_select"]["adapters"], [])
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["v2_select"]["adapters"] = []
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v2_select:adapters")
        with self.subTest(path="selection:v2_select:effects"):
            self.assertNotEqual(accepted["selections"]["v2_select"]["effects"], [])
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["v2_select"]["effects"] = []
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v2_select:effects")
        with self.subTest(path="selection:v2_select:remove-limits"):
            mutated = copy.deepcopy(accepted)
            del mutated["selections"]["v2_select"]["limits"]
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v2_select:limits")
        with self.subTest(path="selection:v2_select:fields"):
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["v2_select"]["unexpected_field"] = 1
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v2_select:fields")
        with self.subTest(path="selection:v3_refuse:missing-expected-failure"):
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["v3_refuse"]["expected_failure"] = "other"
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v3_refuse:missing-expected-failure")
        with self.subTest(path="selection:v3_refuse:fields"):
            mutated = copy.deepcopy(accepted)
            mutated["selections"]["v3_refuse"]["unexpected_field"] = 1
            problems = entity_read.check_accepted(scoped, mutated)
            self.assertB1Problems(problems, "selection:v3_refuse:fields")

    def test_rejected_same_layer_bytes_are_reconstructed(self) -> None:
        scoped, _accepted, rejected = _b1_control()
        self.assertEqual(entity_read.check_rejected(scoped, rejected), [])
        authored = next(row for row in scoped["rejected"] if row["id"] == "req_missing_field")
        base = scoped["cases"][authored["base"]]
        original_bytes = entity_read.build_rejected_bytes(scoped, base, authored["recipe"])
        alternative_recipe = dict(authored["recipe"])
        alternative_recipe["tag"] = 4
        alternative_bytes = entity_read.build_rejected_bytes(scoped, base, alternative_recipe)
        self.assertNotEqual(alternative_bytes, original_bytes)
        self.assertEqual(
            entity_read.validate_rejected(scoped, {"recipe": {"target": "request"}}, alternative_bytes),
            ("request_record", "SCB_FIELD_MISSING"),
        )
        mutated = copy.deepcopy(rejected)
        target = _b1_case_by_id(mutated["cases"], "req_missing_field")
        target["input_hex"] = alternative_bytes.hex()
        problems = entity_read.check_rejected(scoped, mutated)
        self.assertB1Problems(problems, "rejected:req_missing_field:input_hex")

    def test_rejected_expectation_metadata_cannot_authorize_itself(self) -> None:
        scoped, _accepted, rejected = _b1_control()
        self.assertEqual(entity_read.check_rejected(scoped, rejected), [])
        with self.subTest(path="rejected:req_missing_field:expected_scb"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "req_missing_field")["expected_scb"] = None
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:req_missing_field:expected_scb")
        with self.subTest(path="rejected:req_missing_field:expected_code"):
            mutated = copy.deepcopy(rejected)
            target = _b1_case_by_id(mutated["cases"], "req_missing_field")
            target["expected_code"] = 40009
            target["expected_symbol"] = "PROTOCOL_LIMIT_EXCEEDED"
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:req_missing_field:expected_code", "rejected:req_missing_field:expected_symbol")
        with self.subTest(path="rejected:req_missing_field:recipe"):
            mutated = copy.deepcopy(rejected)
            target = _b1_case_by_id(mutated["cases"], "req_missing_field")
            altered = dict(target["recipe"])
            altered["tag"] = 4
            target["recipe"] = altered
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:req_missing_field:recipe")
        with self.subTest(path="rejected:req_missing_field:coherent-replacement"):
            duplicate_authored = next(row for row in scoped["rejected"] if row["id"] == "req_duplicate_field")
            base = scoped["cases"][duplicate_authored["base"]]
            duplicate_bytes = entity_read.build_rejected_bytes(scoped, base, duplicate_authored["recipe"])
            self.assertEqual(
                entity_read.validate_rejected(scoped, {"recipe": {"target": "request"}}, duplicate_bytes),
                ("request_record", "SCB_FIELD_DUPLICATE"),
            )
            mutated = copy.deepcopy(rejected)
            target = _b1_case_by_id(mutated["cases"], "req_missing_field")
            target["recipe"] = copy.deepcopy(duplicate_authored["recipe"])
            target["input_hex"] = duplicate_bytes.hex()
            target["expected_scb"] = duplicate_authored["expected_scb"]
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:req_missing_field:input_hex")
        with self.subTest(control="wrong-layer-still-refused"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "req_missing_field")["failing_layer"] = "response_record"
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "req_missing_field:layer:request_record")
        with self.subTest(control="wrong-scb-still-refused"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "req_missing_field")["expected_scb"] = "SCB_FIELD_DUPLICATE"
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "req_missing_field:scb:SCB_FIELD_MISSING")

    def test_derived_and_pending_rows_remain_bound(self) -> None:
        scoped, _accepted, rejected = _b1_control()
        self.assertEqual(entity_read.check_rejected(scoped, rejected), [])
        with self.subTest(path="bound_k_exact:k-relation"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "bound_k_exact")["max_objects"] = 999
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "bound_k_exact:k-relation")
        with self.subTest(path="rejected:bound_k_exact:note"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "bound_k_exact")["note"] = "changed note"
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:bound_k_exact:note")
        with self.subTest(path="rejected:bound_k_exact:expected_symbol"):
            mutated = copy.deepcopy(rejected)
            target = _b1_case_by_id(mutated["cases"], "bound_k_exact")
            target["expected_symbol"] = "PROTOCOL_PAYLOAD_INVALID"
            target["expected_code"] = 40008
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:bound_k_exact:expected_symbol")
        with self.subTest(path="rejected:seq_wrong_session:title"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "seq_wrong_session")["title"] = "changed title"
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:seq_wrong_session:title")
        with self.subTest(path="rejected:seq_wrong_session:steps"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "seq_wrong_session")["steps"][0]["action"] = "changed action"
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:seq_wrong_session:steps")
        with self.subTest(path="rejected:seq_wrong_session:steps-code"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "seq_wrong_session")["steps"][0]["expected_code"] = 40008
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:seq_wrong_session:steps")
        with self.subTest(path="rejected:seq_wrong_session:observations"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "seq_wrong_session")["observations"] = ["changed"]
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:seq_wrong_session:observations")
        with self.subTest(path="rejected:seq_wrong_session:note"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "seq_wrong_session")["note"] = "changed note"
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:seq_wrong_session:note")
        with self.subTest(path="seq_wrong_session:stateful-must-be-pending"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "seq_wrong_session")["status"] = "complete"
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "seq_wrong_session:stateful-must-be-pending")
        with self.subTest(control="fail_root_mismatch:failure-retryability"):
            mutated = copy.deepcopy(rejected)
            _b1_case_by_id(mutated["cases"], "fail_root_mismatch")["expected_retryability"] = 4
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "fail_root_mismatch:failure-retryability")
        with self.subTest(path="rejected:fail_root_mismatch:input_hex"):
            full = load_authored_inputs()
            substitute = next(row for row in full["rejected"] if row["id"] == "fail_limit")
            substitute_wire = entity_read.build_failure_wire(scoped, substitute).hex()
            original_wire = _b1_case_by_id(rejected["cases"], "fail_root_mismatch")["input_hex"]
            self.assertNotEqual(substitute_wire, original_wire)
            layer, _code = entity_read.validate_rejected(
                scoped, {"recipe": {"target": "failure"}}, bytes.fromhex(substitute_wire)
            )
            self.assertEqual(layer, "failure_accepted")
            mutated = copy.deepcopy(rejected)
            target = _b1_case_by_id(mutated["cases"], "fail_root_mismatch")
            target["input_hex"] = substitute_wire
            target["code"] = substitute["code"]
            target["symbol"] = substitute["symbol"]
            target["expected_code"] = substitute["expected_code"]
            target["expected_symbol"] = substitute["expected_symbol"]
            target["expected_retryability"] = substitute["expected_retryability"]
            problems = entity_read.check_rejected(scoped, mutated)
            self.assertB1Problems(problems, "rejected:fail_root_mismatch:input_hex")

    def test_document_contract_claim_and_extra_fields_are_bound(self) -> None:
        scoped, accepted, rejected = _b1_control()
        self.assertEqual(entity_read.check_accepted(scoped, accepted), [])
        self.assertEqual(entity_read.check_rejected(scoped, rejected), [])
        with self.subTest(path="accepted-contract"):
            mutated = copy.deepcopy(accepted)
            mutated["contract"] = "sley2-entity-read-v2-wrong"
            self.assertB1Problems(entity_read.check_accepted(scoped, mutated), "accepted-contract")
        with self.subTest(path="accepted-claim"):
            mutated = copy.deepcopy(accepted)
            mutated["claim"] = "wrong-claim"
            self.assertB1Problems(entity_read.check_accepted(scoped, mutated), "accepted-claim")
        with self.subTest(path="accepted-missing-contract"):
            mutated = copy.deepcopy(accepted)
            del mutated["contract"]
            self.assertB1Problems(entity_read.check_accepted(scoped, mutated), "accepted-contract")
        with self.subTest(path="accepted-missing-claim"):
            mutated = copy.deepcopy(accepted)
            del mutated["claim"]
            self.assertB1Problems(entity_read.check_accepted(scoped, mutated), "accepted-claim")
        with self.subTest(path="accepted:fields"):
            mutated = copy.deepcopy(accepted)
            mutated["unexpected_top_level"] = 1
            self.assertB1Problems(entity_read.check_accepted(scoped, mutated), "accepted:fields")
        with self.subTest(path="rejected-contract"):
            mutated = copy.deepcopy(rejected)
            mutated["contract"] = "wrong-contract"
            self.assertB1Problems(entity_read.check_rejected(scoped, mutated), "rejected-contract")
        with self.subTest(path="rejected-claim"):
            mutated = copy.deepcopy(rejected)
            mutated["claim"] = "wrong-claim"
            self.assertB1Problems(entity_read.check_rejected(scoped, mutated), "rejected-claim")
        with self.subTest(path="rejected-missing-contract"):
            mutated = copy.deepcopy(rejected)
            del mutated["contract"]
            self.assertB1Problems(entity_read.check_rejected(scoped, mutated), "rejected-contract")
        with self.subTest(path="rejected-missing-claim"):
            mutated = copy.deepcopy(rejected)
            del mutated["claim"]
            self.assertB1Problems(entity_read.check_rejected(scoped, mutated), "rejected-claim")
        with self.subTest(path="rejected:fields"):
            mutated = copy.deepcopy(rejected)
            mutated["unexpected_top_level"] = 1
            self.assertB1Problems(entity_read.check_rejected(scoped, mutated), "rejected:fields")

    def test_explicit_rejected_kind_must_be_known_string(self) -> None:
        scoped, _accepted, rejected = _b1_control()
        self.assertEqual(rejected["contract"], "sley2-entity-read-v2-rejected")
        self.assertEqual(rejected["claim"], "independent-expected")
        self.assertEqual(scoped["rejected"][0]["id"], "req_missing_field")
        self.assertEqual(rejected["cases"][0]["id"], "req_missing_field")
        self.assertNotIn("kind", scoped["rejected"][0])
        self.assertNotIn("kind", rejected["cases"][0])
        self.assertEqual(entity_read.check_rejected(scoped, rejected), [])
        variants = (
            ("null", None),
            ("list", []),
            ("object", {}),
            ("false", False),
            ("zero", 0),
            ("one", 1),
            ("empty-string", ""),
            ("unknown-string", "future"),
        )
        for label, kind in variants:
            with self.subTest(kind=label):
                inputs = copy.deepcopy(scoped)
                doc = copy.deepcopy(rejected)
                inputs["rejected"][0]["kind"] = kind
                doc["cases"][0]["kind"] = kind
                doc["manifest"]["inputs_sha256"] = hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest()
                original = copy.deepcopy((inputs, doc))
                try:
                    problems = entity_read.check_rejected(inputs, doc)
                except Exception as error:
                    self.fail(f"uncontrolled {type(error).__name__} for kind={label!r}: {error!r}")
                self.assertEqual((inputs, doc), original)
                self.assertB1Problems(problems, "rejected:req_missing_field:kind")

    def test_mismatching_fill_length_cannot_drive_expansion(self) -> None:
        scoped = _b1_scoped_inputs()
        authored = copy.deepcopy(next(row for row in load_authored_inputs()["rejected"] if row["id"] == "fill_bytes_exact"))
        scoped["rejected"] = [copy.deepcopy(authored)]
        inputs_sha = hashlib.sha256(json.dumps(scoped, sort_keys=True).encode()).hexdigest()
        doc = {
            "contract": "sley2-entity-read-v2-rejected",
            "claim": "independent-expected",
            "manifest": {"inputs_sha256": inputs_sha},
            "cases": [copy.deepcopy(authored)],
        }
        self.assertEqual(authored["fill_length"], 16777216)
        self.assertEqual(doc["contract"], "sley2-entity-read-v2-rejected")
        self.assertEqual(doc["claim"], "independent-expected")
        self.assertEqual(doc["manifest"]["inputs_sha256"], hashlib.sha256(json.dumps(scoped, sort_keys=True).encode()).hexdigest())
        self.assertEqual(len(doc["cases"]), 1)
        self.assertEqual(entity_read.check_rejected(scoped, doc), [])
        doc["cases"][0]["fill_length"] = 2**64
        original = copy.deepcopy((scoped, doc))
        try:
            problems = entity_read.check_rejected(scoped, doc)
        except Exception as error:
            self.fail(f"uncontrolled {type(error).__name__}: {error!r}")
        self.assertEqual((scoped, doc), original)
        self.assertB1Problems(problems, "rejected:fill_bytes_exact:fill_length")


_B2_LIMIT_KEYS = (
    "max_frame_bytes",
    "max_entities",
    "max_edges",
    "max_depth",
    "max_response_bytes",
    "max_work",
    "max_inflight",
    "max_sessions",
)
_B2_U32_LIMIT_TAGS = frozenset({4, 7, 8})


def _b2_load(case_id):
    inputs = load_authored_inputs()
    case = inputs["cases"][case_id]
    built = entity_read.build_success_case(inputs, case)
    return inputs, case, built


def _b2_max_frame(inputs):
    return int(inputs["selected_limits"]["max_frame_bytes"])


def _b2_valid_frame(inputs, built, side):
    wire = bytes.fromhex(built[f"{side}_wire_hex"])
    stored, _prefix = entity_read.split_wire(wire, _b2_max_frame(inputs))
    payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
    frame = entity_read.decode_frame_payload(payload)
    return wire, stored, payload, frame


def _b2_supplied_wire_only(built, side, mutated_wire):
    supplied = copy.deepcopy(built)
    supplied[f"{side}_wire_hex"] = mutated_wire.hex()
    if side == "response":
        supplied["response_wire_len"] = len(mutated_wire)
    return supplied


def _b2_supplied_with_payload(built, side, mutated_payload):
    supplied = copy.deepcopy(built)
    wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), mutated_payload)
    supplied[f"{side}_wire_hex"] = wire.hex()
    supplied[f"{side}_preimage_hex"] = preimage.hex()
    supplied[f"{side}_frame_id"] = frame_id.hex()
    if side == "response":
        supplied["response_wire_len"] = len(wire)
    return supplied


def _b2_raw_envelope(payload, *, magic, format_value, tag_value, epoch):
    preimage = magic + encode_uvar(format_value) + encode_uvar(tag_value) + epoch + encode_sized(payload)
    trailer = blake3.blake3(entity_read.FRAME_DOMAIN + preimage).digest()
    stored = preimage + trailer
    wire = len(stored).to_bytes(8, "big") + stored
    return wire, preimage, trailer


def _b2_expect(testcase, inputs, case, supplied, side, layer, code):
    problems = []
    entity_read.semantic_check(inputs, case, supplied, problems)
    head = f"{case['id']}:semantic:{side}:{layer}"
    testcase.assertIsInstance(problems, list)
    testcase.assertTrue(any(head in entry for entry in problems), f"missing {head} in {problems!r}")
    if code is not None:
        testcase.assertTrue(any(code in entry for entry in problems), f"missing {code} in {problems!r}")


def _b2_check_request_zero_limits(testcase, inputs, frame):
    bound_fields = entity_read.parse_record(frame["bounds"])
    limits_raw = entity_read.single_field(bound_fields, 1)
    limit_fields = entity_read.parse_record(limits_raw)
    testcase.assertEqual(sorted(tag for tag, _ in limit_fields), [1, 2, 3, 4, 5, 6, 7, 8])
    for tag, payload in limit_fields:
        width = 32 if tag in _B2_U32_LIMIT_TAGS else 64
        testcase.assertEqual(entity_read.decode_uvar_exact(payload, width), 0)


def _b2_check_response_selected_limits(testcase, inputs, frame):
    bound_fields = entity_read.parse_record(frame["bounds"])
    limits_raw = entity_read.single_field(bound_fields, 1)
    limit_fields = entity_read.parse_record(limits_raw)
    testcase.assertEqual(sorted(tag for tag, _ in limit_fields), [1, 2, 3, 4, 5, 6, 7, 8])
    for (tag, payload), key in zip(sorted(limit_fields), _B2_LIMIT_KEYS):
        width = 32 if tag in _B2_U32_LIMIT_TAGS else 64
        testcase.assertEqual(entity_read.decode_uvar_exact(payload, width), int(inputs["selected_limits"][key]))


class SuppliedEntityFrameCases(unittest.TestCase):
    def test_actual_request_and_response_controls(self) -> None:
        for case_id in ("ver_ws", "sig_multi"):
            with self.subTest(case=case_id):
                inputs, case, built = _b2_load(case_id)
                problems = []
                entity_read.semantic_check(inputs, case, built, problems)
                self.assertEqual(problems, [])
                for side in ("request", "response"):
                    with self.subTest(side=side):
                        wire, _stored, payload, frame = _b2_valid_frame(inputs, built, side)
                        self.assertEqual(frame["version"], 2)
                        self.assertEqual(frame["flags"], 0)
                        if side == "request":
                            decoded = entity_read.decode_request_body(frame["body"], {"limits": inputs["selected_limits"]})
                            self.assertEqual(decoded["root"], bytes.fromhex(inputs["context"]["root"]))
                            _b2_check_request_zero_limits(self, inputs, frame)
                            bounds = entity_read.decode_bounds(frame["bounds"])
                            self.assertEqual((bounds["returned_bytes"], bounds["returned_entities"]), (0, 0))
                            self.assertFalse(bounds["truncated"])
                            self.assertFalse(bounds["continuation"])
                        else:
                            response = entity_read.decode_response_body(frame["body"])
                            bounds = entity_read.decode_bounds(frame["bounds"])
                            self.assertEqual(bounds["returned_entities"], built["count_k"])
                            self.assertEqual(bounds["returned_bytes"], len(frame["body"]))
                            self.assertEqual(response["work"], built["work"])
                            _b2_check_response_selected_limits(self, inputs, frame)

    def test_supplied_wire_prefix_and_exhaustion(self) -> None:
        inputs, case, built = _b2_load("ver_ws")
        max_frame = _b2_max_frame(inputs)
        for side in ("request", "response"):
            wire = bytes.fromhex(built[f"{side}_wire_hex"])
            with self.subTest(side=side, variant="control-split"):
                stored, prefix = entity_read.split_wire(wire, max_frame)
                self.assertEqual(int.from_bytes(prefix, "big"), len(stored))
            length = int.from_bytes(wire[:8], "big")
            variants = (
                ("short-prefix", wire[:7]),
                ("prefix-plus-one", (length + 1).to_bytes(8, "big") + wire[8:]),
                ("prefix-minus-one", (length - 1).to_bytes(8, "big") + wire[8:]),
                ("trailing-byte", wire + b"\x00"),
            )
            for name, mutated in variants:
                with self.subTest(side=side, variant=name):
                    supplied = _b2_supplied_wire_only(built, side, mutated)
                    _b2_expect(self, inputs, case, supplied, side, "wire_prefix", None)
            with self.subTest(side=side, variant="beyond-ceiling"):
                over = (max_frame + 1).to_bytes(8, "big") + wire[8:]
                supplied = _b2_supplied_wire_only(built, side, over)
                _b2_expect(self, inputs, case, supplied, side, "wire_ceiling", None)

    def test_supplied_envelope_integrity_and_identity(self) -> None:
        for side in ("request", "response"):
            inputs, case, built = _b2_load("ver_ws")
            wire = bytes.fromhex(built[f"{side}_wire_hex"])
            _wire, stored, payload, _frame = _b2_valid_frame(inputs, built, side)
            epoch = entity_read.protocol_epoch_id()
            with self.subTest(side=side, variant="canonical-raw"):
                raw_wire, raw_pre, raw_trailer = _b2_raw_envelope(
                    payload,
                    magic=entity_read.MAGIC,
                    format_value=entity_read.FORMAT_VERSION,
                    tag_value=entity_read.FRAME_CONTRACT_TAG,
                    epoch=epoch,
                )
                self.assertEqual(raw_wire, wire)
                self.assertEqual(raw_pre.hex(), built[f"{side}_preimage_hex"])
                self.assertEqual(raw_trailer.hex(), built[f"{side}_frame_id"])
            with self.subTest(side=side, variant="bad-digest"):
                preimage, trailer = stored[:-32], stored[-32:]
                self.assertEqual(int.from_bytes(wire[:8], "big"), len(stored))
                self.assertEqual(blake3.blake3(entity_read.FRAME_DOMAIN + preimage).digest(), trailer)
                bad_trailer = trailer[:-1] + bytes([trailer[-1] ^ 0x01])
                bad_stored = preimage + bad_trailer
                self.assertNotEqual(blake3.blake3(entity_read.FRAME_DOMAIN + preimage).digest(), bad_trailer)
                bad_wire = wire[:8] + bad_stored
                supplied = copy.deepcopy(built)
                supplied[f"{side}_wire_hex"] = bad_wire.hex()
                supplied[f"{side}_frame_id"] = bad_trailer.hex()
                if side == "response":
                    supplied["response_wire_len"] = len(bad_wire)
                _b2_expect(self, inputs, case, supplied, side, "frame_envelope", "SCB_DIGEST_MISMATCH")
            bad_epoch = bytes([epoch[0] ^ 0x01]) + epoch[1:]
            rows = (
                ("bad-magic", b"X" + entity_read.MAGIC[1:], entity_read.FORMAT_VERSION, entity_read.FRAME_CONTRACT_TAG, epoch, "SCB_MAGIC_INVALID"),
                ("bad-format", entity_read.MAGIC, entity_read.FORMAT_VERSION + 1, entity_read.FRAME_CONTRACT_TAG, epoch, "SCB_VERSION_UNSUPPORTED"),
                ("bad-tag", entity_read.MAGIC, entity_read.FORMAT_VERSION, entity_read.FRAME_CONTRACT_TAG + 1, epoch, "SCB_CONTRACT_UNKNOWN"),
                ("bad-epoch", entity_read.MAGIC, entity_read.FORMAT_VERSION, entity_read.FRAME_CONTRACT_TAG, bad_epoch, "SCB_EPOCH_MISMATCH"),
            )
            for name, magic, fmt, tag, ep, code in rows:
                with self.subTest(side=side, variant=name):
                    mal_wire, mal_pre, mal_trailer = _b2_raw_envelope(payload, magic=magic, format_value=fmt, tag_value=tag, epoch=ep)
                    self.assertEqual(blake3.blake3(entity_read.FRAME_DOMAIN + mal_pre).digest(), mal_trailer)
                    self.assertEqual(int.from_bytes(mal_wire[:8], "big"), len(mal_wire) - 8)
                    supplied = copy.deepcopy(built)
                    supplied[f"{side}_wire_hex"] = mal_wire.hex()
                    supplied[f"{side}_preimage_hex"] = mal_pre.hex()
                    supplied[f"{side}_frame_id"] = mal_trailer.hex()
                    if side == "response":
                        supplied["response_wire_len"] = len(mal_wire)
                    _b2_expect(self, inputs, case, supplied, side, "frame_envelope", code)

    def test_supplied_frame_structure_and_bounds_structure(self) -> None:
        for side in ("request", "response"):
            inputs, case, built = _b2_load("ver_ws")
            _wire, _stored, _payload, frame = _b2_valid_frame(inputs, built, side)
            base_payload = _payload
            frame_rows = []
            fields = entity_read.parse_record(base_payload)
            frame_rows.append(("missing-field", [(t, p) for t, p in fields if t != 5], "SCB_FIELD_MISSING"))
            frame_rows.append(("extra-field", fields + [(9, b"")], "SCB_FIELD_UNKNOWN"))
            dup = []
            inserted = False
            for tag, payload in fields:
                dup.append((tag, payload))
                if tag == 1 and not inserted:
                    dup.append((tag, payload))
                    inserted = True
            frame_rows.append(("duplicate-field", dup, "SCB_FIELD_DUPLICATE"))
            frame_rows.append(("reordered-fields", list(reversed(fields)), "SCB_FIELD_ORDER"))
            bad_session = encode_uvar(1) + encode_sized(frame["session"][:31] if frame["session"] is not None else b"\x00" * 31)
            frame_rows.append(("bad-session-width", [(t, bad_session if t == 2 else p) for t, p in fields], "SCB_UNION_INVALID"))
            frame_rows.append(("body-trailing", [(t, (encode_sized(frame["body"]) + b"\x00") if t == 8 else p) for t, p in fields], "SCB_TRAILING_BYTES"))
            for name, mutated_fields, code in frame_rows:
                with self.subTest(side=side, variant=name):
                    mutated_payload = entity_read.encode_fields(mutated_fields)
                    supplied = _b2_supplied_with_payload(built, side, mutated_payload)
                    _b2_expect(self, inputs, case, supplied, side, "frame_payload", code)
            bounds_raw = frame["bounds"]
            bound_fields = entity_read.parse_record(bounds_raw)
            bound_by_tag = {tag: payload for tag, payload in bound_fields}
            bound_rows = (
                ("bounds-missing-field-1", [(t, p) for t, p in bound_fields if t != 1], "SCB_FIELD_MISSING"),
                ("bounds-extra-field", bound_fields + [(9, b"")], "SCB_FIELD_UNKNOWN"),
                ("bounds-trailing", None, "SCB_TRAILING_BYTES"),
            )
            for name, mutated_fields, code in bound_rows:
                with self.subTest(side=side, variant=name):
                    if mutated_fields is None:
                        mutated_bounds = bounds_raw + b"\x00"
                    else:
                        mutated_bounds = entity_read.encode_fields(mutated_fields)
                    mutated_payload = entity_read.build_frame_payload(
                        frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"], frame["flags"], mutated_bounds, frame["body"]
                    )
                    supplied = _b2_supplied_with_payload(built, side, mutated_payload)
                    _b2_expect(self, inputs, case, supplied, side, "frame_bounds", code)
            limits_raw = bound_by_tag[1]
            limit_fields = entity_read.parse_record(limits_raw)
            for tag, _payload in sorted(limit_fields):
                with self.subTest(side=side, variant=f"limit-field-{tag}-truncated"):
                    mutated_limits = [(t, (b"\x80" if t == tag else p)) for t, p in limit_fields]
                    mutated_bounds = entity_read.encode_fields([(t, (entity_read.encode_fields(mutated_limits) if t == 1 else p)) for t, p in bound_fields])
                    mutated_payload = entity_read.build_frame_payload(
                        frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"], frame["flags"], mutated_bounds, frame["body"]
                    )
                    supplied = _b2_supplied_with_payload(built, side, mutated_payload)
                    _b2_expect(self, inputs, case, supplied, side, "frame_bounds", "SCB_LENGTH_OVERFLOW")

    def test_supplied_frame_version_and_header(self) -> None:
        for side in ("request", "response"):
            inputs, case, built = _b2_load("ver_ws")
            _wire, _stored, _payload, frame = _b2_valid_frame(inputs, built, side)
            self.assertEqual(frame["version"], 2)
            self.assertEqual(frame["flags"], 0)
            self.assertEqual(inputs["selected_limits"]["max_frame_bytes"], 8388608)
            rows = [
                ("version-1", {"version": 1}, "PROTOCOL_DOWNGRADE"),
                ("version-3", {"version": 3}, "PROTOCOL_VERSION_UNSUPPORTED"),
                ("flag-8", {"flags": 8}, "PROTOCOL_FRAME_INVALID"),
                ("kind-0", {"kind": 0}, "PROTOCOL_FRAME_INVALID"),
                ("kind-5", {"kind": 5}, "PROTOCOL_FRAME_INVALID"),
                ("version-1-flag-8", {"version": 1, "flags": 8}, "PROTOCOL_DOWNGRADE"),
                ("version-3-flag-8", {"version": 3, "flags": 8}, "PROTOCOL_VERSION_UNSUPPORTED"),
            ]
            if side == "request":
                rows.append(("flag-4", {"flags": 4}, "PROTOCOL_FRAME_INVALID"))
            for name, changes, code in rows:
                with self.subTest(side=side, variant=name):
                    mutated_payload = entity_read.build_frame_payload(
                        changes.get("version", frame["version"]),
                        frame["session"],
                        frame["request_id"],
                        changes.get("kind", frame["kind"]),
                        frame["method"],
                        changes.get("flags", frame["flags"]),
                        frame["bounds"],
                        frame["body"],
                    )
                    supplied = _b2_supplied_with_payload(built, side, mutated_payload)
                    # Unknown kinds never reach version comparison: typed kind decoding owns the refusal.
                    _b2_expect(self, inputs, case, supplied, side, "frame_header", code)


if __name__ == "__main__":
    unittest.main()
