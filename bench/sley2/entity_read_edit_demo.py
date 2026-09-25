"""AT-MW-02 I4b response-derived edit demonstrations.

Two bounded demonstrations run over the real runner/stdio pipeline
(`sley serve`, version-2 profile):

* ``expression``: read the exact current Operation object with
  ``entity.version`` (306), derive a canonical opcode-swap edit from its
  returned fields, and create plus successfully validate the candidate.
* ``signature``: read a Function plus its ordered Parameters with
  ``entity.signature`` (307), derive a parameter-type-dependent operand
  edit for the function's Operation from declaration order and exact
  parameter types, and create plus successfully validate the candidate.

Agent-input discipline: the harness stage owns the fixture exchange bytes
(import + session open). The agent stage receives only identities (target
operation/function, principal) plus the transacting callable, and learns
every body through live reads. The frame transcript proves it: any fact
the agent used arrived over the wire.
"""

from __future__ import annotations

import os
import time
from collections.abc import Callable, Mapping
from typing import Any

from bench.sley2.runner import ARM_AFFORDANCES, ARM_DENIED_METHODS

from sley2_scb1_oracle import candidate as CAN
from sley2_scb1_oracle import candidate_result as CR
from sley2_scb1_oracle import entity_read as ER
from sley2_scb1_oracle.codec import encode_record, encode_sized, encode_uvar

import blake3


ALLOWLIST = tuple(ARM_AFFORDANCES)
DENIED = frozenset(ARM_DENIED_METHODS)

# Agent-chosen bounded limits for the two reads: small ceilings keep the
# charged work bound (1 + K*L + 2*B + max_response_bytes) far inside the
# session budget for single-object responses.
READ_MAX_OBJECTS = 8
READ_MAX_RESPONSE_BYTES = 65_536
READ_MAX_WORK = 1_000_000

BOOL_AND = 103
BOOL_OR = 104

CAPABILITY_SUMMARY_MAGIC = b"SLEYCAS1"
CAPABILITY_SUMMARY_VERSION = 1
CAPABILITY_SUMMARY_DOMAIN = b"sley2.capability-summary.v1"


class DemoError(Exception):
    """The demonstration refused to proceed rather than guess."""


def parse_list(payload: bytes) -> list[bytes]:
    reader = ER.Reader(payload)
    count = reader.uvar(64)
    if count > 1_000_000:
        raise DemoError("list too large")
    items = [reader.sized(16_777_216) for _ in range(count)]
    reader.finish()
    return items


def fixed32(payload: bytes) -> bytes:
    if len(payload) != 32:
        raise DemoError("not 32 bytes")
    return payload


def uvar32(payload: bytes) -> int:
    return ER.Reader(payload).uvar(32)


def union_tag(payload: bytes) -> tuple[int, bytes]:
    reader = ER.Reader(payload)
    tag, body = reader.union()
    reader.finish()
    return tag, body


def decode_valueref(raw: bytes) -> dict[str, Any]:
    tag, pay = union_tag(raw)
    if tag == 1:
        return {"variant": "Parameter", "value": fixed32(pay).hex()}
    if tag == 2:
        fields = dict(ER.parse_record(pay))
        return {
            "variant": "OperationResult",
            "value": {
                "operation": fixed32(fields[1]).hex(),
                "result_index": uvar32(fields[2]),
            },
        }
    raise DemoError(f"unknown ValueRef tag {tag}")


def decode_typeexpr(raw: bytes) -> dict[str, Any]:
    tag, pay = union_tag(raw)
    names = {1: "Unit", 2: "Bool", 5: "F32", 6: "F64", 7: "Bytes", 8: "Text"}
    if tag not in names or pay != b"":
        raise DemoError(f"unsupported TypeExpr tag {tag}")
    return {"variant": names[tag]}


def decode_operation_body(body: bytes) -> dict[str, Any]:
    fields = dict(ER.parse_record(body))
    if sorted(fields) != [1, 2, 3, 4, 5, 6]:
        raise DemoError("operation body shape")
    imm_tag, imm_pay = union_tag(fields[6])
    if (imm_tag, imm_pay) != (1, b""):
        raise DemoError("operation immediate must be None for the demo edit")
    return {
        "block": fixed32(fields[1]).hex(),
        "ordinal": uvar32(fields[2]),
        "opcode": uvar32(fields[3]),
        "operands": [decode_valueref(item) for item in parse_list(fields[4])],
        "result_types": [decode_typeexpr(item) for item in parse_list(fields[5])],
        "immediate": {"variant": "None"},
    }


def decode_parameter_body(body: bytes) -> dict[str, Any]:
    fields = dict(ER.parse_record(body))
    if sorted(fields) != [1, 2, 3, 4]:
        raise DemoError("parameter body shape")
    if uvar32(fields[2]) != 1:
        raise DemoError("parameter role must be Function for the demo edit")
    return {
        "owner": fixed32(fields[1]).hex(),
        "ordinal": uvar32(fields[3]),
        "value_type": decode_typeexpr(fields[4]),
    }


