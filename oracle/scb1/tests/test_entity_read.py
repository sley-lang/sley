"""Focused grammar and semantic rejection tests for the entity-read oracle.

Stage A scope: meaningful rejection behavior only (malformed varints and
records, digest integrity, declaration-order and relationship checks, work
arithmetic, negotiation refusals). Stage A tests construct their own tiny
in-memory fixtures through the oracle module and claim no corpus or
runtime coverage.

B2 outer integration scope (B2OuterIntegrationCases): complete controls that
read the tracked hand-authored inputs and compare whole accepted/rejected
documents through the real checkers. Refresh/CLI staging, where present, runs
in private temporary paths outside the repository and claims no corpus
promotion or runtime coverage.

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
    accepted = {
        "contract": "sley2-entity-read-v2",
        "claim": "independent-expected",
        "manifest": {"inputs_sha256": inputs_sha},
        "cases": cases,
        "hellos": hellos,
        "selections": selections,
    }
    if "frame_scenarios" in scoped:
        matrix = {}
        offer = scoped["hellos"]["hello_v2_client"]
        for key, wire_version, expected_version, expect, expected_layer, expected_code in _B2_T4_MATRIX:
            body = entity_read.build_hello(offer)
            payload = entity_read.build_frame_payload(
                wire_version, None, 0, 4, 0, 0, entity_read.zero_bounds(), body
            )
            wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
            matrix[key] = {
                "hello": "hello_v2_client",
                "wire_version": wire_version,
                "expected_version": expected_version,
                "expect": expect,
                "expected_layer": expected_layer,
                "expected_code": expected_code,
                "body_hex": body.hex(),
                "wire_hex": wire.hex(),
                "preimage_hex": preimage.hex(),
                "frame_id": frame_id.hex(),
            }
        accepted["frame_scenarios"] = matrix
    return accepted


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


def _b2_prefix_matches(problems, prefix, code):
    if not isinstance(problems, list):
        return False
    for entry in problems:
        if not isinstance(entry, str):
            return False
    for entry in problems:
        if code is None:
            if entry == prefix or entry.startswith(prefix + ":"):
                return True
        else:
            target = prefix + ":" + code
            if entry == target or entry.startswith(target + ":") or entry.startswith(target + " "):
                return True
    return False


def _b2_matches(problems, case_id, side, layer, code):
    return _b2_prefix_matches(problems, f"{case_id}:semantic:{side}:{layer}", code)


def _b2_hello_matches(problems, row_id, layer, code):
    return _b2_prefix_matches(problems, f"hello_frame:{row_id}:{layer}", code)


def _b2_hello_expect(testcase, problems, row_id, layer, code):
    testcase.assertIsInstance(problems, list)
    for entry in problems:
        testcase.assertIsInstance(entry, str)
    target = f"hello_frame:{row_id}:{layer}:{code}"
    testcase.assertTrue(_b2_hello_matches(problems, row_id, layer, code), f"missing {target} in {problems!r}")


def _b2_expect(testcase, inputs, case, supplied, side, layer, code):
    problems = []
    entity_read.semantic_check(inputs, case, supplied, problems)
    testcase.assertIsInstance(problems, list)
    for entry in problems:
        testcase.assertIsInstance(entry, str)
    prefix = f"{case['id']}:semantic:{side}:{layer}"
    if code is None:
        testcase.assertTrue(_b2_matches(problems, case["id"], side, layer, code), f"missing {prefix} in {problems!r}")
    else:
        target = prefix + ":" + code
        testcase.assertTrue(_b2_matches(problems, case["id"], side, layer, code), f"missing {target} in {problems!r}")


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


def _b2_supplied_with_request_body(inputs, built, frame, mutated_body):
    mutated_payload = entity_read.build_frame_payload(
        frame["version"],
        frame["session"],
        frame["request_id"],
        frame["kind"],
        frame["method"],
        frame["flags"],
        frame["bounds"],
        mutated_body,
    )
    supplied = copy.deepcopy(built)
    wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), mutated_payload)
    supplied["request_body_hex"] = mutated_body.hex()
    supplied["request_wire_hex"] = wire.hex()
    supplied["request_preimage_hex"] = preimage.hex()
    supplied["request_frame_id"] = frame_id.hex()
    return supplied


def _b2_supplied_with_response_body(inputs, built, frame, mutated_body, *, update_body=True):
    bounds = entity_read.build_bounds(inputs["selected_limits"], len(mutated_body), built["count_k"])
    mutated_payload = entity_read.build_frame_payload(
        frame["version"],
        frame["session"],
        frame["request_id"],
        frame["kind"],
        frame["method"],
        frame["flags"],
        bounds,
        mutated_body,
    )
    supplied = copy.deepcopy(built)
    wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), mutated_payload)
    if update_body:
        supplied["response_body_hex"] = mutated_body.hex()
        body_fields = entity_read.parse_record(mutated_body)
        supplied["work"] = entity_read.decode_uvar_exact(entity_read.single_field(body_fields, 8), 64)
    supplied["response_wire_hex"] = wire.hex()
    supplied["response_preimage_hex"] = preimage.hex()
    supplied["response_frame_id"] = frame_id.hex()
    supplied["response_wire_len"] = len(wire)
    return supplied


def _b2_uvar_len(value):
    assert isinstance(value, int) and value >= 0
    return max(1, (int(value).bit_length() + 6) // 7)


def _b2_sig_base(testcase):
    inputs, case, built = _b2_load("sig_multi")
    problems = []
    entity_read.semantic_check(inputs, case, built, problems)
    testcase.assertEqual(problems, [])
    _wire, _stored, _payload, frame = _b2_valid_frame(inputs, built, "response")
    response = entity_read.decode_response_body(frame["body"])
    raw_entries = list(response["entries"])
    entries = [entity_read.decode_response_entry(raw) for raw in raw_entries]
    testcase.assertEqual(len(entries), 3)
    stored_b = sum(len(entry["stored"]) for entry in entries)
    testcase.assertEqual(stored_b, built["stored_b"])
    testcase.assertEqual(stored_b, 935)
    lookup_l = entity_read.derived_lookup_l(inputs)
    testcase.assertEqual((int(inputs["context"]["root_bindings"]), lookup_l), (256, 10))
    actual_a = 1 + len(entries) * lookup_l + 2 * stored_b
    testcase.assertEqual(actual_a, 1901)
    testcase.assertEqual(built["count_k"], 3)
    epoch = bytes.fromhex(inputs["context"]["content_epoch"])
    for entry in entries:
        checked = entity_read.check_stored_object(entry["stored"], entry["kind"], entry["entity"], epoch)
        testcase.assertEqual(entry["object_id"], checked["object_id"])
    return inputs, case, built, frame, response, raw_entries, entries, stored_b, lookup_l, actual_a


def _b2_assemble_sig(testcase, inputs, case, template_built, entry_blobs, *, request_m, request_k, request_w, selected_overrides=None, response_work=None):
    local_inputs = copy.deepcopy(inputs)
    if selected_overrides:
        for key, value in selected_overrides.items():
            local_inputs["selected_limits"][key] = value
    selected = {key: int(local_inputs["selected_limits"][key]) for key in _B2_LIMIT_KEYS}
    entity_read.validate_selected_limits(selected)
    local_case = copy.deepcopy(case)
    local_case["request"] = dict(case["request"])
    local_case["request"]["max_objects"] = request_k
    local_case["request"]["max_response_bytes"] = request_m
    local_case["request"]["max_work"] = request_w
    local_inputs["cases"][local_case["id"]] = copy.deepcopy(local_case)
    context = local_inputs["context"]
    root = bytes.fromhex(context["root"])
    entity_id = bytes.fromhex(local_inputs["entities"][local_case["entity"]]["id"])
    request_body = entity_read.build_request_body(root, entity_id, request_k, request_m, request_w)
    decoded_request = entity_read.decode_request_body(request_body, {"limits": selected})
    testcase.assertEqual((decoded_request["root"], decoded_request["entity"]), (root, entity_id))
    testcase.assertEqual(
        (decoded_request["max_objects"], decoded_request["ceiling_m"], decoded_request["max_work"]),
        (request_k, request_m, request_w),
    )
    testcase.assertLessEqual(len(request_body), request_m)
    decoded_entries = [entity_read.decode_response_entry(raw) for raw in entry_blobs]
    actual_k = len(entry_blobs)
    actual_b = sum(len(entry["stored"]) for entry in decoded_entries)
    lookup_l = entity_read.derived_lookup_l(local_inputs)
    actual_a = 1 + actual_k * lookup_l + 2 * actual_b
    work = actual_a + request_m if response_work is None else response_work
    testcase.assertLessEqual(work, entity_read.U64_MAX)
    response_body = entity_read.build_response_body(
        bytes.fromhex(context["workspace"]),
        root,
        bytes.fromhex(context["content_epoch"]),
        bytes.fromhex(context["session"]),
        entity_id,
        list(entry_blobs),
        work,
    )
    response_bounds = entity_read.build_bounds(selected, len(response_body), actual_k)
    session = bytes.fromhex(context["session"])
    request_id = int(local_case["request"]["request_id"])
    method = int(local_case["method"])
    request_payload = entity_read.build_frame_payload(2, session, request_id, 1, method, 0, entity_read.zero_bounds(), request_body)
    response_payload = entity_read.build_frame_payload(2, session, request_id, 2, method, 0, response_bounds, response_body)
    protocol_epoch = entity_read.protocol_epoch_id()
    request_wire, request_preimage, request_frame_id = entity_read.build_envelope(protocol_epoch, request_payload)
    response_wire, response_preimage, response_frame_id = entity_read.build_envelope(protocol_epoch, response_payload)
    testcase.assertEqual(int.from_bytes(response_wire[:8], "big"), len(response_wire) - 8)
    supplied = {
        "request_body_hex": request_body.hex(),
        "response_body_hex": response_body.hex(),
        "request_wire_hex": request_wire.hex(),
        "request_preimage_hex": request_preimage.hex(),
        "request_frame_id": request_frame_id.hex(),
        "response_wire_hex": response_wire.hex(),
        "response_preimage_hex": response_preimage.hex(),
        "response_frame_id": response_frame_id.hex(),
        "response_wire_len": len(response_wire),
        "work": work,
        "count_k": actual_k,
        "stored_b": actual_b,
        "objects": copy.deepcopy(template_built["objects"]),
    }
    return local_inputs, local_case, supplied, request_body, response_body, response_wire


def _b2_hello_frame_row(testcase, scoped, accepted, *, wire_version, expected_version, expect, expected_layer, expected_code, mutate_flags=None, mutate_body_empty=False):
    offer = scoped["hellos"]["hello_v2_client"]
    hello_body = entity_read.build_hello(offer)
    body_fields = entity_read.parse_record(hello_body)
    testcase.assertEqual(sorted(tag for tag, _ in body_fields), [1, 2, 3, 4, 5, 6, 7])
    flags = 0 if mutate_flags is None else mutate_flags
    body = hello_body if not mutate_body_empty else entity_read.encode_fields([])
    if mutate_body_empty:
        testcase.assertNotEqual(body, hello_body)
    payload = entity_read.build_frame_payload(wire_version, None, 0, 4, 0, flags, entity_read.zero_bounds(), body)
    wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
    stored, _prefix = entity_read.split_wire(wire, int(scoped["selected_limits"]["max_frame_bytes"]))
    checked_payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
    frame = entity_read.decode_frame_payload(checked_payload)
    testcase.assertEqual(
        (frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"], frame["flags"]),
        (wire_version, None, 0, 4, 0, flags),
    )
    testcase.assertEqual(frame["body"], body)
    row_id = f"hello_local_wire{wire_version}_expected{expected_version}"
    authored_row = {
        "hello": "hello_v2_client",
        "wire_version": wire_version,
        "expected_version": expected_version,
        "expect": expect,
        "expected_layer": expected_layer,
        "expected_code": expected_code,
    }
    supplied_row = {
        **authored_row,
        "body_hex": body.hex(),
        "wire_hex": wire.hex(),
        "preimage_hex": preimage.hex(),
        "frame_id": frame_id.hex(),
    }
    local_inputs = copy.deepcopy(scoped)
    local_inputs["frame_scenarios"] = {row_id: copy.deepcopy(authored_row)}
    local_accepted = copy.deepcopy(accepted)
    local_accepted["frame_scenarios"] = {row_id: copy.deepcopy(supplied_row)}
    return row_id, local_inputs, local_accepted


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
        mechanics_rows = (
            ("correct", ["ver_ws:semantic:request:frame_bounds:SCB_FIELD_MISSING"], "request", "frame_bounds", "SCB_FIELD_MISSING", True),
            ("correct-detail", ["ver_ws:semantic:request:frame_bounds:SCB_FIELD_MISSING: detail"], "request", "frame_bounds", "SCB_FIELD_MISSING", True),
            ("wrong-side", ["ver_ws:semantic:response:frame_bounds:SCB_FIELD_MISSING"], "request", "frame_bounds", "SCB_FIELD_MISSING", False),
            ("layer-suffix", ["ver_ws:semantic:request:frame_bounds_extra:SCB_FIELD_MISSING"], "request", "frame_bounds", "SCB_FIELD_MISSING", False),
            ("split", ["ver_ws:semantic:request:frame_bounds", "SCB_FIELD_MISSING"], "request", "frame_bounds", "SCB_FIELD_MISSING", False),
            ("prefixed", ["xxver_ws:semantic:request:frame_bounds:SCB_FIELD_MISSING"], "request", "frame_bounds", "SCB_FIELD_MISSING", False),
            ("nonstring", [None], "request", "frame_bounds", "SCB_FIELD_MISSING", False),
            ("mixed-valid-nonstring", ["ver_ws:semantic:request:frame_bounds:SCB_FIELD_MISSING", None], "request", "frame_bounds", "SCB_FIELD_MISSING", False),
            ("none-correct", ["ver_ws:semantic:request:wire_prefix: mismatch"], "request", "wire_prefix", None, True),
            ("none-wrong-side", ["ver_ws:semantic:response:wire_prefix: mismatch"], "request", "wire_prefix", None, False),
        )
        for name, probs, mside, mlayer, mcode, want in mechanics_rows:
            with self.subTest(mechanics=name):
                self.assertEqual(_b2_matches(probs, "ver_ws", mside, mlayer, mcode), want)

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
            inner_rows = (
                ("inner-missing-field", [(t, p) for t, p in limit_fields if t != 1], "SCB_FIELD_MISSING"),
                ("inner-extra-field", limit_fields + [(9, b"")], "SCB_FIELD_UNKNOWN"),
            )
            for name, mutated_limits, code in inner_rows:
                with self.subTest(side=side, variant=name):
                    mutated_bounds = entity_read.encode_fields([(t, (entity_read.encode_fields(mutated_limits) if t == 1 else p)) for t, p in bound_fields])
                    mutated_payload = entity_read.build_frame_payload(
                        frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"], frame["flags"], mutated_bounds, frame["body"]
                    )
                    supplied = _b2_supplied_with_payload(built, side, mutated_payload)
                    _b2_expect(self, inputs, case, supplied, side, "frame_bounds", code)
            with self.subTest(side=side, variant="inner-trailing"):
                trailing_inner = entity_read.encode_fields(limit_fields) + b"\x00"
                mutated_bounds = entity_read.encode_fields([(t, (trailing_inner if t == 1 else p)) for t, p in bound_fields])
                mutated_payload = entity_read.build_frame_payload(
                    frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"], frame["flags"], mutated_bounds, frame["body"]
                )
                supplied = _b2_supplied_with_payload(built, side, mutated_payload)
                _b2_expect(self, inputs, case, supplied, side, "frame_bounds", "SCB_TRAILING_BYTES")
            for tag, _payload in sorted(limit_fields):
                with self.subTest(side=side, variant=f"limit-field-{tag}-overflow"):
                    overflow = (1 << 32) if tag in _B2_U32_LIMIT_TAGS else (1 << 64)
                    mutated_limits = [(t, (encode_uvar(overflow) if t == tag else p)) for t, p in limit_fields]
                    mutated_bounds = entity_read.encode_fields([(t, (entity_read.encode_fields(mutated_limits) if t == 1 else p)) for t, p in bound_fields])
                    mutated_payload = entity_read.build_frame_payload(
                        frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"], frame["flags"], mutated_bounds, frame["body"]
                    )
                    supplied = _b2_supplied_with_payload(built, side, mutated_payload)
                    _b2_expect(self, inputs, case, supplied, side, "frame_bounds", "SCB_INTEGER_OVERFLOW")
            width_rows = (
                ("uvar-1-32-at-64", encode_uvar(1 << 32), 64, 1 << 32, None),
                ("uvar-1-32-at-32", encode_uvar(1 << 32), 32, None, "SCB_INTEGER_OVERFLOW"),
                ("uvar-1-64-at-64", encode_uvar(1 << 64), 64, None, "SCB_INTEGER_OVERFLOW"),
            )
            for name, raw, width, want, code in width_rows:
                with self.subTest(side=side, variant=name):
                    if want is not None:
                        self.assertEqual(entity_read.decode_uvar_exact(raw, width), want)
                    else:
                        with self.assertRaisesRegex(ScbError, code):
                            entity_read.decode_uvar_exact(raw, width)

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
                ("kind-4", {"kind": 4}, "PROTOCOL_FRAME_INVALID"),
                ("kind-4-version-1", {"version": 1, "kind": 4}, "PROTOCOL_DOWNGRADE"),
                ("kind-4-version-3", {"version": 3, "kind": 4}, "PROTOCOL_VERSION_UNSUPPORTED"),
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


    def test_supplied_frame_binding(self) -> None:
        for case_id in ("ver_ws", "sig_multi"):
            with self.subTest(case=case_id, variant="control"):
                inputs, case, built = _b2_load(case_id)
                problems = []
                entity_read.semantic_check(inputs, case, built, problems)
                self.assertEqual(problems, [])
                for side in ("request", "response"):
                    _wire, _stored, _payload, frame = _b2_valid_frame(inputs, built, side)
                    self.assertEqual(frame["session"], bytes.fromhex(inputs["context"]["session"]))
                    self.assertEqual(frame["request_id"], int(case["request"]["request_id"]))
                    self.assertEqual(frame["kind"], 1 if side == "request" else 2)
                    self.assertEqual(frame["method"], int(case["method"]))
                    self.assertEqual(frame["flags"], 0)
        inputs, case, built = _b2_load("ver_ws")
        _wire, _stored, _payload, req_frame = _b2_valid_frame(inputs, built, "request")
        _wire, _stored, _payload, resp_frame = _b2_valid_frame(inputs, built, "response")
        wrong_session = bytes([req_frame["session"][0] ^ 0x01]) + req_frame["session"][1:]
        self.assertNotEqual(wrong_session, req_frame["session"])
        other_method = 306 if int(case["method"]) == 307 else 307
        frames = {"request": req_frame, "response": resp_frame}
        rows = []
        for side in ("request", "response"):
            frame = frames[side]
            rows.append((side, "wrong-session", "session", {"session": wrong_session}))
            rows.append((side, "absent-session", "session", {"session": None}))
            rows.append((side, "request-id-plus-one", "request_id", {"request_id": frame["request_id"] + 1}))
            rows.append((side, "opposite-kind", "kind", {"kind": 2 if side == "request" else 1}))
            rows.append((side, "other-method", "method", {"method": other_method}))
            rows.append((side, "flag-1", "flags", {"flags": 1}))
            rows.append((side, "flag-2", "flags", {"flags": 2}))
        rows.append(("response", "flag-4-forbidden-success", "flags", {"flags": 4}))
        for side, name, field, overrides in rows:
            with self.subTest(side=side, variant=name):
                frame = frames[side]
                mutated_payload = entity_read.build_frame_payload(
                    frame["version"],
                    overrides["session"] if "session" in overrides else frame["session"],
                    overrides.get("request_id", frame["request_id"]),
                    overrides.get("kind", frame["kind"]),
                    overrides.get("method", frame["method"]),
                    overrides.get("flags", frame["flags"]),
                    frame["bounds"],
                    frame["body"],
                )
                mutated = entity_read.decode_frame_payload(mutated_payload)
                if field == "session":
                    self.assertEqual(mutated["session"], overrides["session"])
                else:
                    self.assertEqual(mutated[field], overrides[field])
                supplied = _b2_supplied_with_payload(built, side, mutated_payload)
                self.assertEqual(supplied[f"{side}_body_hex"], built[f"{side}_body_hex"])
                _b2_expect(self, inputs, case, supplied, side, "frame_binding", field)
        s_inputs, s_case, s_built = _b2_load("sig_multi")
        s_other = 306 if int(s_case["method"]) == 307 else 307
        for side in ("request", "response"):
            with self.subTest(side=side, variant="other-method-sig"):
                _wire, _stored, _payload, frame = _b2_valid_frame(s_inputs, s_built, side)
                mutated_payload = entity_read.build_frame_payload(
                    frame["version"], frame["session"], frame["request_id"], frame["kind"], s_other,
                    frame["flags"], frame["bounds"], frame["body"],
                )
                self.assertEqual(entity_read.decode_frame_payload(mutated_payload)["method"], s_other)
                supplied = _b2_supplied_with_payload(s_built, side, mutated_payload)
                _b2_expect(self, s_inputs, s_case, supplied, side, "frame_binding", "method")

    def test_request_actual_bounds_are_zero(self) -> None:
        inputs, case, built = _b2_load("ver_ws")
        problems = []
        entity_read.semantic_check(inputs, case, built, problems)
        self.assertEqual(problems, [])
        _wire, _stored, _payload, frame = _b2_valid_frame(inputs, built, "request")
        _b2_check_request_zero_limits(self, inputs, frame)
        bounds = entity_read.decode_bounds(frame["bounds"])
        self.assertEqual((bounds["returned_bytes"], bounds["returned_entities"]), (0, 0))
        self.assertFalse(bounds["truncated"])
        self.assertFalse(bounds["continuation"])
        bound_fields = entity_read.parse_record(frame["bounds"])
        limit_fields = entity_read.parse_record(entity_read.single_field(bound_fields, 1))
        rows = []
        for tag, _lp in sorted(limit_fields):
            rows.append((f"applied-limit-{tag}", "limit", tag, encode_uvar(1), _B2_LIMIT_KEYS[tag - 1]))
        counter_names = {2: "returned_bytes", 3: "returned_entities", 4: "returned_edges", 5: "reached_depth", 6: "omitted"}
        for tag, name in sorted(counter_names.items()):
            rows.append((f"counter-{name}", "bound", tag, encode_uvar(1), name))
        rows.append(("flag-truncated-true", "bound", 7, encode_uvar(2), "truncated"))
        rows.append(("flag-continuation-true", "bound", 8, encode_uvar(2), "continuation"))
        for name, kind, tag, raw, field in rows:
            with self.subTest(variant=name):
                if kind == "limit":
                    mutated_limits = [(t, raw if t == tag else p) for t, p in limit_fields]
                    mutated_bounds = entity_read.encode_fields(
                        [(t, entity_read.encode_fields(mutated_limits) if t == 1 else p) for t, p in bound_fields]
                    )
                    check = entity_read.parse_record(entity_read.single_field(entity_read.parse_record(mutated_bounds), 1))
                    width = 32 if tag in _B2_U32_LIMIT_TAGS else 64
                    self.assertEqual(entity_read.decode_uvar_exact(dict(check)[tag], width), 1)
                else:
                    mutated_bounds = entity_read.encode_fields([(t, raw if t == tag else p) for t, p in bound_fields])
                mutated_payload = entity_read.build_frame_payload(
                    frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"],
                    frame["flags"], mutated_bounds, frame["body"],
                )
                supplied = _b2_supplied_with_payload(built, "request", mutated_payload)
                _b2_expect(self, inputs, case, supplied, "request", "request_bounds", field)

    def test_response_actual_applied_limits_equal_selection(self) -> None:
        inputs, case, built = _b2_load("ver_ws")
        problems = []
        entity_read.semantic_check(inputs, case, built, problems)
        self.assertEqual(problems, [])
        _wire, _stored, _payload, frame = _b2_valid_frame(inputs, built, "response")
        _b2_check_response_selected_limits(self, inputs, frame)
        bound_fields = entity_read.parse_record(frame["bounds"])
        limit_fields = entity_read.parse_record(entity_read.single_field(bound_fields, 1))
        ceilings = entity_read.LIMIT_CEILINGS
        for (tag, _lp), key, current, ceiling in zip(sorted(limit_fields), _B2_LIMIT_KEYS, [int(inputs["selected_limits"][k]) for k in _B2_LIMIT_KEYS], ceilings):
            with self.subTest(variant=f"applied-{key}"):
                profile = {k: (current - 1 if k == key else int(inputs["selected_limits"][k])) for k in _B2_LIMIT_KEYS}
                self.assertGreater(current - 1, 0)
                self.assertLessEqual(current - 1, ceiling)
                entity_read.validate_selected_limits(profile)
                mutated_limits = [(t, encode_uvar(current - 1) if t == tag else p) for t, p in limit_fields]
                mutated_bounds = entity_read.encode_fields(
                    [(t, entity_read.encode_fields(mutated_limits) if t == 1 else p) for t, p in bound_fields]
                )
                check = entity_read.parse_record(entity_read.single_field(entity_read.parse_record(mutated_bounds), 1))
                width = 32 if tag in _B2_U32_LIMIT_TAGS else 64
                self.assertEqual(entity_read.decode_uvar_exact(dict(check)[tag], width), current - 1)
                mutated_payload = entity_read.build_frame_payload(
                    frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"],
                    frame["flags"], mutated_bounds, frame["body"],
                )
                supplied = _b2_supplied_with_payload(built, "response", mutated_payload)
                _b2_expect(self, inputs, case, supplied, "response", "applied_limits", key)
        with self.subTest(variant="applied-all-zero"):
            zero = entity_read.build_limits({key: 0 for key in _B2_LIMIT_KEYS})
            mutated_bounds = entity_read.encode_fields([(t, zero if t == 1 else p) for t, p in bound_fields])
            mutated_payload = entity_read.build_frame_payload(
                frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"],
                frame["flags"], mutated_bounds, frame["body"],
            )
            supplied = _b2_supplied_with_payload(built, "response", mutated_payload)
            _b2_expect(self, inputs, case, supplied, "response", "applied_limits", None)

    def test_actual_request_body_binding_and_range(self) -> None:
        inputs, case, built = _b2_load("ver_ws")
        problems = []
        entity_read.semantic_check(inputs, case, built, problems)
        self.assertEqual(problems, [])
        _wire, _stored, _payload, frame = _b2_valid_frame(inputs, built, "request")
        selected = {"limits": inputs["selected_limits"]}
        decoded = entity_read.decode_request_body(frame["body"], selected)
        root = bytes.fromhex(inputs["context"]["root"])
        entity = bytes.fromhex(inputs["entities"][case["entity"]]["id"])
        self.assertEqual(decoded["root"], root)
        self.assertEqual(decoded["entity"], entity)
        mo = int(case["request"]["max_objects"])
        mm = int(case["request"]["max_response_bytes"])
        mw = int(case["request"]["max_work"])
        self.assertEqual((decoded["max_objects"], decoded["ceiling_m"], decoded["max_work"]), (mo, mm, mw))
        self.assertLessEqual(mo, int(inputs["selected_limits"]["max_entities"]))
        self.assertLessEqual(mm, int(inputs["selected_limits"]["max_response_bytes"]))
        self.assertLessEqual(mw, int(inputs["selected_limits"]["max_work"]))
        alt_root = root[:-1] + bytes([root[-1] ^ 0x01])
        alt_entity = entity[:-1] + bytes([entity[-1] ^ 0x01])
        self.assertNotEqual(alt_root, root)
        self.assertNotEqual(alt_entity, entity)

        def _raw_request(r, e, a, m, w):
            return entity_read.encode_fields([(1, r), (2, e), (3, encode_uvar(a)), (4, encode_uvar(m)), (5, encode_uvar(w))])

        binding_rows = (
            ("root", _raw_request(alt_root, entity, mo, mm, mw), "root"),
            ("entity", _raw_request(root, alt_entity, mo, mm, mw), "entity"),
            ("max-objects", _raw_request(root, entity, mo + 1, mm, mw), "max_objects"),
            ("max-response-bytes", _raw_request(root, entity, mo, mm + 1, mw), "max_response_bytes"),
            ("max-work", _raw_request(root, entity, mo, mm, mw + 1), "max_work"),
        )
        self.assertLessEqual(mo + 1, int(inputs["selected_limits"]["max_entities"]))
        self.assertLessEqual(mm + 1, int(inputs["selected_limits"]["max_response_bytes"]))
        self.assertLessEqual(mw + 1, int(inputs["selected_limits"]["max_work"]))
        for name, mutated_body, field in binding_rows:
            with self.subTest(variant=f"binding-{name}"):
                check = entity_read.decode_request_body(mutated_body, selected)
                self.assertNotEqual(
                    (check["root"], check["entity"], check["max_objects"], check["ceiling_m"], check["max_work"]),
                    (root, entity, mo, mm, mw),
                )
                supplied = _b2_supplied_with_request_body(inputs, built, frame, mutated_body)
                self.assertEqual(_b2_valid_frame(inputs, supplied, "request")[3]["body"], mutated_body)
                _b2_expect(self, inputs, case, supplied, "request", "request_binding", field)
        range_rows = (
            ("zero-max-objects", _raw_request(root, entity, 0, mm, mw), "max_objects"),
            ("zero-max-response-bytes", _raw_request(root, entity, mo, 0, mw), "max_response_bytes"),
            ("zero-max-work", _raw_request(root, entity, mo, mm, 0), "max_work"),
            ("above-max-objects", _raw_request(root, entity, int(inputs["selected_limits"]["max_entities"]) + 1, mm, mw), "max_objects"),
            ("above-max-response-bytes", _raw_request(root, entity, mo, int(inputs["selected_limits"]["max_response_bytes"]) + 1, mw), "max_response_bytes"),
            ("above-max-work", _raw_request(root, entity, mo, mm, int(inputs["selected_limits"]["max_work"]) + 1), "max_work"),
        )
        for name, mutated_body, field in range_rows:
            with self.subTest(variant=f"range-{name}"):
                with self.assertRaisesRegex(entity_read.CheckFailed, "request_range"):
                    entity_read.decode_request_body(mutated_body, selected)
                supplied = _b2_supplied_with_request_body(inputs, built, frame, mutated_body)
                self.assertEqual(_b2_valid_frame(inputs, supplied, "request")[3]["body"], mutated_body)
                _b2_expect(self, inputs, case, supplied, "request", "request_range", None)
        base_fields = entity_read.parse_record(frame["body"])
        record_rows = (
            ("root-31", [(t, root[:31] if t == 1 else p) for t, p in base_fields], "SCB_LENGTH_OVERFLOW"),
            ("unknown-field", base_fields + [(9, b"")], "SCB_FIELD_UNKNOWN"),
            ("duplicate-field", base_fields[:2] + [base_fields[1]] + base_fields[2:], "SCB_FIELD_DUPLICATE"),
        )
        for name, mutated_fields, code in record_rows:
            with self.subTest(variant=f"record-{name}"):
                mutated_body = entity_read.encode_fields(mutated_fields)
                with self.assertRaisesRegex(ScbError, code):
                    entity_read.decode_request_body(mutated_body, selected)
                supplied = _b2_supplied_with_request_body(inputs, built, frame, mutated_body)
                self.assertEqual(_b2_valid_frame(inputs, supplied, "request")[3]["body"], mutated_body)
                _b2_expect(self, inputs, case, supplied, "request", "request_record", code)

    def test_actual_response_body_drives_semantics(self) -> None:
        inputs, case, built = _b2_load("ver_ws")
        problems = []
        entity_read.semantic_check(inputs, case, built, problems)
        self.assertEqual(problems, [])
        _wire, _stored, _payload, frame = _b2_valid_frame(inputs, built, "response")
        response = entity_read.decode_response_body(frame["body"])
        bounds = entity_read.decode_bounds(frame["bounds"])
        self.assertEqual(bounds["returned_entities"], built["count_k"])
        self.assertEqual(bounds["returned_bytes"], len(frame["body"]))
        self.assertEqual(response["work"], built["work"])
        requested = bytes.fromhex(inputs["entities"][case["entity"]]["id"])
        alt_requested = requested[:-1] + bytes([requested[-1] ^ 0x01])
        self.assertNotEqual(alt_requested, requested)
        authored_root = bytes.fromhex(inputs["context"]["root"])
        alt_root = authored_root[:-1] + bytes([authored_root[-1] ^ 0x01])
        field_rows = (
            ("workspace", 2, bytes([0xAA]) * 32),
            ("root", 3, alt_root),
            ("epoch", 4, bytes([0xE5]) * 32),
            ("session", 5, bytes([0x5A]) * 32),
            ("requested", 6, alt_requested),
        )
        for name, tag, intended in field_rows:
            with self.subTest(variant=f"context-{name}"):
                mutated_body = entity_read.apply_record_surgery(
                    frame["body"], {"op": "set_field_payload", "tag": tag, "payload_hex": intended.hex()}
                )
                self.assertEqual(entity_read.single_field(entity_read.parse_record(mutated_body), tag), intended)
                supplied = _b2_supplied_with_response_body(inputs, built, frame, mutated_body)
                self.assertEqual(_b2_valid_frame(inputs, supplied, "response")[3]["body"], mutated_body)
                _b2_expect(self, inputs, case, supplied, "response", "context", None)
        with self.subTest(variant="response-version-1"):
            mutated_body = entity_read.apply_record_surgery(frame["body"], {"op": "set_field_uvar", "tag": 1, "value": 1})
            with self.assertRaisesRegex(entity_read.CheckFailed, "response_record"):
                entity_read.decode_response_body(mutated_body)
            supplied = _b2_supplied_with_response_body(inputs, built, frame, mutated_body)
            self.assertEqual(_b2_valid_frame(inputs, supplied, "response")[3]["body"], mutated_body)
            _b2_expect(self, inputs, case, supplied, "response", "response_record", None)
        with self.subTest(variant="entry-object-id"):
            mutated_body = entity_read.mutate_entry_in_body(
                inputs, frame["body"], {"op": "flip_entry_field_byte", "index": 0, "field": 3, "offset": 31}
            )
            mutated_entries = entity_read.decode_response_body(mutated_body)["entries"]
            mutated_entry = entity_read.decode_response_entry(mutated_entries[0])
            self.assertNotEqual(mutated_entry["object_id"], mutated_entry["stored"][-32:])
            supplied = _b2_supplied_with_response_body(inputs, built, frame, mutated_body)
            self.assertEqual(_b2_valid_frame(inputs, supplied, "response")[3]["body"], mutated_body)
            _b2_expect(self, inputs, case, supplied, "response", "entry_binding", None)
        with self.subTest(variant="stored-entity-id"):
            entry = entity_read.decode_response_entry(response["entries"][0])
            alt_stored_entity = entry["entity"][:-1] + bytes([entry["entity"][-1] ^ 0x01])
            new_stored = resplice_stored(
                entry["stored"], lambda fields: [(t, alt_stored_entity if t == 1 else p) for t, p in fields]
            )
            new_object_id = new_stored[-32:]
            self.assertNotEqual(new_object_id, entry["object_id"])
            content_epoch = bytes.fromhex(inputs["context"]["content_epoch"])
            unbound = entity_read.decode_stored_unbound(new_stored, content_epoch)
            self.assertEqual(unbound["entity_id"], alt_stored_entity)
            self.assertEqual(unbound["object_id"], new_object_id)
            new_entry = entity_read.build_entry(entry["entity"], entry["kind"], new_object_id, new_stored)
            body_fields = entity_read.parse_record(frame["body"])
            items_raw = entity_read.single_field(body_fields, 7)
            reader = entity_read.Reader(items_raw)
            count = reader.uvar(64)
            entries = [reader.sized(entity_read.MAX_STANDALONE_BYTES) for _ in range(count)]
            reader.finish()
            entries[0] = new_entry
            items = encode_uvar(count) + b"".join(encode_sized(item) for item in entries)
            mutated_body = entity_read.encode_fields([(t, items if t == 7 else p) for t, p in body_fields])
            check_entry = entity_read.decode_response_entry(entity_read.decode_response_body(mutated_body)["entries"][0])
            self.assertEqual(check_entry["entity"], entry["entity"])
            self.assertNotEqual(check_entry["entity"], alt_stored_entity)
            supplied = _b2_supplied_with_response_body(inputs, built, frame, mutated_body)
            self.assertEqual(_b2_valid_frame(inputs, supplied, "response")[3]["body"], mutated_body)
            _b2_expect(self, inputs, case, supplied, "response", "entry_binding", None)
        with self.subTest(variant="stored-digest"):
            entry = entity_read.decode_response_entry(response["entries"][0])
            stored = entry["stored"]
            preimage, trailer = stored[:-32], stored[-32:]
            self.assertEqual(blake3.blake3(entity_read.OBJECT_DOMAIN + preimage).digest(), trailer)
            bad_trailer = trailer[:-1] + bytes([trailer[-1] ^ 0x01])
            self.assertNotEqual(bad_trailer, trailer)
            bad_stored = preimage + bad_trailer
            self.assertEqual(bad_stored[:-32], preimage)
            self.assertEqual(bad_stored[:-32], stored[:-32])
            self.assertEqual(len(bad_stored), len(stored))
            self.assertNotEqual(blake3.blake3(entity_read.OBJECT_DOMAIN + preimage).digest(), bad_trailer)
            content_epoch = bytes.fromhex(inputs["context"]["content_epoch"])
            with self.assertRaisesRegex(ScbError, "SCB_DIGEST_MISMATCH"):
                entity_read.check_stored_object(bad_stored, entry["kind"], entry["entity"], content_epoch)
            new_entry = entity_read.build_entry(entry["entity"], entry["kind"], bad_trailer, bad_stored)
            check_entry = entity_read.decode_response_entry(new_entry)
            self.assertEqual(check_entry["object_id"], bad_trailer)
            self.assertEqual(check_entry["stored"][-32:], bad_trailer)
            body_fields = entity_read.parse_record(frame["body"])
            items_raw = entity_read.single_field(body_fields, 7)
            reader = entity_read.Reader(items_raw)
            count = reader.uvar(64)
            entries = [reader.sized(entity_read.MAX_STANDALONE_BYTES) for _ in range(count)]
            reader.finish()
            self.assertEqual(count, built["count_k"])
            entries[0] = new_entry
            items = encode_uvar(count) + b"".join(encode_sized(item) for item in entries)
            mutated_body = entity_read.encode_fields([(t, items if t == 7 else p) for t, p in body_fields])
            self.assertEqual(len(entity_read.decode_response_body(mutated_body)["entries"]), built["count_k"])
            self.assertEqual(len(bad_stored), built["stored_b"])
            self.assertEqual(len(mutated_body), len(frame["body"]))
            supplied = _b2_supplied_with_response_body(inputs, built, frame, mutated_body)
            self.assertEqual(supplied["work"], built["work"])
            self.assertEqual(supplied["response_body_hex"], mutated_body.hex())
            self.assertEqual(_b2_valid_frame(inputs, supplied, "response")[3]["body"], mutated_body)
            _b2_expect(self, inputs, case, supplied, "response", "object_envelope", "SCB_DIGEST_MISMATCH")
        with self.subTest(variant="claimed-work"):
            mutated_body = entity_read.apply_record_surgery(frame["body"], {"op": "shift_uvar_field", "tag": 8, "delta": -1})
            body_fields = entity_read.parse_record(mutated_body)
            self.assertEqual(entity_read.decode_uvar_exact(entity_read.single_field(body_fields, 8), 64), built["work"] - 1)
            supplied = _b2_supplied_with_response_body(inputs, built, frame, mutated_body)
            self.assertEqual(supplied["work"], built["work"] - 1)
            self.assertEqual(_b2_valid_frame(inputs, supplied, "response")[3]["body"], mutated_body)
            _b2_expect(self, inputs, case, supplied, "response", "work", None)
        with self.subTest(variant="actual-before-detached-components"):
            mutated_body = entity_read.apply_record_surgery(
                frame["body"], {"op": "set_field_payload", "tag": 2, "payload_hex": (bytes([0xAA]) * 32).hex()}
            )
            supplied = _b2_supplied_with_response_body(inputs, built, frame, mutated_body, update_body=False)
            self.assertEqual(supplied["response_body_hex"], built["response_body_hex"])
            self.assertEqual(_b2_valid_frame(inputs, supplied, "response")[3]["body"], mutated_body)
            _b2_expect(self, inputs, case, supplied, "response", "context", None)
        s_inputs, s_case, s_built = _b2_load("sig_multi")
        problems = []
        entity_read.semantic_check(s_inputs, s_case, s_built, problems)
        self.assertEqual(problems, [])
        self.assertEqual(s_built["count_k"], 3)
        _wire, _stored, _payload, s_frame = _b2_valid_frame(s_inputs, s_built, "response")
        for rejected_id in ("sig_owner", "sig_swapped"):
            with self.subTest(variant=rejected_id):
                authored = next(item for item in s_inputs["rejected"] if item["id"] == rejected_id)
                base = s_inputs["cases"][authored["base"]]
                mutated_wire = entity_read.build_rejected_bytes(s_inputs, base, authored["recipe"])
                max_frame = _b2_max_frame(s_inputs)
                stored, _prefix = entity_read.split_wire(mutated_wire, max_frame)
                payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
                mutated_frame = entity_read.decode_frame_payload(payload)
                mutated_body = mutated_frame["body"]
                mutated_response = entity_read.decode_response_body(mutated_body)
                mutated_entries = [entity_read.decode_response_entry(raw) for raw in mutated_response["entries"]]
                content_epoch = bytes.fromhex(s_inputs["context"]["content_epoch"])
                for entry in mutated_entries:
                    checked = entity_read.check_stored_object(entry["stored"], entry["kind"], entry["entity"], content_epoch)
                    self.assertEqual(entry["object_id"], checked["object_id"])
                requested = bytes.fromhex(s_inputs["entities"][base["entity"]]["id"])
                entity_read.check_context(mutated_response, s_inputs["context"], requested)
                actual_k = len(mutated_entries)
                actual_b = sum(len(entry["stored"]) for entry in mutated_entries)
                expected_work = entity_read.compute_work(
                    actual_k, entity_read.derived_lookup_l(s_inputs), actual_b, int(base["request"]["max_response_bytes"])
                )
                self.assertEqual(mutated_response["work"], expected_work)
                self.assertEqual(
                    mutated_frame["bounds"],
                    entity_read.build_bounds(s_inputs["selected_limits"], len(mutated_body), s_built["count_k"]),
                )
                self.assertEqual(entity_read.validate_rejected(s_inputs, authored, mutated_wire), ("signature", None))
                supplied = _b2_supplied_with_response_body(s_inputs, s_built, s_frame, mutated_body)
                _b2_expect(self, s_inputs, s_case, supplied, "response", "signature", None)


    def test_response_actual_counts_and_required_k(self) -> None:
        inputs, case, built, frame, response, raw_entries, entries, stored_b, lookup_l, actual_a = _b2_sig_base(self)
        self.assertEqual((len(entries), stored_b, lookup_l, actual_a), (3, 935, 10, 1901))
        self.assertEqual(built["work"], actual_a + int(case["request"]["max_response_bytes"]))
        bound_fields = entity_read.parse_record(frame["bounds"])
        actual_bytes = len(frame["body"])
        counter_rows = (
            ("returned_bytes", 2, encode_uvar(actual_bytes + 1)),
            ("returned_entities", 3, encode_uvar(len(entries) + 1)),
            ("returned_edges", 4, encode_uvar(1)),
            ("reached_depth", 5, encode_uvar(1)),
            ("omitted", 6, encode_uvar(1)),
            ("truncated", 7, encode_uvar(2)),
            ("continuation", 8, encode_uvar(2)),
        )
        for name, tag, raw in counter_rows:
            with self.subTest(variant=f"counter-{name}"):
                mutated_bounds = entity_read.encode_fields([(t, raw if t == tag else p) for t, p in bound_fields])
                mutated_payload = entity_read.build_frame_payload(
                    frame["version"],
                    frame["session"],
                    frame["request_id"],
                    frame["kind"],
                    frame["method"],
                    frame["flags"],
                    mutated_bounds,
                    frame["body"],
                )
                mutated_frame = entity_read.decode_frame_payload(mutated_payload)
                self.assertEqual(mutated_frame["body"], frame["body"])
                self.assertNotEqual(mutated_frame["bounds"], frame["bounds"])
                mutated_counts = entity_read.decode_bounds(mutated_frame["bounds"])
                if tag == 2:
                    self.assertEqual(mutated_counts["returned_bytes"], actual_bytes + 1)
                elif tag == 3:
                    self.assertEqual(mutated_counts["returned_entities"], len(entries) + 1)
                elif tag in (4, 5, 6):
                    key = {4: "returned_edges", 5: "reached_depth", 6: "omitted"}[tag]
                    self.assertEqual(mutated_counts[key], 1)
                else:
                    key = {7: "truncated", 8: "continuation"}[tag]
                    self.assertTrue(mutated_counts[key])
                supplied = _b2_supplied_with_payload(built, "response", mutated_payload)
                self.assertEqual(supplied["response_body_hex"], built["response_body_hex"])
                self.assertEqual(supplied["work"], built["work"])
                _b2_expect(self, inputs, case, supplied, "response", "count", None)
        with self.subTest(variant="required-k-missing-parameter"):
            self.assertEqual([ref for ref in case["objects"]], ["sig_f", "sig_p_high", "sig_p_low"])
            self.assertEqual(case["signature"]["parameters"], ["sig_p_high", "sig_p_low"])
            kept = [raw_entries[0], raw_entries[1]]
            kept_entries = [entity_read.decode_response_entry(raw) for raw in kept]
            epoch = bytes.fromhex(inputs["context"]["content_epoch"])
            for entry in kept_entries:
                checked = entity_read.check_stored_object(entry["stored"], entry["kind"], entry["entity"], epoch)
                self.assertEqual(entry["object_id"], checked["object_id"])
            kept_b = sum(len(entry["stored"]) for entry in kept_entries)
            self.assertEqual(kept_b, stored_b - len(entries[2]["stored"]))
            request_m = int(case["request"]["max_response_bytes"])
            kept_a = 1 + 2 * lookup_l + 2 * kept_b
            kept_work = kept_a + request_m
            local_inputs, local_case, supplied, _req_body, kept_body, _wire = _b2_assemble_sig(
                self,
                inputs,
                case,
                built,
                kept,
                request_m=request_m,
                request_k=int(case["request"]["max_objects"]),
                request_w=int(case["request"]["max_work"]),
            )
            self.assertEqual(supplied["count_k"], 2)
            self.assertEqual(supplied["stored_b"], kept_b)
            self.assertEqual(supplied["work"], kept_work)
            actual_count = len(entity_read.decode_response_body(bytes.fromhex(supplied["response_body_hex"]))["entries"])
            self.assertEqual(actual_count, 2)
            self.assertEqual(len(local_case["objects"]), 3)
            _b2_expect(self, local_inputs, local_case, supplied, "response", "count", None)
        with self.subTest(variant="wrong-owner-wrong-count"):
            authored = next(item for item in inputs["rejected"] if item["id"] == "sig_owner")
            base = inputs["cases"][authored["base"]]
            self.assertEqual(base["id"], "sig_multi")
            owner_wire = entity_read.build_rejected_bytes(inputs, base, authored["recipe"])
            max_frame = _b2_max_frame(inputs)
            owner_stored, _prefix = entity_read.split_wire(owner_wire, max_frame)
            owner_payload, _trailer = entity_read.check_envelope(owner_stored, entity_read.protocol_epoch_id())
            owner_frame = entity_read.decode_frame_payload(owner_payload)
            owner_response = entity_read.decode_response_body(owner_frame["body"])
            owner_entries = [entity_read.decode_response_entry(raw) for raw in owner_response["entries"]]
            owner_epoch = bytes.fromhex(inputs["context"]["content_epoch"])
            for entry in owner_entries:
                checked = entity_read.check_stored_object(entry["stored"], entry["kind"], entry["entity"], owner_epoch)
                self.assertEqual(entry["object_id"], checked["object_id"])
            requested = bytes.fromhex(inputs["entities"][base["entity"]]["id"])
            entity_read.check_context(owner_response, inputs["context"], requested)
            owner_k = len(owner_entries)
            owner_b = sum(len(entry["stored"]) for entry in owner_entries)
            owner_l = entity_read.derived_lookup_l(inputs)
            self.assertEqual(
                owner_response["work"], 1 + owner_k * owner_l + 2 * owner_b + int(base["request"]["max_response_bytes"])
            )
            owner_bounds = entity_read.decode_bounds(owner_frame["bounds"])
            self.assertEqual((owner_bounds["returned_entities"], owner_bounds["returned_bytes"]), (owner_k, len(owner_frame["body"])))
            self.assertEqual(entity_read.validate_rejected(inputs, authored, owner_wire), ("signature", None))
            owner_fields = entity_read.parse_record(owner_frame["bounds"])
            mutated_bounds = entity_read.encode_fields([(t, encode_uvar(owner_k + 1) if t == 3 else p) for t, p in owner_fields])
            mutated_payload = entity_read.build_frame_payload(
                owner_frame["version"],
                owner_frame["session"],
                owner_frame["request_id"],
                owner_frame["kind"],
                owner_frame["method"],
                owner_frame["flags"],
                mutated_bounds,
                owner_frame["body"],
            )
            mutated_frame = entity_read.decode_frame_payload(mutated_payload)
            self.assertEqual(mutated_frame["body"], owner_frame["body"])
            self.assertEqual(entity_read.decode_response_body(mutated_frame["body"])["work"], owner_response["work"])
            owner_wire2, owner_pre2, owner_fid2 = entity_read.build_envelope(entity_read.protocol_epoch_id(), mutated_payload)
            supplied = copy.deepcopy(built)
            supplied["response_body_hex"] = owner_frame["body"].hex()
            body_fields = entity_read.parse_record(owner_frame["body"])
            supplied["work"] = entity_read.decode_uvar_exact(entity_read.single_field(body_fields, 8), 64)
            supplied["response_wire_hex"] = owner_wire2.hex()
            supplied["response_preimage_hex"] = owner_pre2.hex()
            supplied["response_frame_id"] = owner_fid2.hex()
            supplied["response_wire_len"] = len(owner_wire2)
            supplied["count_k"] = owner_k
            supplied["stored_b"] = owner_b
            _b2_expect(self, inputs, case, supplied, "response", "count", None)

    def test_detached_components_match_actual_wire(self) -> None:
        inputs, case, built, frame, response, raw_entries, entries, stored_b, lookup_l, actual_a = _b2_sig_base(self)
        _wire, _stored, _payload, req_frame = _b2_valid_frame(inputs, built, "request")
        self.assertEqual((req_frame["version"], frame["version"]), (2, 2))
        hex_targets = (
            "request_body_hex",
            "request_preimage_hex",
            "request_frame_id",
            "response_body_hex",
            "response_preimage_hex",
            "response_frame_id",
        )
        for key in hex_targets:
            with self.subTest(variant=f"detached-{key}"):
                original = built[key]
                flipped = ("0" if original[0] != "0" else "1") + original[1:]
                self.assertNotEqual(flipped, original)
                supplied = copy.deepcopy(built)
                supplied[key] = flipped
                _b2_valid_frame(inputs, supplied, "request")
                _b2_valid_frame(inputs, supplied, "response")
                side = "request" if key.startswith("request_") else "response"
                _b2_expect(self, inputs, case, supplied, side, "component_binding", key)
        int_targets = (
            ("response_wire_len", built["response_wire_len"] + 1),
            ("count_k", built["count_k"] + 1),
            ("stored_b", built["stored_b"] + 1),
            ("work", built["work"] + 1),
        )
        for key, mutated in int_targets:
            with self.subTest(variant=f"detached-{key}"):
                supplied = copy.deepcopy(built)
                supplied[key] = mutated
                _b2_valid_frame(inputs, supplied, "request")
                _b2_valid_frame(inputs, supplied, "response")
                _b2_expect(self, inputs, case, supplied, "response", "component_binding", key)

    def test_actual_ingress_and_outgoing_sizes_are_distinct(self) -> None:
        inputs, case, built, frame, response, raw_entries, entries, stored_b, lookup_l, actual_a = _b2_sig_base(self)
        request_m = int(case["request"]["max_response_bytes"])
        request_k = int(case["request"]["max_objects"])
        request_w = int(case["request"]["max_work"])
        _seed_inputs, _seed_case, seed_supplied, _req_body, seed_body, _seed_wire = _b2_assemble_sig(
            self,
            inputs,
            case,
            built,
            raw_entries,
            request_m=request_m,
            request_k=request_k,
            request_w=request_w,
            selected_overrides={"max_frame_bytes": 4096},
        )
        seed_wire = bytes.fromhex(seed_supplied["response_wire_hex"])
        full_size = len(seed_wire)
        self.assertEqual(seed_body, bytes.fromhex(built["response_body_hex"]))
        self.assertEqual(seed_supplied["work"], built["work"])
        for cap in (4096, full_size, full_size - 1, full_size - 8, full_size - 9):
            self.assertEqual(_b2_uvar_len(cap), 2)
        self.assertLessEqual(128, full_size - 9)
        self.assertLess(full_size, 16384)
        self.assertEqual(full_size, 1559)
        self.assertEqual(int.from_bytes(seed_wire[:8], "big"), full_size - 8)
        self.assertEqual(int.from_bytes(seed_wire[:8], "big"), 1551)
        request_envelope_len = len(bytes.fromhex(seed_supplied["request_wire_hex"])) - 8
        self.assertLessEqual(request_envelope_len, full_size - 9)
        rows = (
            ("exact", full_size, "control"),
            ("one-below", full_size - 1, "outgoing_wire_ceiling"),
            ("ingress-exact", full_size - 8, "outgoing_wire_ceiling"),
            ("ingress-below", full_size - 9, "wire_ceiling"),
        )
        for name, cap, outcome in rows:
            with self.subTest(variant=name):
                local_inputs, local_case, supplied, _req, body, wire = _b2_assemble_sig(
                    self,
                    inputs,
                    case,
                    built,
                    raw_entries,
                    request_m=request_m,
                    request_k=request_k,
                    request_w=request_w,
                    selected_overrides={"max_frame_bytes": cap},
                )
                self.assertEqual(local_case["request"], case["request"])
                self.assertEqual(int(local_inputs["selected_limits"]["max_frame_bytes"]), cap)
                self.assertEqual(body, seed_body)
                self.assertEqual(len(wire), full_size)
                self.assertEqual(int.from_bytes(wire[:8], "big"), full_size - 8)
                self.assertLessEqual(len(body), request_m)
                self.assertLessEqual(request_m, int(local_inputs["selected_limits"]["max_response_bytes"]))
                self.assertLessEqual(len(entries), request_k)
                self.assertLessEqual(supplied["work"], request_w)
                self.assertLessEqual(request_w, int(local_inputs["selected_limits"]["max_work"]))
                response_stored = wire[8:]
                self.assertEqual(len(response_stored), len(wire) - 8)
                authenticated_payload, _trailer = entity_read.check_envelope(
                    response_stored, entity_read.protocol_epoch_id()
                )
                resp_frame = entity_read.decode_frame_payload(authenticated_payload)
                _b2_check_response_selected_limits(self, local_inputs, resp_frame)
                self.assertEqual(entity_read.decode_bounds(resp_frame["bounds"])["returned_bytes"], len(body))
                stored_len = len(wire) - 8
                if outcome == "control":
                    stored, _prefix = entity_read.split_wire(wire, cap)
                    self.assertEqual(len(stored), stored_len)
                    check_problems = []
                    entity_read.semantic_check(local_inputs, local_case, supplied, check_problems)
                    self.assertEqual(check_problems, [])
                elif outcome == "outgoing_wire_ceiling":
                    stored, _prefix = entity_read.split_wire(wire, cap)
                    self.assertEqual(len(stored), stored_len)
                    _b2_expect(self, local_inputs, local_case, supplied, "response", "outgoing_wire_ceiling", None)
                else:
                    with self.assertRaisesRegex(entity_read.CheckFailed, "wire_ceiling"):
                        entity_read.split_wire(wire, cap)
                    _b2_expect(self, local_inputs, local_case, supplied, "response", "wire_ceiling", None)

    def test_actual_response_resource_bounds(self) -> None:
        inputs, case, built, frame, response, raw_entries, entries, stored_b, lookup_l, actual_a = _b2_sig_base(self)
        ordinary_m = int(case["request"]["max_response_bytes"])
        ordinary_k = int(case["request"]["max_objects"])
        ordinary_w = int(case["request"]["max_work"])
        with self.subTest(variant="k-exact-control"):
            local_inputs, local_case, supplied, _req, body, _wire = _b2_assemble_sig(
                self, inputs, case, built, raw_entries, request_m=ordinary_m, request_k=3, request_w=ordinary_w
            )
            check_problems = []
            entity_read.semantic_check(local_inputs, local_case, supplied, check_problems)
            self.assertEqual(check_problems, [])
        with self.subTest(variant="k-exact-cobound-control"):
            local_inputs, local_case, supplied, _req, body, _wire = _b2_assemble_sig(
                self,
                inputs,
                case,
                built,
                raw_entries,
                request_m=ordinary_m,
                request_k=3,
                request_w=ordinary_w,
                selected_overrides={"max_entities": 3},
            )
            check_problems = []
            entity_read.semantic_check(local_inputs, local_case, supplied, check_problems)
            self.assertEqual(check_problems, [])
        for name, req_k, sel_entities in (("k-below-request", 2, None), ("k-cobound", 2, 2)):
            with self.subTest(variant=name):
                overrides = None if sel_entities is None else {"max_entities": sel_entities}
                local_inputs, local_case, supplied, req_body, body, _wire = _b2_assemble_sig(
                    self,
                    inputs,
                    case,
                    built,
                    raw_entries,
                    request_m=ordinary_m,
                    request_k=req_k,
                    request_w=ordinary_w,
                    selected_overrides=overrides,
                )
                admitted = entity_read.decode_request_body(req_body, {"limits": dict(local_inputs["selected_limits"])})
                self.assertEqual(admitted["max_objects"], req_k)
                _w, _s, _p, resp_frame = _b2_valid_frame(local_inputs, supplied, "response")
                self.assertEqual(entity_read.decode_bounds(resp_frame["bounds"])["returned_entities"], 3)
                _b2_expect(self, local_inputs, local_case, supplied, "response", "response_range", None)
        template_body = entity_read.build_response_body(
            bytes.fromhex(inputs["context"]["workspace"]),
            bytes.fromhex(inputs["context"]["root"]),
            bytes.fromhex(inputs["context"]["content_epoch"]),
            bytes.fromhex(inputs["context"]["session"]),
            bytes.fromhex(inputs["entities"][case["entity"]]["id"]),
            list(raw_entries),
            0,
        )
        template_d = len(template_body) - 1
        self.assertEqual(template_d, 1352)
        candidates = []
        for width in range(1, 11):
            cand_m = template_d + width
            cand_w = actual_a + cand_m
            if cand_m <= 0 or cand_w > entity_read.U64_MAX:
                continue
            if _b2_uvar_len(cand_w) != width:
                continue
            candidates.append((width, cand_m, cand_w))
        self.assertTrue(candidates)
        width, exact_m, exact_w = candidates[0]
        self.assertEqual((width, exact_m, exact_w), (2, 1354, 3255))
        self.assertEqual(_b2_uvar_len(exact_w - 1), width)
        with self.subTest(variant="m-exact-control"):
            local_inputs, local_case, supplied, req_body, body, _wire = _b2_assemble_sig(
                self, inputs, case, built, raw_entries, request_m=exact_m, request_k=ordinary_k, request_w=ordinary_w
            )
            self.assertEqual(len(body), exact_m)
            self.assertEqual(supplied["work"], exact_w)
            decoded_req = entity_read.decode_request_body(req_body, {"limits": dict(local_inputs["selected_limits"])})
            self.assertEqual(decoded_req["ceiling_m"], exact_m)
            decoded_resp = entity_read.decode_response_body(body)
            self.assertEqual((len(decoded_resp["entries"]), decoded_resp["work"]), (3, exact_w))
            _w, _s, _p, resp_frame = _b2_valid_frame(local_inputs, supplied, "response")
            _b2_check_response_selected_limits(self, local_inputs, resp_frame)
            self.assertEqual(entity_read.decode_bounds(resp_frame["bounds"])["returned_bytes"], exact_m)
            check_problems = []
            entity_read.semantic_check(local_inputs, local_case, supplied, check_problems)
            self.assertEqual(check_problems, [])
        with self.subTest(variant="m-one-below"):
            local_inputs, local_case, supplied, req_body, body, _wire = _b2_assemble_sig(
                self,
                inputs,
                case,
                built,
                raw_entries,
                request_m=exact_m - 1,
                request_k=ordinary_k,
                request_w=ordinary_w,
                response_work=exact_w - 1,
            )
            self.assertEqual(len(body), exact_m)
            self.assertEqual(supplied["work"], exact_w - 1)
            self.assertEqual(len(body), (exact_m - 1) + 1)
            admitted = entity_read.decode_request_body(req_body, {"limits": dict(local_inputs["selected_limits"])})
            self.assertEqual(admitted["ceiling_m"], exact_m - 1)
            _w, _s, _p, resp_frame = _b2_valid_frame(local_inputs, supplied, "response")
            self.assertEqual(entity_read.decode_bounds(resp_frame["bounds"])["returned_bytes"], exact_m)
            _b2_expect(self, local_inputs, local_case, supplied, "response", "response_range", None)
        with self.subTest(variant="m-cobound-control"):
            local_inputs, local_case, supplied, _req, body, _wire = _b2_assemble_sig(
                self,
                inputs,
                case,
                built,
                raw_entries,
                request_m=exact_m,
                request_k=ordinary_k,
                request_w=ordinary_w,
                selected_overrides={"max_response_bytes": exact_m},
            )
            self.assertEqual(len(body), exact_m)
            check_problems = []
            entity_read.semantic_check(local_inputs, local_case, supplied, check_problems)
            self.assertEqual(check_problems, [])
        with self.subTest(variant="m-cobound-one-below"):
            local_inputs, local_case, supplied, req_body, body, _wire = _b2_assemble_sig(
                self,
                inputs,
                case,
                built,
                raw_entries,
                request_m=exact_m - 1,
                request_k=ordinary_k,
                request_w=ordinary_w,
                selected_overrides={"max_response_bytes": exact_m - 1},
                response_work=exact_w - 1,
            )
            self.assertEqual(len(body), exact_m)
            admitted = entity_read.decode_request_body(req_body, {"limits": dict(local_inputs["selected_limits"])})
            self.assertEqual(admitted["ceiling_m"], exact_m - 1)
            _b2_expect(self, local_inputs, local_case, supplied, "response", "response_range", None)
        fixed_m = 200000
        fixed_w = actual_a + fixed_m
        self.assertEqual(_b2_uvar_len(fixed_w), 3)
        with self.subTest(variant="w-exact-control"):
            local_inputs, local_case, supplied, req_body, body, _wire = _b2_assemble_sig(
                self, inputs, case, built, raw_entries, request_m=fixed_m, request_k=ordinary_k, request_w=fixed_w
            )
            admitted = entity_read.decode_request_body(req_body, {"limits": dict(local_inputs["selected_limits"])})
            self.assertEqual((admitted["ceiling_m"], admitted["max_work"]), (fixed_m, fixed_w))
            self.assertLessEqual(len(body), fixed_m)
            check_problems = []
            entity_read.semantic_check(local_inputs, local_case, supplied, check_problems)
            self.assertEqual(check_problems, [])
        with self.subTest(variant="w-one-below-request"):
            local_inputs, local_case, supplied, req_body, body, _wire = _b2_assemble_sig(
                self, inputs, case, built, raw_entries, request_m=fixed_m, request_k=ordinary_k, request_w=fixed_w - 1
            )
            admitted = entity_read.decode_request_body(req_body, {"limits": dict(local_inputs["selected_limits"])})
            self.assertEqual(admitted["max_work"], fixed_w - 1)
            self.assertEqual(supplied["work"], fixed_w)
            _b2_expect(self, local_inputs, local_case, supplied, "response", "response_range", None)
        with self.subTest(variant="w-cobound-control"):
            self.assertGreater(fixed_w, 0)
            self.assertLessEqual(fixed_w, entity_read.LIMIT_CEILINGS[5])
            local_inputs, local_case, supplied, _req, body, _wire = _b2_assemble_sig(
                self,
                inputs,
                case,
                built,
                raw_entries,
                request_m=fixed_m,
                request_k=ordinary_k,
                request_w=fixed_w,
                selected_overrides={"max_work": fixed_w},
            )
            check_problems = []
            entity_read.semantic_check(local_inputs, local_case, supplied, check_problems)
            self.assertEqual(check_problems, [])
        with self.subTest(variant="w-cobound-one-below"):
            self.assertGreater(fixed_w - 1, 0)
            self.assertLessEqual(fixed_w - 1, entity_read.LIMIT_CEILINGS[5])
            local_inputs, local_case, supplied, req_body, body, _wire = _b2_assemble_sig(
                self,
                inputs,
                case,
                built,
                raw_entries,
                request_m=fixed_m,
                request_k=ordinary_k,
                request_w=fixed_w - 1,
                selected_overrides={"max_work": fixed_w - 1},
            )
            admitted = entity_read.decode_request_body(req_body, {"limits": dict(local_inputs["selected_limits"])})
            self.assertEqual(admitted["max_work"], fixed_w - 1)
            _b2_expect(self, local_inputs, local_case, supplied, "response", "response_range", None)

    def test_actual_layer_precedence_controls(self) -> None:
        inputs, case, built = _b2_load("sig_multi")
        check_problems = []
        entity_read.semantic_check(inputs, case, built, check_problems)
        self.assertEqual(check_problems, [])
        _wire, _stored, _payload, frame = _b2_valid_frame(inputs, built, "response")
        self.assertEqual((frame["version"], frame["kind"], frame["flags"]), (2, 2, 0))
        with self.subTest(variant="prefix-before-digest"):
            wire = bytes.fromhex(built["response_wire_hex"])
            preimage = bytes.fromhex(built["response_preimage_hex"])
            trailer = bytes.fromhex(built["response_frame_id"])
            self.assertEqual(int.from_bytes(wire[:8], "big"), len(preimage) + len(trailer))
            bad_trailer = trailer[:-1] + bytes([trailer[-1] ^ 0x01])
            self.assertNotEqual(blake3.blake3(entity_read.FRAME_DOMAIN + preimage).digest(), bad_trailer)
            bad_wire = wire[:7] + preimage + bad_trailer
            supplied = _b2_supplied_wire_only(built, "response", bad_wire)
            _b2_expect(self, inputs, case, supplied, "response", "wire_prefix", None)
        with self.subTest(variant="bounds-before-version"):
            bound_fields = entity_read.parse_record(frame["bounds"])
            mutated_bounds = entity_read.encode_fields([(t, p) for t, p in bound_fields if t != 1])
            mutated_payload = entity_read.build_frame_payload(
                1,
                frame["session"],
                frame["request_id"],
                frame["kind"],
                frame["method"],
                frame["flags"],
                mutated_bounds,
                frame["body"],
            )
            supplied = _b2_supplied_with_payload(built, "response", mutated_payload)
            _b2_expect(self, inputs, case, supplied, "response", "frame_bounds", "SCB_FIELD_MISSING")
        for name, version, code in (
            ("downgrade-before-flag", 1, "PROTOCOL_DOWNGRADE"),
            ("unsupported-before-flag", 3, "PROTOCOL_VERSION_UNSUPPORTED"),
        ):
            with self.subTest(variant=name):
                mutated_payload = entity_read.build_frame_payload(
                    version,
                    frame["session"],
                    frame["request_id"],
                    frame["kind"],
                    frame["method"],
                    8,
                    frame["bounds"],
                    frame["body"],
                )
                mutated_frame = entity_read.decode_frame_payload(mutated_payload)
                self.assertEqual((mutated_frame["version"], mutated_frame["flags"]), (version, 8))
                supplied = _b2_supplied_with_payload(built, "response", mutated_payload)
                _b2_expect(self, inputs, case, supplied, "response", "frame_header", code)
        with self.subTest(variant="hello-valid-control"):
            full_inputs = load_authored_inputs()
            accepted = _b1_build_accepted(full_inputs)
            control_problems = []
            entity_read.check_selection(full_inputs, accepted, control_problems)
            self.assertEqual(control_problems, [])
            row_id, local_inputs, local_accepted = _b2_hello_frame_row(
                self,
                full_inputs,
                accepted,
                wire_version=1,
                expected_version=1,
                expect="accepted",
                expected_layer=None,
                expected_code=None,
            )
            hello_problems = []
            entity_read.check_selection(local_inputs, local_accepted, hello_problems)
            self.assertIsInstance(hello_problems, list)
            for entry in hello_problems:
                self.assertIsInstance(entry, str)
            self.assertEqual(hello_problems, [])
            self.assertFalse(_b2_hello_matches(hello_problems, row_id, "frame_header", "PROTOCOL_FRAME_INVALID"))
        with self.subTest(variant="hello-header-before-body"):
            full_inputs = load_authored_inputs()
            accepted = _b1_build_accepted(full_inputs)
            control_problems = []
            entity_read.check_selection(full_inputs, accepted, control_problems)
            self.assertEqual(control_problems, [])
            row_id, local_inputs, local_accepted = _b2_hello_frame_row(
                self,
                full_inputs,
                accepted,
                wire_version=1,
                expected_version=1,
                expect="accepted",
                expected_layer=None,
                expected_code=None,
                mutate_flags=1,
                mutate_body_empty=True,
            )
            hello_problems = []
            entity_read.check_selection(local_inputs, local_accepted, hello_problems)
            _b2_hello_expect(self, hello_problems, row_id, "frame_header", "PROTOCOL_FRAME_INVALID")
        hello_mechanics = (
            ("correct", ["hello_frame:row1:frame_header:PROTOCOL_FRAME_INVALID"], "row1", "frame_header", "PROTOCOL_FRAME_INVALID", True),
            ("correct-detail", ["hello_frame:row1:frame_header:PROTOCOL_FRAME_INVALID: extra"], "row1", "frame_header", "PROTOCOL_FRAME_INVALID", True),
            ("wrong-row", ["hello_frame:row2:frame_header:PROTOCOL_FRAME_INVALID"], "row1", "frame_header", "PROTOCOL_FRAME_INVALID", False),
            ("wrong-layer", ["hello_frame:row1:frame_bounds:PROTOCOL_FRAME_INVALID"], "row1", "frame_header", "PROTOCOL_FRAME_INVALID", False),
            ("none-correct", ["hello_frame:row1:wire_prefix: mismatch"], "row1", "wire_prefix", None, True),
            ("nonstring", [None], "row1", "frame_header", "PROTOCOL_FRAME_INVALID", False),
        )
        for name, probs, mrow, mlayer, mcode, want in hello_mechanics:
            with self.subTest(mechanics=name):
                self.assertEqual(_b2_hello_matches(probs, mrow, mlayer, mcode), want)


_B2_T4_MATRIX = (
    ("hello_wire1_expected1", 1, 1, "accepted", None, None),
    ("hello_wire1_expected2", 1, 2, "rejected", "frame_header", "PROTOCOL_DOWNGRADE"),
    ("hello_wire2_expected1", 2, 1, "rejected", "frame_header", "PROTOCOL_VERSION_UNSUPPORTED"),
    ("hello_wire2_expected2", 2, 2, "rejected", "frame_header", "PROTOCOL_FRAME_INVALID"),
)


def _b2_t4_carrier(testcase):
    full_inputs = load_authored_inputs()
    accepted = _b1_build_accepted(full_inputs)
    testcase.assertEqual(set(accepted["hellos"].keys()), set(full_inputs["hellos"].keys()))
    testcase.assertEqual(set(accepted["selections"].keys()), set(full_inputs["selection_scenarios"].keys()))
    testcase.assertEqual(sorted(accepted["cases"].keys()), ["sig_multi", "ver_ws"])
    control = []
    entity_read.check_selection(full_inputs, accepted, control)
    testcase.assertIsInstance(control, list)
    for entry in control:
        testcase.assertIsInstance(entry, str)
    testcase.assertEqual(control, [])
    return full_inputs, accepted


def _b2_t4_authenticate(testcase, full_inputs, wire, *, version, session, request_id, kind, method, flags, body):
    testcase.assertGreaterEqual(len(wire), 8)
    testcase.assertEqual(int.from_bytes(wire[:8], "big"), len(wire) - 8)
    max_frame = int(full_inputs["selected_limits"]["max_frame_bytes"])
    stored, prefix = entity_read.split_wire(wire, max_frame)
    testcase.assertEqual(prefix, wire[:8])
    testcase.assertEqual(len(stored), int.from_bytes(prefix, "big"))
    payload, trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
    frame = entity_read.decode_frame_payload(payload)
    testcase.assertEqual(
        (frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"], frame["flags"]),
        (version, session, request_id, kind, method, flags),
    )
    testcase.assertEqual(frame["body"], body)
    return stored, payload, frame, trailer


def _b2_t4_check_valid_hello_body(testcase, body, offer):
    testcase.assertEqual(body, entity_read.build_hello(offer))
    fields = entity_read.parse_record(body)
    testcase.assertEqual(sorted(tag for tag, _ in fields), [1, 2, 3, 4, 5, 6, 7])


def _b2_t4_check_zero_bounds(testcase, bounds):
    bound_fields = entity_read.parse_record(bounds)
    testcase.assertEqual(sorted(tag for tag, _ in bound_fields), [1, 2, 3, 4, 5, 6, 7, 8])
    limits_raw = entity_read.single_field(bound_fields, 1)
    limit_fields = entity_read.parse_record(limits_raw)
    testcase.assertEqual(sorted(tag for tag, _ in limit_fields), [1, 2, 3, 4, 5, 6, 7, 8])
    for tag, payload in limit_fields:
        width = 32 if tag in _B2_U32_LIMIT_TAGS else 64
        testcase.assertEqual(entity_read.decode_uvar_exact(payload, width), 0)
    decoded = entity_read.decode_bounds(bounds)
    testcase.assertEqual(
        (
            decoded["returned_bytes"],
            decoded["returned_entities"],
            decoded["returned_edges"],
            decoded["reached_depth"],
            decoded["omitted"],
        ),
        (0, 0, 0, 0, 0),
    )
    testcase.assertFalse(decoded["truncated"])
    testcase.assertFalse(decoded["continuation"])


def _b2_t4_valid_row(testcase, full_inputs, *, key, wire_version, expected_version, expect, expected_layer, expected_code, session=None, request_id=0, method=0, flags=0):
    offer = full_inputs["hellos"]["hello_v2_client"]
    body = entity_read.build_hello(offer)
    _b2_t4_check_valid_hello_body(testcase, body, offer)
    bounds = entity_read.zero_bounds()
    _b2_t4_check_zero_bounds(testcase, bounds)
    payload = entity_read.build_frame_payload(wire_version, session, request_id, 4, method, flags, bounds, body)
    wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
    stored, _payload_bytes, _frame, trailer = _b2_t4_authenticate(
        testcase,
        full_inputs,
        wire,
        version=wire_version,
        session=session,
        request_id=request_id,
        kind=4,
        method=method,
        flags=flags,
        body=body,
    )
    testcase.assertEqual(stored[:-32], preimage)
    testcase.assertEqual(stored[-32:], frame_id)
    testcase.assertEqual(trailer, frame_id)
    authored = {
        "hello": "hello_v2_client",
        "wire_version": wire_version,
        "expected_version": expected_version,
        "expect": expect,
        "expected_layer": expected_layer,
        "expected_code": expected_code,
    }
    supplied = {
        **authored,
        "body_hex": body.hex(),
        "wire_hex": wire.hex(),
        "preimage_hex": preimage.hex(),
        "frame_id": frame_id.hex(),
    }
    testcase.assertEqual(
        set(authored.keys()),
        {"hello", "wire_version", "expected_version", "expect", "expected_layer", "expected_code"},
    )
    testcase.assertEqual(len(authored), 6)
    testcase.assertEqual(len(supplied), 10)
    testcase.assertEqual(supplied["body_hex"], body.hex())
    testcase.assertEqual(supplied["wire_hex"], wire.hex())
    testcase.assertEqual(supplied["preimage_hex"], preimage.hex())
    testcase.assertEqual(supplied["frame_id"], frame_id.hex())
    return authored, supplied, wire, preimage, frame_id, body, bounds


def _b2_t4_singleton(testcase, full_inputs, accepted, key, authored, supplied):
    local_inputs = copy.deepcopy(full_inputs)
    local_inputs["frame_scenarios"] = {key: copy.deepcopy(authored)}
    local_accepted = copy.deepcopy(accepted)
    local_accepted["frame_scenarios"] = {key: copy.deepcopy(supplied)}
    return local_inputs, local_accepted


def _b2_t4_expect_inventory(testcase, problems):
    testcase.assertIsInstance(problems, list)
    for entry in problems:
        testcase.assertIsInstance(entry, str)
    testcase.assertIn("accepted:frame_scenarios:inventory", problems)


class SuppliedHelloFrameCases(unittest.TestCase):
    def test_authored_hello_four_scenarios_are_exact(self) -> None:
        full_inputs, accepted = _b2_t4_carrier(self)
        fresh = load_authored_inputs()
        self.assertEqual(set(full_inputs["cases"].keys()), set(fresh["cases"].keys()))
        self.assertEqual(set(full_inputs["hellos"].keys()), set(fresh["hellos"].keys()))
        self.assertEqual(set(full_inputs["selection_scenarios"].keys()), set(fresh["selection_scenarios"].keys()))
        self.assertEqual(set(full_inputs["hellos"].keys()), set(accepted["hellos"].keys()))
        self.assertEqual(set(full_inputs["selection_scenarios"].keys()), set(accepted["selections"].keys()))
        authored_rows = {}
        supplied_rows = {}
        for key, wire_version, expected_version, expect, expected_layer, expected_code in _B2_T4_MATRIX:
            with self.subTest(row=key):
                authored, supplied, wire, preimage, frame_id, body, bounds = _b2_t4_valid_row(
                    self,
                    full_inputs,
                    key=key,
                    wire_version=wire_version,
                    expected_version=expected_version,
                    expect=expect,
                    expected_layer=expected_layer,
                    expected_code=expected_code,
                )
                self.assertEqual(
                    authored,
                    {
                        "hello": "hello_v2_client",
                        "wire_version": wire_version,
                        "expected_version": expected_version,
                        "expect": expect,
                        "expected_layer": expected_layer,
                        "expected_code": expected_code,
                    },
                )
                offer = full_inputs["hellos"]["hello_v2_client"]
                self.assertEqual(supplied["body_hex"], entity_read.build_hello(offer).hex())
                self.assertEqual(supplied["wire_hex"], wire.hex())
                self.assertEqual(supplied["preimage_hex"], preimage.hex())
                self.assertEqual(supplied["frame_id"], frame_id.hex())
                authored_rows[key] = authored
                supplied_rows[key] = supplied
        self.assertEqual(
            sorted(authored_rows.keys()),
            ["hello_wire1_expected1", "hello_wire1_expected2", "hello_wire2_expected1", "hello_wire2_expected2"],
        )
        self.assertEqual(sorted(supplied_rows.keys()), sorted(authored_rows.keys()))
        local_inputs = copy.deepcopy(full_inputs)
        local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
        local_accepted = copy.deepcopy(accepted)
        local_accepted["frame_scenarios"] = copy.deepcopy(supplied_rows)
        hello_problems = []
        entity_read.check_selection(local_inputs, local_accepted, hello_problems)
        self.assertIsInstance(hello_problems, list)
        for entry in hello_problems:
            self.assertIsInstance(entry, str)
        self.assertEqual(hello_problems, [])

    def test_supplied_hello_version_matrix_is_semantically_checked(self) -> None:
        full_inputs, accepted = _b2_t4_carrier(self)
        with self.subTest(row="hello_wire1_expected1-accepted-control"):
            authored, supplied, _wire, _pre, _fid, _body, _bounds = _b2_t4_valid_row(
                self,
                full_inputs,
                key="hello_wire1_expected1",
                wire_version=1,
                expected_version=1,
                expect="accepted",
                expected_layer=None,
                expected_code=None,
            )
            local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            self.assertIsInstance(problems, list)
            for entry in problems:
                self.assertIsInstance(entry, str)
            self.assertEqual(problems, [])
        wrong_rows = (
            ("hello_wire1_expected2", 1, 2, "PROTOCOL_DOWNGRADE"),
            ("hello_wire2_expected1", 2, 1, "PROTOCOL_VERSION_UNSUPPORTED"),
            ("hello_wire2_expected2", 2, 2, "PROTOCOL_FRAME_INVALID"),
        )
        for key, wire_version, expected_version, code in wrong_rows:
            with self.subTest(row=key, variant="wrong-accepted-claim"):
                authored, supplied, _wire, _pre, _fid, _body, _bounds = _b2_t4_valid_row(
                    self,
                    full_inputs,
                    key=key,
                    wire_version=wire_version,
                    expected_version=expected_version,
                    expect="accepted",
                    expected_layer=None,
                    expected_code=None,
                )
                local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, key, authored, supplied)
                problems = []
                entity_read.check_selection(local_inputs, local_accepted, problems)
                _b2_hello_expect(self, problems, key, "frame_header", code)
        for key, wire_version, expected_version, code in wrong_rows:
            with self.subTest(row=key, variant="correct-rejection-control"):
                layer = "frame_header"
                expect = "rejected"
                authored, supplied, _wire, _pre, _fid, _body, _bounds = _b2_t4_valid_row(
                    self,
                    full_inputs,
                    key=key,
                    wire_version=wire_version,
                    expected_version=expected_version,
                    expect=expect,
                    expected_layer=layer,
                    expected_code=code,
                )
                local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, key, authored, supplied)
                problems = []
                entity_read.check_selection(local_inputs, local_accepted, problems)
                self.assertIsInstance(problems, list)
                for entry in problems:
                    self.assertIsInstance(entry, str)
                self.assertEqual(problems, [])

    def test_supplied_hello_header_shape(self) -> None:
        full_inputs, accepted = _b2_t4_carrier(self)
        with self.subTest(variant="valid-control"):
            authored, supplied, _wire, _pre, _fid, _body, _bounds = _b2_t4_valid_row(
                self,
                full_inputs,
                key="hello_wire1_expected1",
                wire_version=1,
                expected_version=1,
                expect="accepted",
                expected_layer=None,
                expected_code=None,
            )
            local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            self.assertIsInstance(problems, list)
            for entry in problems:
                self.assertIsInstance(entry, str)
            self.assertEqual(problems, [])
        session = bytes.fromhex(full_inputs["context"]["session"])
        self.assertEqual(len(session), 32)
        shape_rows = (
            ("non-none-session", {"session": session}),
            ("request-id-1", {"request_id": 1}),
            ("method-306", {"method": 306}),
            ("flags-1", {"flags": 1}),
            ("flags-8", {"flags": 8}),
        )
        for name, overrides in shape_rows:
            with self.subTest(variant=name):
                authored, supplied, _wire, _pre, _fid, _body, _bounds = _b2_t4_valid_row(
                    self,
                    full_inputs,
                    key="hello_wire1_expected1",
                    wire_version=1,
                    expected_version=1,
                    expect="accepted",
                    expected_layer=None,
                    expected_code=None,
                    session=overrides.get("session"),
                    request_id=overrides.get("request_id", 0),
                    method=overrides.get("method", 0),
                    flags=overrides.get("flags", 0),
                )
                local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
                problems = []
                entity_read.check_selection(local_inputs, local_accepted, problems)
                _b2_hello_expect(self, problems, "hello_wire1_expected1", "frame_header", "PROTOCOL_FRAME_INVALID")
        version_shape_rows = (
            ("lower-version-plus-bad-shape", "hello_wire1_expected2", 1, 2, "PROTOCOL_DOWNGRADE"),
            ("higher-version-plus-bad-shape", "hello_wire2_expected1", 2, 1, "PROTOCOL_VERSION_UNSUPPORTED"),
        )
        for name, key, wire_version, expected_version, expected_code in version_shape_rows:
            with self.subTest(variant=name):
                authored, supplied, _wire, _pre, _fid, _body, _bounds = _b2_t4_valid_row(
                    self,
                    full_inputs,
                    key=key,
                    wire_version=wire_version,
                    expected_version=expected_version,
                    expect="accepted",
                    expected_layer=None,
                    expected_code=None,
                    flags=8,
                )
                local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, key, authored, supplied)
                problems = []
                entity_read.check_selection(local_inputs, local_accepted, problems)
                _b2_hello_expect(self, problems, key, "frame_header", expected_code)

    def test_supplied_hello_body_and_bounds_are_decoded(self) -> None:
        full_inputs, accepted = _b2_t4_carrier(self)
        offer = full_inputs["hellos"]["hello_v2_client"]
        valid_body = entity_read.build_hello(offer)
        with self.subTest(variant="valid-control"):
            authored, supplied, _wire, _pre, _fid, _body, _bounds = _b2_t4_valid_row(
                self,
                full_inputs,
                key="hello_wire1_expected1",
                wire_version=1,
                expected_version=1,
                expect="accepted",
                expected_layer=None,
                expected_code=None,
            )
            local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            self.assertIsInstance(problems, list)
            for entry in problems:
                self.assertIsInstance(entry, str)
            self.assertEqual(problems, [])
        with self.subTest(variant="empty-body-premise"):
            empty_body = entity_read.encode_fields([])
            self.assertNotEqual(empty_body, valid_body)
            with self.assertRaisesRegex(ScbError, "SCB_FIELD_MISSING"):
                entity_read.exact_fields(entity_read.parse_record(empty_body), [1, 2, 3, 4, 5, 6, 7])
        with self.subTest(variant="empty-body"):
            empty_body = entity_read.encode_fields([])
            bounds = entity_read.zero_bounds()
            payload = entity_read.build_frame_payload(1, None, 0, 4, 0, 0, bounds, empty_body)
            wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
            stored, _payload_bytes, frame, trailer = _b2_t4_authenticate(
                self, full_inputs, wire, version=1, session=None, request_id=0, kind=4, method=0, flags=0, body=empty_body
            )
            self.assertEqual(stored[:-32], preimage)
            self.assertEqual(trailer, frame_id)
            authored = {
                "hello": "hello_v2_client",
                "wire_version": 1,
                "expected_version": 1,
                "expect": "accepted",
                "expected_layer": None,
                "expected_code": None,
            }
            supplied = {
                **authored,
                "body_hex": empty_body.hex(),
                "wire_hex": wire.hex(),
                "preimage_hex": preimage.hex(),
                "frame_id": frame_id.hex(),
            }
            local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_hello_expect(self, problems, "hello_wire1_expected1", "hello_body", "SCB_FIELD_MISSING")
        with self.subTest(variant="truncated-body-premise"):
            with self.assertRaisesRegex(ScbError, "SCB_LENGTH_OVERFLOW"):
                entity_read.parse_record(b"\x80")
        with self.subTest(variant="truncated-body"):
            truncated_body = b"\x80"
            self.assertNotEqual(truncated_body, valid_body)
            with self.assertRaisesRegex(ScbError, "SCB_LENGTH_OVERFLOW"):
                entity_read.Reader(truncated_body).uvar(64)
            bounds = entity_read.zero_bounds()
            payload = entity_read.build_frame_payload(1, None, 0, 4, 0, 0, bounds, truncated_body)
            wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
            stored, _payload_bytes, frame, trailer = _b2_t4_authenticate(
                self, full_inputs, wire, version=1, session=None, request_id=0, kind=4, method=0, flags=0, body=truncated_body
            )
            self.assertEqual(stored[:-32], preimage)
            self.assertEqual(trailer, frame_id)
            authored = {
                "hello": "hello_v2_client",
                "wire_version": 1,
                "expected_version": 1,
                "expect": "accepted",
                "expected_layer": None,
                "expected_code": None,
            }
            supplied = {
                **authored,
                "body_hex": truncated_body.hex(),
                "wire_hex": wire.hex(),
                "preimage_hex": preimage.hex(),
                "frame_id": frame_id.hex(),
            }
            local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_hello_expect(self, problems, "hello_wire1_expected1", "hello_body", "SCB_LENGTH_OVERFLOW")
        for missing in (1, 2, 3, 4, 5, 6, 7, 8):
            with self.subTest(variant="missing-applied-tag", tag=missing):
                limits_raw = encode_record([(tag, encode_uvar(0)) for tag in (1, 2, 3, 4, 5, 6, 7, 8) if tag != missing])
                with self.assertRaisesRegex(ScbError, "SCB_FIELD_MISSING"):
                    entity_read.exact_fields(entity_read.parse_record(limits_raw), [1, 2, 3, 4, 5, 6, 7, 8])
                bounds = encode_record(
                    [
                        (1, limits_raw),
                        (2, encode_uvar(0)),
                        (3, encode_uvar(0)),
                        (4, encode_uvar(0)),
                        (5, encode_uvar(0)),
                        (6, encode_uvar(0)),
                        (7, encode_uvar(1)),
                        (8, encode_uvar(1)),
                    ]
                )
                payload = entity_read.build_frame_payload(1, None, 0, 4, 0, 0, bounds, valid_body)
                wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
                stored, _payload_bytes, frame, trailer = _b2_t4_authenticate(
                    self, full_inputs, wire, version=1, session=None, request_id=0, kind=4, method=0, flags=0, body=valid_body
                )
                self.assertEqual(stored[:-32], preimage)
                self.assertEqual(trailer, frame_id)
                authored = {
                    "hello": "hello_v2_client",
                    "wire_version": 1,
                    "expected_version": 1,
                    "expect": "accepted",
                    "expected_layer": None,
                    "expected_code": None,
                }
                supplied = {
                    **authored,
                    "body_hex": valid_body.hex(),
                    "wire_hex": wire.hex(),
                    "preimage_hex": preimage.hex(),
                    "frame_id": frame_id.hex(),
                }
                local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
                problems = []
                entity_read.check_selection(local_inputs, local_accepted, problems)
                _b2_hello_expect(self, problems, "hello_wire1_expected1", "frame_bounds", "SCB_FIELD_MISSING")
        with self.subTest(variant="truncated-applied-premise"):
            with self.assertRaisesRegex(ScbError, "SCB_LENGTH_OVERFLOW"):
                entity_read.decode_uvar_exact(b"\x80", 64)
        with self.subTest(variant="truncated-applied-scalar"):
            limits_raw = encode_record(
                [(1, b"\x80")] + [(tag, encode_uvar(0)) for tag in (2, 3, 4, 5, 6, 7, 8)]
            )
            self.assertEqual(entity_read.single_field(entity_read.parse_record(limits_raw), 1), b"\x80")
            with self.assertRaisesRegex(ScbError, "SCB_LENGTH_OVERFLOW"):
                entity_read.decode_uvar_exact(entity_read.single_field(entity_read.parse_record(limits_raw), 1), 64)
            bounds = encode_record(
                [
                    (1, limits_raw),
                    (2, encode_uvar(0)),
                    (3, encode_uvar(0)),
                    (4, encode_uvar(0)),
                    (5, encode_uvar(0)),
                    (6, encode_uvar(0)),
                    (7, encode_uvar(1)),
                    (8, encode_uvar(1)),
                ]
            )
            payload = entity_read.build_frame_payload(1, None, 0, 4, 0, 0, bounds, valid_body)
            wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
            stored, _payload_bytes, frame, trailer = _b2_t4_authenticate(
                self, full_inputs, wire, version=1, session=None, request_id=0, kind=4, method=0, flags=0, body=valid_body
            )
            self.assertEqual(stored[:-32], preimage)
            self.assertEqual(trailer, frame_id)
            authored = {
                "hello": "hello_v2_client",
                "wire_version": 1,
                "expected_version": 1,
                "expect": "accepted",
                "expected_layer": None,
                "expected_code": None,
            }
            supplied = {
                **authored,
                "body_hex": valid_body.hex(),
                "wire_hex": wire.hex(),
                "preimage_hex": preimage.hex(),
                "frame_id": frame_id.hex(),
            }
            local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_hello_expect(self, problems, "hello_wire1_expected1", "frame_bounds", "SCB_LENGTH_OVERFLOW")
        with self.subTest(variant="u32-overflow-premise"):
            wide = encode_uvar(2**32)
            with self.assertRaisesRegex(ScbError, "SCB_INTEGER_OVERFLOW"):
                entity_read.decode_uvar_exact(wide, 32)
        with self.subTest(variant="u32-overflow-field-4"):
            wide = encode_uvar(2**32)
            limits_raw = encode_record(
                [(1, encode_uvar(0)), (2, encode_uvar(0)), (3, encode_uvar(0)), (4, wide)]
                + [(tag, encode_uvar(0)) for tag in (5, 6, 7, 8)]
            )
            with self.assertRaisesRegex(ScbError, "SCB_INTEGER_OVERFLOW"):
                entity_read.decode_uvar_exact(entity_read.single_field(entity_read.parse_record(limits_raw), 4), 32)
            bounds = encode_record(
                [
                    (1, limits_raw),
                    (2, encode_uvar(0)),
                    (3, encode_uvar(0)),
                    (4, encode_uvar(0)),
                    (5, encode_uvar(0)),
                    (6, encode_uvar(0)),
                    (7, encode_uvar(1)),
                    (8, encode_uvar(1)),
                ]
            )
            payload = entity_read.build_frame_payload(1, None, 0, 4, 0, 0, bounds, valid_body)
            wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
            stored, _payload_bytes, frame, trailer = _b2_t4_authenticate(
                self, full_inputs, wire, version=1, session=None, request_id=0, kind=4, method=0, flags=0, body=valid_body
            )
            self.assertEqual(stored[:-32], preimage)
            self.assertEqual(trailer, frame_id)
            authored = {
                "hello": "hello_v2_client",
                "wire_version": 1,
                "expected_version": 1,
                "expect": "accepted",
                "expected_layer": None,
                "expected_code": None,
            }
            supplied = {
                **authored,
                "body_hex": valid_body.hex(),
                "wire_hex": wire.hex(),
                "preimage_hex": preimage.hex(),
                "frame_id": frame_id.hex(),
            }
            local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_hello_expect(self, problems, "hello_wire1_expected1", "frame_bounds", "SCB_INTEGER_OVERFLOW")
        zero_limits_raw = entity_read.build_limits(
            {
                "max_frame_bytes": 0,
                "max_entities": 0,
                "max_edges": 0,
                "max_depth": 0,
                "max_response_bytes": 0,
                "max_work": 0,
                "max_inflight": 0,
                "max_sessions": 0,
            }
        )
        for flag_tag in (7, 8):
            for flag_value in (0, 3):
                with self.subTest(variant="bad-outer-flag", tag=flag_tag, value=flag_value):
                    fields = [(1, zero_limits_raw), (2, encode_uvar(0)), (3, encode_uvar(0)), (4, encode_uvar(0)), (5, encode_uvar(0)), (6, encode_uvar(0))]
                    fields.append((7, encode_uvar(flag_value) if flag_tag == 7 else encode_uvar(1)))
                    fields.append((8, encode_uvar(flag_value) if flag_tag == 8 else encode_uvar(1)))
                    bounds = encode_record(fields)
                    with self.assertRaisesRegex(ScbError, "SCB_UNION_INVALID"):
                        entity_read.decode_bounds(bounds)
                    payload = entity_read.build_frame_payload(1, None, 0, 4, 0, 0, bounds, valid_body)
                    wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
                    stored, _payload_bytes, frame, trailer = _b2_t4_authenticate(
                        self, full_inputs, wire, version=1, session=None, request_id=0, kind=4, method=0, flags=0, body=valid_body
                    )
                    self.assertEqual(stored[:-32], preimage)
                    self.assertEqual(trailer, frame_id)
                    authored = {
                        "hello": "hello_v2_client",
                        "wire_version": 1,
                        "expected_version": 1,
                        "expect": "accepted",
                        "expected_layer": None,
                        "expected_code": None,
                    }
                    supplied = {
                        **authored,
                        "body_hex": valid_body.hex(),
                        "wire_hex": wire.hex(),
                        "preimage_hex": preimage.hex(),
                        "frame_id": frame_id.hex(),
                    }
                    local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
                    problems = []
                    entity_read.check_selection(local_inputs, local_accepted, problems)
                    _b2_hello_expect(self, problems, "hello_wire1_expected1", "frame_bounds", "SCB_UNION_INVALID")
        with self.subTest(variant="header-before-body"):
            empty_body = entity_read.encode_fields([])
            payload = entity_read.build_frame_payload(1, None, 0, 4, 0, 1, entity_read.zero_bounds(), empty_body)
            wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
            stored, _payload_bytes, frame, trailer = _b2_t4_authenticate(
                self, full_inputs, wire, version=1, session=None, request_id=0, kind=4, method=0, flags=1, body=empty_body
            )
            self.assertEqual(stored[:-32], preimage)
            self.assertEqual(trailer, frame_id)
            authored = {
                "hello": "hello_v2_client",
                "wire_version": 1,
                "expected_version": 1,
                "expect": "accepted",
                "expected_layer": None,
                "expected_code": None,
            }
            supplied = {
                **authored,
                "body_hex": empty_body.hex(),
                "wire_hex": wire.hex(),
                "preimage_hex": preimage.hex(),
                "frame_id": frame_id.hex(),
            }
            local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_hello_expect(self, problems, "hello_wire1_expected1", "frame_header", "PROTOCOL_FRAME_INVALID")
        with self.subTest(variant="bounds-before-version"):
            limits_raw = encode_record([(tag, encode_uvar(0)) for tag in (2, 3, 4, 5, 6, 7, 8)])
            with self.assertRaisesRegex(ScbError, "SCB_FIELD_MISSING"):
                entity_read.exact_fields(entity_read.parse_record(limits_raw), [1, 2, 3, 4, 5, 6, 7, 8])
            bounds = encode_record(
                [
                    (1, limits_raw),
                    (2, encode_uvar(0)),
                    (3, encode_uvar(0)),
                    (4, encode_uvar(0)),
                    (5, encode_uvar(0)),
                    (6, encode_uvar(0)),
                    (7, encode_uvar(1)),
                    (8, encode_uvar(1)),
                ]
            )
            payload = entity_read.build_frame_payload(1, None, 0, 4, 0, 0, bounds, valid_body)
            wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
            stored, _payload_bytes, frame, trailer = _b2_t4_authenticate(
                self, full_inputs, wire, version=1, session=None, request_id=0, kind=4, method=0, flags=0, body=valid_body
            )
            self.assertEqual(stored[:-32], preimage)
            self.assertEqual(trailer, frame_id)
            authored = {
                "hello": "hello_v2_client",
                "wire_version": 1,
                "expected_version": 2,
                "expect": "accepted",
                "expected_layer": None,
                "expected_code": None,
            }
            supplied = {
                **authored,
                "body_hex": valid_body.hex(),
                "wire_hex": wire.hex(),
                "preimage_hex": preimage.hex(),
                "frame_id": frame_id.hex(),
            }
            local_inputs, local_accepted = _b2_t4_singleton(self, full_inputs, accepted, "hello_wire1_expected1", authored, supplied)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_hello_expect(self, problems, "hello_wire1_expected1", "frame_bounds", "SCB_FIELD_MISSING")

    def test_supplied_hello_scenario_inventory_and_components(self) -> None:
        full_inputs, accepted = _b2_t4_carrier(self)
        authored_rows = {}
        supplied_rows = {}
        for key, wire_version, expected_version, expect, expected_layer, expected_code in _B2_T4_MATRIX:
            with self.subTest(row=key, variant="matrix-baseline"):
                authored, supplied, _wire, _pre, _fid, _body, _bounds = _b2_t4_valid_row(
                    self,
                    full_inputs,
                    key=key,
                    wire_version=wire_version,
                    expected_version=expected_version,
                    expect=expect,
                    expected_layer=expected_layer,
                    expected_code=expected_code,
                )
                authored_rows[key] = authored
                supplied_rows[key] = supplied
        with self.subTest(variant="correct-matrix-control"):
            local_inputs = copy.deepcopy(full_inputs)
            local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
            local_accepted = copy.deepcopy(accepted)
            local_accepted["frame_scenarios"] = copy.deepcopy(supplied_rows)
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            self.assertIsInstance(problems, list)
            for entry in problems:
                self.assertIsInstance(entry, str)
            self.assertEqual(problems, [])
        for field in ("expected_layer", "expected_code"):
            with self.subTest(variant="supplied-absent-field", field=field):
                local_inputs = copy.deepcopy(full_inputs)
                local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
                local_accepted = copy.deepcopy(accepted)
                mutated = copy.deepcopy(supplied_rows)
                self.assertIn(field, authored_rows["hello_wire1_expected1"])
                self.assertIsNone(authored_rows["hello_wire1_expected1"][field])
                self.assertIn(field, mutated["hello_wire1_expected1"])
                del mutated["hello_wire1_expected1"][field]
                self.assertNotIn(field, mutated["hello_wire1_expected1"])
                for other in ("hello", "wire_version", "expected_version", "expect", "body_hex", "wire_hex", "preimage_hex", "frame_id"):
                    self.assertEqual(mutated["hello_wire1_expected1"][other], supplied_rows["hello_wire1_expected1"][other])
                local_accepted["frame_scenarios"] = mutated
                problems = []
                entity_read.check_selection(local_inputs, local_accepted, problems)
                self.assertIsInstance(problems, list)
                for entry in problems:
                    self.assertIsInstance(entry, str)
                with self.subTest(variant="supplied-absent-field", field=field, absence="no-frame-header"):
                    self.assertFalse(_b2_hello_matches(problems, "hello_wire1_expected1", "frame_header", None))
                with self.subTest(variant="supplied-absent-field", field=field, absence="no-hello-body"):
                    self.assertFalse(_b2_hello_matches(problems, "hello_wire1_expected1", "hello_body", None))
                _b2_hello_expect(self, problems, "hello_wire1_expected1", "scenario_binding", field)
        with self.subTest(variant="omit-one-scenario"):
            local_inputs = copy.deepcopy(full_inputs)
            local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
            local_accepted = copy.deepcopy(accepted)
            mutated = copy.deepcopy(supplied_rows)
            del mutated["hello_wire2_expected2"]
            local_accepted["frame_scenarios"] = mutated
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_t4_expect_inventory(self, problems)
        with self.subTest(variant="extra-scenario"):
            local_inputs = copy.deepcopy(full_inputs)
            local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
            local_accepted = copy.deepcopy(accepted)
            mutated = copy.deepcopy(supplied_rows)
            mutated["hello_extra"] = copy.deepcopy(supplied_rows["hello_wire1_expected1"])
            local_accepted["frame_scenarios"] = mutated
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_t4_expect_inventory(self, problems)
        target = "hello_wire1_expected1"
        field_mutations = (
            ("expected_version", 2),
            ("expect", "rejected"),
            ("expected_layer", "frame_header"),
            ("expected_code", "PROTOCOL_FRAME_INVALID"),
            ("hello", "hello_v2_server"),
            ("wire_version", 2),
        )
        for field, value in field_mutations:
            with self.subTest(variant="supplied-field", field=field):
                local_inputs = copy.deepcopy(full_inputs)
                local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
                local_accepted = copy.deepcopy(accepted)
                mutated = copy.deepcopy(supplied_rows)
                mutated[target][field] = value
                local_accepted["frame_scenarios"] = mutated
                problems = []
                entity_read.check_selection(local_inputs, local_accepted, problems)
                if field == "expected_version":
                    self.assertIsInstance(problems, list)
                    for entry in problems:
                        self.assertIsInstance(entry, str)
                    with self.subTest(variant="supplied-field", field=field, absence="no-frame-header"):
                        self.assertFalse(_b2_hello_matches(problems, target, "frame_header", None))
                _b2_hello_expect(self, problems, target, "scenario_binding", field)
        for field in ("body_hex", "preimage_hex", "frame_id"):
            with self.subTest(variant="detached-component", field=field):
                local_inputs = copy.deepcopy(full_inputs)
                local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
                local_accepted = copy.deepcopy(accepted)
                mutated = copy.deepcopy(supplied_rows)
                mutated[target][field] = _b1_flip_hex(mutated[target][field])
                self.assertNotEqual(mutated[target][field], supplied_rows[target][field])
                local_accepted["frame_scenarios"] = mutated
                problems = []
                entity_read.check_selection(local_inputs, local_accepted, problems)
                _b2_hello_expect(self, problems, target, "component_binding", field)
        with self.subTest(variant="bad-wire-header"):
            _bad_authored, bad_supplied, _w, _p, _f, _b, _bo = _b2_t4_valid_row(
                self,
                full_inputs,
                key=target,
                wire_version=2,
                expected_version=2,
                expect="accepted",
                expected_layer=None,
                expected_code=None,
            )
            local_inputs = copy.deepcopy(full_inputs)
            local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
            local_accepted = copy.deepcopy(accepted)
            mutated = copy.deepcopy(supplied_rows)
            mutated[target]["body_hex"] = bad_supplied["body_hex"]
            mutated[target]["wire_hex"] = bad_supplied["wire_hex"]
            mutated[target]["preimage_hex"] = bad_supplied["preimage_hex"]
            mutated[target]["frame_id"] = bad_supplied["frame_id"]
            local_accepted["frame_scenarios"] = mutated
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_hello_expect(self, problems, target, "frame_header", "PROTOCOL_VERSION_UNSUPPORTED")
        with self.subTest(variant="bad-wire-content"):
            _bad_authored, bad_supplied, _w, _p, _f, _b, _bo = _b2_t4_valid_row(
                self,
                full_inputs,
                key=target,
                wire_version=2,
                expected_version=2,
                expect="accepted",
                expected_layer=None,
                expected_code=None,
            )
            local_inputs = copy.deepcopy(full_inputs)
            local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
            local_accepted = copy.deepcopy(accepted)
            mutated = copy.deepcopy(supplied_rows)
            mutated[target]["body_hex"] = bad_supplied["body_hex"]
            mutated[target]["wire_hex"] = bad_supplied["wire_hex"]
            mutated[target]["preimage_hex"] = bad_supplied["preimage_hex"]
            mutated[target]["frame_id"] = bad_supplied["frame_id"]
            local_accepted["frame_scenarios"] = mutated
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            _b2_hello_expect(self, problems, target, "scenario_binding", "wire_hex")
        with self.subTest(variant="different-valid-body"):
            client_body = entity_read.build_hello(full_inputs["hellos"]["hello_v2_client"])
            server_body = entity_read.build_hello(full_inputs["hellos"]["hello_v2_server"])
            self.assertNotEqual(server_body, client_body)
            server_fields = entity_read.parse_record(server_body)
            self.assertEqual(sorted(tag for tag, _ in server_fields), [1, 2, 3, 4, 5, 6, 7])
            bounds = entity_read.zero_bounds()
            _b2_t4_check_zero_bounds(self, bounds)
            payload = entity_read.build_frame_payload(1, None, 0, 4, 0, 0, bounds, server_body)
            wire, preimage, frame_id = entity_read.build_envelope(entity_read.protocol_epoch_id(), payload)
            stored, _payload_bytes, frame, trailer = _b2_t4_authenticate(
                self, full_inputs, wire, version=1, session=None, request_id=0, kind=4, method=0, flags=0, body=server_body
            )
            self.assertEqual(stored[:-32], preimage)
            self.assertEqual(trailer, frame_id)
            local_inputs = copy.deepcopy(full_inputs)
            local_inputs["frame_scenarios"] = copy.deepcopy(authored_rows)
            local_accepted = copy.deepcopy(accepted)
            mutated = copy.deepcopy(supplied_rows)
            mutated[target]["body_hex"] = server_body.hex()
            mutated[target]["wire_hex"] = wire.hex()
            mutated[target]["preimage_hex"] = preimage.hex()
            mutated[target]["frame_id"] = frame_id.hex()
            self.assertEqual(mutated[target]["hello"], "hello_v2_client")
            local_accepted["frame_scenarios"] = mutated
            problems = []
            entity_read.check_selection(local_inputs, local_accepted, problems)
            self.assertIsInstance(problems, list)
            for entry in problems:
                self.assertIsInstance(entry, str)
            with self.subTest(variant="different-valid-body", absence="no-hello-body"):
                self.assertFalse(_b2_hello_matches(problems, target, "hello_body", None))
            with self.subTest(variant="different-valid-body", absence="no-frame-header"):
                self.assertFalse(_b2_hello_matches(problems, target, "frame_header", None))
            _b2_hello_expect(self, problems, target, "scenario_binding", "hello")


def _b2_complete_accepted(testcase):
    full_inputs = load_authored_inputs()
    testcase.assertEqual(len(full_inputs["cases"]), 23)
    base = _b1_build_accepted(full_inputs)
    testcase.assertEqual(set(base["hellos"].keys()), set(full_inputs["hellos"].keys()))
    testcase.assertEqual(set(base["selections"].keys()), set(full_inputs["selection_scenarios"].keys()))
    cases = {}
    kinds = set()
    for case_id, case in full_inputs["cases"].items():
        testcase.assertEqual(case.get("kind", "success"), "success")
        built = entity_read.build_success_case(full_inputs, case)
        cases[case_id] = {"id": case_id, **built}
        if case_id.startswith("ver_"):
            for ref in case["objects"]:
                kinds.add(int(full_inputs["entities"][ref]["kind"]))
    testcase.assertEqual(set(cases.keys()), set(full_inputs["cases"].keys()))
    testcase.assertEqual(len(cases), 23)
    testcase.assertEqual(len(kinds), 18)
    testcase.assertEqual(full_inputs["cases"]["sig_zero"]["objects"], ["sig_f0"])
    testcase.assertEqual(full_inputs["cases"]["sig_multi"]["objects"], ["sig_f", "sig_p_high", "sig_p_low"])
    accepted = {
        "contract": base["contract"],
        "claim": base["claim"],
        "manifest": base["manifest"],
        "cases": cases,
        "hellos": base["hellos"],
        "selections": base["selections"],
    }
    if "frame_scenarios" in full_inputs:
        expected_authored = {
            key: {
                "hello": "hello_v2_client",
                "wire_version": wire_version,
                "expected_version": expected_version,
                "expect": expect,
                "expected_layer": expected_layer,
                "expected_code": expected_code,
            }
            for key, wire_version, expected_version, expect, expected_layer, expected_code in _B2_T4_MATRIX
        }
        testcase.assertEqual(full_inputs["frame_scenarios"], expected_authored)
        testcase.assertIn("frame_scenarios", base)
        accepted["frame_scenarios"] = base["frame_scenarios"]
    else:
        testcase.assertNotIn("frame_scenarios", base)
    return full_inputs, accepted


_B2_COMPLETE_REJECTED_KINDS = frozenset(
    {
        "relation",
        "runtime_sequence",
        "failure_response",
        "failure_wire",
        "owner_case",
        "fill_recipe",
        "hello_invalid",
        "selection_invalid",
    }
)


def _b2_complete_rejected(testcase):
    full_inputs = load_authored_inputs()
    authored_rows = full_inputs["rejected"]
    testcase.assertEqual(len(authored_rows), 91)
    cases = []
    for authored in authored_rows:
        row = copy.deepcopy(authored)
        if "kind" not in row:
            base = full_inputs["cases"][row["base"]]
            row["input_hex"] = entity_read.build_rejected_bytes(full_inputs, base, row["recipe"]).hex()
        else:
            testcase.assertIn(row["kind"], _B2_COMPLETE_REJECTED_KINDS)
            if row["kind"] in ("failure_response", "failure_wire"):
                row["input_hex"] = entity_read.build_failure_wire(full_inputs, row).hex()
            elif row["kind"] == "relation":
                row = entity_read.derive_relation(full_inputs, row)
        cases.append(row)
    testcase.assertEqual([row["id"] for row in cases], [row["id"] for row in authored_rows])
    testcase.assertEqual(len(cases), 91)
    for row in cases:
        if row.get("kind") == "runtime_sequence":
            testcase.assertEqual(row["status"], "pending_runtime_comparison")
    inputs_sha = hashlib.sha256(json.dumps(full_inputs, sort_keys=True).encode()).hexdigest()
    rejected = {
        "contract": "sley2-entity-read-v2-rejected",
        "claim": "independent-expected",
        "manifest": {"inputs_sha256": inputs_sha},
        "cases": cases,
    }
    return full_inputs, rejected


def _b2_complete_local_matrix(testcase):
    full_inputs, accepted = _b2_complete_accepted(testcase)
    local_inputs = copy.deepcopy(full_inputs)
    authored_rows = {}
    supplied_rows = {}
    for key, wire_version, expected_version, expect, expected_layer, expected_code in _B2_T4_MATRIX:
        authored, supplied, _wire, _preimage, _frame_id, _body, _bounds = _b2_t4_valid_row(
            testcase,
            full_inputs,
            key=key,
            wire_version=wire_version,
            expected_version=expected_version,
            expect=expect,
            expected_layer=expected_layer,
            expected_code=expected_code,
        )
        authored_rows[key] = authored
        supplied_rows[key] = supplied
    local_inputs["frame_scenarios"] = authored_rows
    local_accepted = copy.deepcopy(accepted)
    local_accepted["frame_scenarios"] = supplied_rows
    local_accepted["manifest"] = {
        "inputs_sha256": hashlib.sha256(json.dumps(local_inputs, sort_keys=True).encode()).hexdigest()
    }
    testcase.assertEqual(len(local_accepted["cases"]), 23)
    testcase.assertEqual(set(local_accepted["cases"].keys()), set(local_inputs["cases"].keys()))
    return local_inputs, local_accepted


class B2OuterIntegrationCases(unittest.TestCase):
    def test_tracked_frame_scenario_inventory_matches_literal_matrix(self) -> None:
        inputs = load_authored_inputs()
        self.assertIn("frame_scenarios", inputs)
        scenarios = inputs["frame_scenarios"]
        self.assertIsInstance(scenarios, dict)
        expected = {
            "hello_wire1_expected1": {
                "hello": "hello_v2_client",
                "wire_version": 1,
                "expected_version": 1,
                "expect": "accepted",
                "expected_layer": None,
                "expected_code": None,
            },
            "hello_wire1_expected2": {
                "hello": "hello_v2_client",
                "wire_version": 1,
                "expected_version": 2,
                "expect": "rejected",
                "expected_layer": "frame_header",
                "expected_code": "PROTOCOL_DOWNGRADE",
            },
            "hello_wire2_expected1": {
                "hello": "hello_v2_client",
                "wire_version": 2,
                "expected_version": 1,
                "expect": "rejected",
                "expected_layer": "frame_header",
                "expected_code": "PROTOCOL_VERSION_UNSUPPORTED",
            },
            "hello_wire2_expected2": {
                "hello": "hello_v2_client",
                "wire_version": 2,
                "expected_version": 2,
                "expect": "rejected",
                "expected_layer": "frame_header",
                "expected_code": "PROTOCOL_FRAME_INVALID",
            },
        }
        self.assertEqual(set(scenarios.keys()), set(expected.keys()))
        for row_id, want in expected.items():
            with self.subTest(row=row_id):
                row = scenarios[row_id]
                self.assertIsInstance(row, dict)
                self.assertEqual(set(row.keys()), set(want.keys()))
                for field, value in want.items():
                    candidate = row[field]
                    if value is None:
                        self.assertIsNone(candidate)
                    elif isinstance(value, int):
                        self.assertIs(type(candidate), int)
                        self.assertEqual(candidate, value)
                    else:
                        self.assertIs(type(candidate), str)
                        self.assertEqual(candidate, value)
        self.assertEqual(
            sorted(inputs["cases"].keys()),
            [
                "sig_multi",
                "sig_zero",
                "ver_adapter",
                "ver_block",
                "ver_capreq",
                "ver_const",
                "ver_const_long",
                "ver_contract",
                "ver_contract_limits",
                "ver_depbinding",
                "ver_effect",
                "ver_entrypoint",
                "ver_func",
                "ver_gval",
                "ver_ns",
                "ver_ns_none",
                "ver_op",
                "ver_param",
                "ver_pkg",
                "ver_policy",
                "ver_testcase",
                "ver_typedef",
                "ver_ws",
            ],
        )
        self.assertEqual(
            sorted(inputs["hellos"].keys()),
            ["hello_v1_server", "hello_v2_client", "hello_v2_server", "hello_v3_client", "hello_v3_server"],
        )
        self.assertEqual(sorted(inputs["selection_scenarios"].keys()), ["v1_select", "v2_select", "v3_refuse"])

    def test_complete_authored_accepted_control(self) -> None:
        full_inputs, accepted = _b2_complete_accepted(self)
        before = copy.deepcopy(full_inputs)
        self.assertEqual(accepted["contract"], "sley2-entity-read-v2")
        self.assertEqual(accepted["claim"], "independent-expected")
        max_frame = int(full_inputs["selected_limits"]["max_frame_bytes"])
        for case_id in sorted(full_inputs["cases"].keys()):
            with self.subTest(case=case_id):
                case = full_inputs["cases"][case_id]
                built = accepted["cases"][case_id]
                for side in ("request", "response"):
                    wire = bytes.fromhex(built[f"{side}_wire_hex"])
                    stored, prefix = entity_read.split_wire(wire, max_frame)
                    self.assertEqual(prefix, wire[:8])
                    payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
                    frame = entity_read.decode_frame_payload(payload)
                    self.assertEqual(frame["version"], 2)
                    self.assertEqual(frame["kind"], 1 if side == "request" else 2)
                direct: list[str] = []
                entity_read.semantic_check(full_inputs, case, built, direct)
                self.assertIsInstance(direct, list)
                for entry in direct:
                    self.assertIsInstance(entry, str)
                self.assertEqual(direct, [])
        problems = entity_read.check_accepted(full_inputs, accepted)
        self.assertIsInstance(problems, list)
        for entry in problems:
            self.assertIsInstance(entry, str)
        self.assertEqual(problems, [])
        self.assertEqual(full_inputs, before)

    def test_outer_entity_wires_reach_semantics_and_content_comparison(self) -> None:
        full_inputs, accepted = _b2_complete_accepted(self)
        control = entity_read.check_accepted(full_inputs, accepted)
        self.assertIsInstance(control, list)
        for entry in control:
            self.assertIsInstance(entry, str)
        self.assertEqual(control, [])
        case = full_inputs["cases"]["ver_ws"]
        built = accepted["cases"]["ver_ws"]
        max_frame = int(full_inputs["selected_limits"]["max_frame_bytes"])
        for side in ("request", "response"):
            with self.subTest(side=side):
                wire = bytes.fromhex(built[f"{side}_wire_hex"])
                stored, _prefix = entity_read.split_wire(wire, max_frame)
                payload, _trailer = entity_read.check_envelope(stored, entity_read.protocol_epoch_id())
                frame = entity_read.decode_frame_payload(payload)
                self.assertEqual(frame["version"], 2)
                mutated_payload = entity_read.build_frame_payload(
                    3,
                    frame["session"],
                    frame["request_id"],
                    frame["kind"],
                    frame["method"],
                    frame["flags"],
                    frame["bounds"],
                    frame["body"],
                )
                mutated = _b2_supplied_with_payload(built, side, mutated_payload)
                fresh_wire = bytes.fromhex(mutated[f"{side}_wire_hex"])
                fresh_stored, fresh_prefix = entity_read.split_wire(fresh_wire, max_frame)
                self.assertEqual(fresh_prefix, fresh_wire[:8])
                self.assertEqual(fresh_stored[:-32], bytes.fromhex(mutated[f"{side}_preimage_hex"]))
                self.assertEqual(fresh_stored[-32:], bytes.fromhex(mutated[f"{side}_frame_id"]))
                fresh_frame = entity_read.decode_frame_payload(
                    entity_read.check_envelope(fresh_stored, entity_read.protocol_epoch_id())[0]
                )
                self.assertEqual(fresh_frame["version"], 3)
                self.assertEqual(fresh_frame["body"], frame["body"])
                self.assertEqual(fresh_frame["bounds"], frame["bounds"])
                self.assertEqual(fresh_frame["session"], frame["session"])
                self.assertEqual(fresh_frame["request_id"], frame["request_id"])
                self.assertEqual(fresh_frame["kind"], frame["kind"])
                self.assertEqual(fresh_frame["method"], frame["method"])
                self.assertEqual(fresh_frame["flags"], frame["flags"])
                self.assertEqual(mutated[f"{side}_body_hex"], built[f"{side}_body_hex"])
                self.assertEqual(mutated["objects"], built["objects"])
                self.assertEqual(mutated["work"], built["work"])
                self.assertEqual(mutated["count_k"], built["count_k"])
                self.assertEqual(mutated["stored_b"], built["stored_b"])
                direct: list[str] = []
                entity_read.semantic_check(full_inputs, case, mutated, direct)
                self.assertTrue(
                    _b2_matches(direct, "ver_ws", side, "frame_header", "PROTOCOL_VERSION_UNSUPPORTED"),
                    f"missing ver_ws:semantic:{side}:frame_header:PROTOCOL_VERSION_UNSUPPORTED in {direct!r}",
                )
                mutated_accepted = copy.deepcopy(accepted)
                mutated_accepted["cases"]["ver_ws"] = mutated
                problems = entity_read.check_accepted(full_inputs, mutated_accepted)
                self.assertIsInstance(problems, list)
                for entry in problems:
                    self.assertIsInstance(entry, str)
                with self.subTest(side=side, check="content"):
                    self.assertIn(f"ver_ws:{side}_wire_hex", problems)
                with self.subTest(side=side, check="semantics"):
                    self.assertTrue(
                        _b2_matches(problems, "ver_ws", side, "frame_header", "PROTOCOL_VERSION_UNSUPPORTED"),
                        f"missing ver_ws:semantic:{side}:frame_header:PROTOCOL_VERSION_UNSUPPORTED in {problems!r}",
                    )
                for marker in ("accepted:fields", "manifest:inputs-sha256", "ver_ws:rebuild", "ver_ws:missing-expected"):
                    self.assertFalse(
                        any(entry == marker or entry.startswith(marker + ":") for entry in problems), marker
                    )

    def test_complete_local_matrix_is_admitted_by_outer_schema(self) -> None:
        local_inputs, local_accepted = _b2_complete_local_matrix(self)
        self.assertEqual(len(local_accepted["cases"]), 23)
        self.assertEqual(set(local_accepted["cases"].keys()), set(local_inputs["cases"].keys()))
        self.assertEqual(set(local_accepted["hellos"].keys()), set(local_inputs["hellos"].keys()))
        self.assertEqual(set(local_accepted["selections"].keys()), set(local_inputs["selection_scenarios"].keys()))
        hello_problems: list[str] = []
        entity_read.check_selection(local_inputs, local_accepted, hello_problems)
        self.assertIsInstance(hello_problems, list)
        for entry in hello_problems:
            self.assertIsInstance(entry, str)
        self.assertEqual(hello_problems, [])
        problems = entity_read.check_accepted(local_inputs, local_accepted)
        self.assertIsInstance(problems, list)
        for entry in problems:
            self.assertIsInstance(entry, str)
        self.assertEqual(problems, [])

    def test_complete_rejected_staging_prerequisite(self) -> None:
        full_inputs, rejected = _b2_complete_rejected(self)
        self.assertEqual(rejected["contract"], "sley2-entity-read-v2-rejected")
        self.assertEqual(rejected["claim"], "independent-expected")
        self.assertEqual(len(rejected["cases"]), 91)
        self.assertEqual(
            [row["id"] for row in rejected["cases"]],
            [row["id"] for row in full_inputs["rejected"]],
        )
        problems = entity_read.check_rejected(full_inputs, rejected)
        self.assertIsInstance(problems, list)
        for entry in problems:
            self.assertIsInstance(entry, str)
        self.assertEqual(problems, [])


if __name__ == "__main__":
    unittest.main()
