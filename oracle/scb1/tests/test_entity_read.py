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

import unittest

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
        self.assertEqual(entity_read.PROTOCOL_DECODER_LIMITS_HASH, "212b293d90646621c2e83caf095c2a21b6864888a3cc9887b7f6ab81ab98fe7")
        self.assertEqual(len(entity_read.protocol_epoch_id()), 32)

    def test_retryability_mapping(self) -> None:
        self.assertEqual(entity_read.retryability("PROTOCOL_LIMIT_EXCEEDED"), 4)
        self.assertEqual(entity_read.retryability("PROTOCOL_PAYLOAD_INVALID"), 1)
        self.assertEqual(entity_read.retryability("QUERY_ROOT_MISMATCH"), 1)


if __name__ == "__main__":
    unittest.main()