def decode_function_body(body: bytes) -> dict[str, Any]:
    fields = dict(ER.parse_record(body))
    if sorted(fields) != [1, 2, 3, 4, 5, 6, 7, 8]:
        raise DemoError("function body shape")
    return {
        "parameters": [fixed32(item).hex() for item in parse_list(fields[2])],
        "result_type": decode_typeexpr(fields[3]),
        "entry_block": fixed32(fields[5]).hex(),
    }


def empty_capability_summary_digest(
    principal: bytes, workspace: bytes, policy_root: bytes, state_root: bytes
) -> bytes:
    """SLEYCAS1 projection over the empty token set (server recomputes this)."""
    record = encode_record(
        [
            (1, encode_uvar(1)),
            (2, principal),
            (3, workspace),
            (4, policy_root),
            (5, state_root),
            (6, encode_uvar(0)),
        ]
    )
    preimage = CAPABILITY_SUMMARY_MAGIC + encode_uvar(CAPABILITY_SUMMARY_VERSION) + encode_sized(record)
    hasher = blake3.blake3()
    hasher.update(CAPABILITY_SUMMARY_DOMAIN)
    hasher.update(preimage)
    return hasher.digest()


def discover(transact: Callable[..., dict[str, Any]]) -> dict[str, bytes]:
    """Refs, head, and revision summary through live reads only."""
    listed = transact("refs.list", "0a")
    summaries = parse_list(bytes.fromhex(listed["body"]))
    if not summaries:
        raise DemoError("no branches")
    first = dict(ER.parse_record(summaries[0]))
    name = ER.Reader(first[1]).sized(1_000_000)
    resolved = transact("refs.resolve", name.hex())
    head = dict(ER.parse_record(bytes.fromhex(resolved["body"])))
    revision = transact("revision.read", head[3].hex())
    fields = dict(ER.parse_record(bytes.fromhex(revision["body"])))
    return {
        "branch": name,
        "head_tx": fixed32(fields[1]),
        "root": fixed32(fields[2]),
        "policy": fixed32(fields[3]),
        "workspace": fixed32(fields[4]),
        "epoch": fixed32(fields[5]),
    }


def read_operation(
    transact: Callable[..., dict[str, Any]], root: bytes, target: bytes
) -> dict[str, Any]:
    request = ER.build_request_body(root, target, READ_MAX_OBJECTS, READ_MAX_RESPONSE_BYTES, READ_MAX_WORK)
    answer = transact("entity.version", request.hex())
    response = ER.decode_response_body(bytes.fromhex(answer["body"]))
    if len(response["entries"]) != 1:
        raise DemoError("entity.version must return exactly one object")
    entry = ER.decode_response_entry(response["entries"][0])
    if entry["entity"] != target or entry["kind"] != 8:
        raise DemoError("entity.version returned the wrong object")
    checked = ER.check_stored_object(entry["stored"], 8, target, response["epoch"])
    return {
        "object_id": checked["object_id"],
        "body": decode_operation_body(checked["body"]),
        "work": response["work"],
        "epoch": response["epoch"],
    }


def read_signature(
    transact: Callable[..., dict[str, Any]], root: bytes, target: bytes
) -> dict[str, Any]:
    request = ER.build_request_body(root, target, READ_MAX_OBJECTS, READ_MAX_RESPONSE_BYTES, READ_MAX_WORK)
    answer = transact("entity.signature", request.hex())
    response = ER.decode_response_body(bytes.fromhex(answer["body"]))
    if not response["entries"]:
        raise DemoError("entity.signature returned no objects")
    function_entry = ER.decode_response_entry(response["entries"][0])
    if function_entry["entity"] != target or function_entry["kind"] != 5:
        raise DemoError("entity.signature head is not the requested Function")
    function_checked = ER.check_stored_object(
        function_entry["stored"], 5, target, response["epoch"]
    )
    function = decode_function_body(function_checked["body"])
    parameters = []
    for raw in response["entries"][1:]:
        entry = ER.decode_response_entry(raw)
        if entry["kind"] != 6:
            raise DemoError("entity.signature trailing entry is not a Parameter")
        checked = ER.check_stored_object(entry["stored"], 6, entry["entity"], response["epoch"])
        parameters.append({"entity": entry["entity"].hex(), **decode_parameter_body(checked["body"])})
    declared = function["parameters"]
    if [item["entity"] for item in parameters] != declared:
        raise DemoError("parameter order disagrees with the Function declaration")
    for position, item in enumerate(parameters):
        if item["owner"] != target.hex() or item["ordinal"] != position:
            raise DemoError("parameter owner/ordinal disagrees with declaration order")
    return {
        "function": function,
        "parameters": parameters,
        "work": response["work"],
        "epoch": response["epoch"],
    }


def submit_edit(
    transact: Callable[..., dict[str, Any]],
    context: Mapping[str, bytes],
    principal: bytes,
    target: bytes,
    target_kind: int,
    object_id: bytes,
    new_body: Mapping[str, Any],
    body_type: str,
) -> dict[str, Any]:
    now_ms = int(time.time() * 1_000)
    candidate = CAN.build_candidate(
        {
            "format_version": 1,
            "workspace_id": context["workspace"].hex(),
            "base_transaction_id": context["head_tx"].hex(),
            "base_root": context["root"].hex(),
            "schema_epoch_id": context["epoch"].hex(),
            "policy_root_id": context["policy"].hex(),
            "principal_id": principal.hex(),
            "capability_summary_digest": empty_capability_summary_digest(
                principal, context["workspace"], context["policy"], context["root"]
            ).hex(),
            "operations": [
                {
                    "ordinal": 0,
                    "class": "ReplaceEntityVersion",
                    "target_kind": target_kind,
                    "target_entity": target.hex(),
                    "field_tag": None,
                    "payload": dict(new_body),
                    "precondition_ordinal": 0,
                }
            ],
            "preconditions": [
                {
                    "operation_ordinal": 0,
                    "requirement": "ExactEntityVersion",
                    "payload": {"entity_id": target.hex(), "object_id": object_id.hex()},
                }
            ],
            "validation_profile_id": CAN.validation_profile_id().hex(),
            "candidate_nonce": os.urandom(32).hex(),
            "expiry": {"clock": 1, "not_after": now_ms + 3_600_000},
        }
    )
    created = transact("candidate.create", candidate["record"].hex())
    stored = bytes.fromhex(created["body"])
    CAN.import_candidate(stored)
    validate_body = encode_record(
        [
            (1, context["head_tx"]),
            (2, principal),
            (3, encode_uvar(now_ms)),
            (4, stored),
        ]
    )
    validated = transact("candidate.validate", validate_body.hex())
    result = CR.decode_candidate_result(bytes.fromhex(validated["body"]))
    if result["decision_tag"] != 1 or result["failed_phase"] is not None:
        raise DemoError(f"candidate did not validate: {result}")
    return {
        "candidate_id": candidate["candidate_id"].hex(),
        "result": result,
        "stored_hex": stored.hex(),
        "record_hex": candidate["record"].hex(),
    }


def demo_expression_edit(
    transact: Callable[..., dict[str, Any]], operation: bytes, principal: bytes
) -> dict[str, Any]:
    """Read one Operation, swap its Boolean binary opcode, validate."""
    context = discover(transact)
    read = read_operation(transact, context["root"], operation)
    before = read["body"]
    if before["opcode"] == BOOL_AND:
        opcode = BOOL_OR
    elif before["opcode"] == BOOL_OR:
        opcode = BOOL_AND
    else:
        raise DemoError(f"unsupported base opcode {before['opcode']}")
    after = dict(before, opcode=opcode)
    submitted = submit_edit(
        transact, context, principal, operation, 8, read["object_id"], after, "OperationBody"
    )
    return {
        "demo": "expression",
        "before": freeze_operation(before, read["object_id"]),
        "after": freeze_operation(after, None),
        "submitted": submitted,
        "root": context["root"].hex(),
        "head_tx": context["head_tx"].hex(),
        "work": read["work"],
    }


def freeze_operation(body: Mapping[str, Any], object_id: bytes | None) -> dict[str, Any]:
    frozen: dict[str, Any] = {
        "block": body["block"],
        "ordinal": body["ordinal"],
        "opcode": body["opcode"],
        "operands": [dict(item) for item in body["operands"]],
        "result_types": [dict(item) for item in body["result_types"]],
        "immediate": dict(body["immediate"]),
    }
    if object_id is not None:
        frozen["object_id"] = object_id.hex()
    return frozen


def demo_signature_edit(
    transact: Callable[..., dict[str, Any]],
    function: bytes,
    operation: bytes,
    principal: bytes,
) -> dict[str, Any]:
    """Read a Function signature, retarget one operand by type, validate."""
    context = discover(transact)
    signature = read_signature(transact, context["root"], function)
    for item in signature["parameters"]:
        if item["value_type"] != {"variant": "Bool"}:
            raise DemoError("demo signature must be all-Bool")
    read = read_operation(transact, context["root"], operation)
    before = read["body"]
    if before["opcode"] not in (BOOL_AND, BOOL_OR):
        raise DemoError(f"unsupported base opcode {before['opcode']}")
    first = before["operands"][0]
    if first["variant"] != "Parameter":
        raise DemoError("first operand is not a parameter reference")
    current = first["value"]
    others = [
        item["entity"]
        for item in signature["parameters"]
        if item["entity"] != current and item["value_type"] == {"variant": "Bool"}
    ]
    if not others:
        raise DemoError("no same-typed parameter to retarget to")
    replacement = others[0]
    after = dict(
        before,
        operands=[{"variant": "Parameter", "value": replacement}, *before["operands"][1:]],
    )
    submitted = submit_edit(
        transact, context, principal, operation, 8, read["object_id"], after, "OperationBody"
    )
    return {
        "demo": "signature",
        "before": freeze_operation(before, read["object_id"]),
        "after": freeze_operation(after, None),
        "submitted": submitted,
        "root": context["root"].hex(),
        "head_tx": context["head_tx"].hex(),
        "work": read["work"],
        "parameter_order": [item["entity"] for item in signature["parameters"]],
        "parameter_types": [item["value_type"]["variant"] for item in signature["parameters"]],
    }
