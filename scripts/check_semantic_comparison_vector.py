#!/usr/bin/env python3
"""Independently reproduce the S20-510 semantic delta over the frozen corpus.

For every corpus pair the oracle re-derives, from the two compact root
descriptions, the change classes, the field deltas, the relation deltas, the
root sets, and the collateral set of `SEMANTIC_COMPARISON_V1.md`; decides
which functions carry a body delta from a slot-normalized inventory
projection (the restricted fingerprint bytes themselves are taken from the
corpus and checked to differ); re-encodes the delta into its canonical SCB1
bytes; and re-derives `SemanticDeltaId`. It also decodes every rejection
input strictly and reproduces its code. Run under the frozen oracle
environment (`uv run --project oracle/scb1 --frozen`) for BLAKE3.
"""

from __future__ import annotations

import json
from pathlib import Path

import blake3


ROOT = Path(__file__).resolve().parents[1]
ACCEPTED = ROOT / "conformance/semantic-comparison/v1/accepted.json"
REJECTED = ROOT / "conformance/semantic-comparison/v1/rejected.json"
DOMAIN = b"sley2.semantic-delta.v1"
MAGIC = b"SLEYSCB1"
CONTRACT_TAG = 510
MAX_DELTA_BYTES = 67_108_864
# Frozen delta schema epoch pin (S20-510 revision 2, nabu P0-3): the
# `schema_epoch_id()` of the standalone epoch-1 record stated in
# `SEMANTIC_COMPARISON_V1.md`. Stored deltas carry these bytes at
# `stored[11:43]`; the oracle compares against the pin below rather than
# trusting the artifact under test.
DELTA_SCHEMA_EPOCH = bytes.fromhex(
    "25b186d5ec4238f3f05e8af05454f62bac649143ebddf37c01c1786180b6dee4"
)

OWNERSHIP, VALUE_REFERENCE, CONTROL_FLOW, CALL, EFFECT = 1, 3, 4, 5, 6
CAPABILITY, CONTRACT, INITIALIZER, TEST_TARGET = 7, 8, 9, 10
ADDED, REMOVED, CHANGED, RETYPED, METADATA_ONLY = 1, 2, 3, 4, 5

EDGE_ROWS: dict[int, list[tuple[str, int, int | None, str]]] = {
    1: [
        ("packages", OWNERSHIP, 2, "list"),
        ("root_namespace", OWNERSHIP, 3, "one"),
        ("capability_requirements", CAPABILITY, 12, "list"),
        ("contracts", CONTRACT, 13, "list"),
        ("tests", TEST_TARGET, 14, "list"),
    ],
    2: [
        ("workspace", OWNERSHIP, 1, "one"),
        ("root_namespace", OWNERSHIP, 3, "one"),
        ("dependencies", OWNERSHIP, 18, "list"),
        ("exports", OWNERSHIP, None, "list"),
    ],
    3: [("parent", OWNERSHIP, 3, "option"), ("members", OWNERSHIP, None, "list")],
    4: [("invariants", CONTRACT, 13, "list")],
    5: [
        ("parameters", OWNERSHIP, 6, "list"),
        ("effects", EFFECT, 11, "list"),
        ("entry_block", CONTROL_FLOW, 7, "one"),
        ("blocks", OWNERSHIP, 7, "list"),
        ("contracts", CONTRACT, 13, "list"),
    ],
    7: [
        ("function", OWNERSHIP, 5, "one"),
        ("parameters", OWNERSHIP, 6, "list"),
        ("operations", OWNERSHIP, 8, "list"),
        ("return_parameter", VALUE_REFERENCE, 6, "option"),
    ],
    9: [("function_ref", CALL, 5, "option")],
    10: [("initializer", INITIALIZER, 9, "one")],
    11: [],
    12: [("effect", EFFECT, 11, "one"), ("constraint_contracts", CONTRACT, 13, "list")],
    13: [("target", CONTRACT, None, "one"), ("predicate", CONTRACT, 5, "one")],
    14: [("target", TEST_TARGET, 5, "one")],
    15: [("effects", EFFECT, 11, "list")],
    16: [("function", OWNERSHIP, 5, "one")],
    17: [("subject", OWNERSHIP, None, "one"), ("requirements", CAPABILITY, 12, "list")],
    18: [("local_namespace", OWNERSHIP, 3, "one")],
}

# (field tag, json key, shape) per kind in contract field order; "set" fields
# carry identity add/remove sets, "scalar" fields only the flag bit.
FIELD_ROWS: dict[int, list[tuple[int, str, str]]] = {
    1: [(1, "packages", "set"), (2, "root_namespace", "scalar"), (3, "capability_requirements", "set"), (4, "contracts", "set"), (5, "tests", "set")],
    2: [(1, "workspace", "scalar"), (2, "root_namespace", "scalar"), (3, "dependencies", "set"), (4, "exports", "set")],
    3: [(1, "parent", "scalar"), (2, "members", "set")],
    4: [(1, "type_parameters", "scalar"), (2, "form", "form"), (3, "invariants", "set"), (4, "visibility", "scalar")],
    5: [(1, "type_parameters", "scalar"), (2, "parameters", "parameters"), (3, "result_type", "scalar"), (4, "effects", "set"), (5, "entry_block", "scalar"), (6, "blocks", "set"), (7, "contracts", "set"), (8, "visibility", "scalar")],
    6: [(1, "owner", "scalar"), (2, "role", "scalar"), (3, "ordinal", "scalar"), (4, "value_type", "scalar")],
    7: [(1, "function", "scalar"), (2, "parameters", "set"), (3, "operations", "set"), (4, "terminator", "terminator"), (5, "reachability", "scalar")],
    8: [(1, "block", "scalar"), (2, "ordinal", "scalar"), (3, "opcode", "scalar"), (4, "operands", "scalar"), (5, "result_types", "scalar"), (6, "immediate", "scalar")],
    9: [(1, "value", "scalar")],
    10: [(1, "value_type", "scalar"), (2, "initializer", "scalar"), (3, "visibility", "scalar")],
    11: [(1, "effect_kind", "scalar"), (2, "types.0", "scalar"), (3, "types.1", "scalar"), (4, "types.2", "scalar"), (5, "types.3", "scalar"), (6, "visibility", "scalar")],
    12: [(1, "effect", "scalar"), (2, "allowed_scopes", "scalar"), (3, "constraint_contracts", "set")],
    13: [(1, "target", "scalar"), (2, "contract_kind", "scalar"), (3, "predicate", "scalar"), (4, "bindings", "scalar"), (5, "resource_limits", "scalar")],
    14: [(1, "target", "scalar"), (2, "inputs", "scalar"), (3, "environment", "scalar"), (4, "expected", "scalar"), (5, "observations", "scalar"), (6, "limits", "scalar")],
    15: [(1, "adapter_id", "scalar"), (2, "abi_version", "scalar"), (3, "types.0", "scalar"), (4, "types.1", "scalar"), (5, "types.2", "scalar"), (6, "effects", "set")],
    16: [(1, "function", "scalar"), (2, "exposure", "scalar")],
    17: [(1, "subject", "scalar"), (2, "requirements", "set")],
    18: [(1, "dependency_root", "scalar"), (2, "external_package", "scalar"), (3, "local_namespace", "scalar")],
}


# The closed section-2 grammar, derived from the emit table above: every
# (kind, field) pair the judgment emits, with the only flag bits beyond
# the presence bit the contract names (TypeDef field 2 bits 1-3, Function
# field 2 bit 1). Mirrors the Rust decoder's valid_field_grammar exactly.
FIELD_TAGS = {kind: {tag for tag, _, _ in rows} for kind, rows in FIELD_ROWS.items()}


def valid_field_grammar(kind: int, field: int, flags: int) -> bool:
    if flags & 1 == 0:
        return False
    if (kind, field) == (4, 2):
        return flags <= 15
    if (kind, field) == (5, 2):
        return flags <= 3
    return field in FIELD_TAGS.get(kind, set()) and flags == 1


class Failure(Exception):
    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def encode_uvar(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def sized(value: bytes) -> bytes:
    return encode_uvar(len(value)) + value


def encode_record(fields: list[tuple[int, bytes]]) -> bytes:
    out = encode_uvar(len(fields))
    for tag, value in fields:
        out += encode_uvar(tag) + sized(value)
    return out


def encode_list(elements: list[bytes]) -> bytes:
    out = encode_uvar(len(elements))
    for element in elements:
        out += sized(element)
    return out


def hex32(value: str) -> bytes:
    raw = bytes.fromhex(value)
    if len(raw) != 32:
        raise Failure("bad-identity")
    return raw


def lookup(entity: dict, key: str):
    if "." in key:
        base, index = key.split(".")
        return entity[base][int(index)]
    return entity.get(key)


def references(entity: dict, key: str, shape: str) -> list[str]:
    value = entity.get(key)
    if shape == "list":
        return list(value or [])
    if shape == "option":
        return [] if value is None else [value]
    return [value]


def direct_edges(entities: dict[str, dict]) -> set[tuple[str, str, int]]:
    edges: set[tuple[str, str, int]] = set()
    for entity in entities.values():
        kind = entity["kind"]
        rows = EDGE_ROWS.get(kind, [])
        if kind == 6:
            rows = [("owner", OWNERSHIP, 5 if entity["role"] == "function" else 7, "one")]
        for key, edge_kind, expected, shape in rows:
            if kind == 7 and key == "return_parameter" and entity["terminator_kind"] != "return":
                continue
            for dependency in references(entity, key, shape):
                actual = entities.get(dependency, {}).get("kind")
                if actual is None:
                    raise Failure("IMPACT_UNRESOLVED_ENTITY")
                if expected is not None and actual != expected:
                    raise Failure("IMPACT_WRONG_ENTITY_KIND")
                edges.add((entity["id"], dependency, edge_kind))
    return edges


def body_of(entity: dict) -> dict:
    return {key: value for key, value in entity.items() if key not in ("id", "object")}


def set_diff(base: list[str], target: list[str]) -> tuple[list[str], list[str]]:
    return sorted(set(target) - set(base)), sorted(set(base) - set(target))


def form_delta(base: dict, target: dict) -> tuple[int, list[str], list[str]]:
    flags = 0
    if base["kind"] != target["kind"]:
        flags |= 2
    base_members = [member[0] for member in base["members"]]
    target_members = [member[0] for member in target["members"]]
    base_set, target_set = set(base_members), set(target_members)
    shared_base = [member for member in base_members if member in target_set]
    shared_target = [member for member in target_members if member in base_set]
    if shared_base != shared_target:
        flags |= 8
    if flags & 2 == 0:
        target_by_id = {member[0]: member[1:] for member in target["members"]}
        if any(member[1:] != target_by_id[member[0]] for member in base["members"] if member[0] in target_by_id):
            flags |= 4
    return flags, sorted(target_set - base_set), sorted(base_set - target_set)


def field_deltas(base: dict, target: dict, base_entities: dict, target_entities: dict) -> list[list]:
    kind = base["kind"]
    deltas = []
    for tag, key, shape in FIELD_ROWS.get(kind, []):
        if shape == "terminator":
            base_value = (base["terminator_kind"], base["return_parameter"])
            target_value = (target["terminator_kind"], target["return_parameter"])
            if base_value != target_value:
                deltas.append([base["id"], kind, tag, 1, [], []])
            continue
        base_value, target_value = lookup(base, key), lookup(target, key)
        if base_value == target_value:
            continue
        if shape == "scalar":
            deltas.append([base["id"], kind, tag, 1, [], []])
        elif shape == "set":
            added, removed = set_diff(base_value, target_value)
            deltas.append([base["id"], kind, tag, 1, added, removed])
        elif shape == "form":
            flags, added, removed = form_delta(base_value, target_value)
            deltas.append([base["id"], kind, tag, 1 | flags, added, removed])
        elif shape == "parameters":
            base_types = [base_entities.get(p, {}).get("value_type") for p in base_value]
            target_types = [target_entities.get(p, {}).get("value_type") for p in target_value]
            added, removed = set_diff(base_value, target_value)
            deltas.append([base["id"], kind, tag, 1 | (2 if base_types != target_types else 0), added, removed])
    return deltas


def inventory_projection(function: dict, entities: dict) -> tuple:
    blocks = function["blocks"]
    slot = {block: index for index, block in enumerate(blocks)}
    parameter_slot = {parameter: index for index, parameter in enumerate(function["parameters"])}
    projected_blocks = []
    for block_id in blocks:
        block = entities[block_id]
        block_parameters = [entities[p]["value_type"] for p in block["parameters"]]
        returned = block["return_parameter"]
        projected_blocks.append(
            (
                block["reachability"],
                tuple(block_parameters),
                len(block["operations"]),
                block["terminator_kind"],
                parameter_slot.get(returned) if returned else None,
            )
        )
    return (
        function["type_parameters"],
        tuple(entities[p]["value_type"] for p in function["parameters"]),
        function["result_type"],
        tuple(function["effects"]),
        slot.get(function["entry_block"]),
        tuple(projected_blocks),
        tuple(function["contracts"]),
        function["visibility"],
    )


def transitive(edges: set[tuple[str, str, int]], seeds: set[str]) -> set[str]:
    reverse: dict[str, set[str]] = {}
    for dependent, dependency, _ in edges:
        reverse.setdefault(dependency, set()).add(dependent)
    visited = set(seeds)
    queue = list(seeds)
    while queue:
        current = queue.pop(0)
        for dependent in reverse.get(current, ()):
            if dependent not in visited:
                visited.add(dependent)
                queue.append(dependent)
    return visited


def compare(base_root: dict, target_root: dict) -> dict:
    base = {entity["id"]: entity for entity in base_root["entities"]}
    target = {entity["id"]: entity for entity in target_root["entities"]}
    if base_root["workspace_id"] != target_root["workspace_id"]:
        raise Failure("COMPARE_WORKSPACE_MISMATCH")
    if base_root["schema_epoch_id"] != target_root["schema_epoch_id"]:
        raise Failure("COMPARE_EPOCH_MISMATCH")
    base_edges = direct_edges(base)
    target_edges = direct_edges(target)
    entities = []
    fields = []
    seeds_base: set[str] = set()
    seeds_target: set[str] = set()
    delta_ids: set[str] = set()
    for entity_id in sorted(set(base) | set(target)):
        before, after = base.get(entity_id), target.get(entity_id)
        if before is None:
            entities.append([entity_id, ADDED, 0, after["kind"], None, after["object"]])
            seeds_target.add(entity_id)
        elif after is None:
            entities.append([entity_id, REMOVED, before["kind"], 0, before["object"], None])
            seeds_base.add(entity_id)
        else:
            if before["object"] == after["object"]:
                continue
            if before["kind"] != after["kind"]:
                change = RETYPED
            elif body_of(before) == body_of(after):
                change = METADATA_ONLY
            else:
                change = CHANGED
                fields.extend(field_deltas(before, after, base, target))
            if change in (CHANGED, RETYPED):
                seeds_base.add(entity_id)
                seeds_target.add(entity_id)
            entities.append([entity_id, change, before["kind"], after["kind"], before["object"], after["object"]])
        delta_ids.add(entity_id)
    fields.sort(key=lambda entry: (entry[0], entry[1], entry[2]))
    body_functions = []
    for entity_id, before in sorted(base.items()):
        after = target.get(entity_id)
        if before["kind"] != 5 or after is None or after["kind"] != 5:
            continue
        if inventory_projection(before, base) != inventory_projection(after, target):
            body_functions.append(entity_id)
            seeds_base.add(entity_id)
            seeds_target.add(entity_id)
    relations = sorted(
        [[dependent, dependency, kind, REMOVED] for dependent, dependency, kind in base_edges - target_edges]
        + [[dependent, dependency, kind, ADDED] for dependent, dependency, kind in target_edges - base_edges],
        key=lambda entry: (entry[0], entry[1], entry[2]),
    )
    reached = set()
    if seeds_base:
        reached |= transitive(base_edges, seeds_base)
    if seeds_target:
        reached |= transitive(target_edges, seeds_target)
    collateral = sorted(entity_id for entity_id in reached if entity_id not in delta_ids and entity_id in base and entity_id in target)
    roots_added, roots_removed = set_diff(base_root["dependency_roots"], target_root["dependency_roots"])
    entries_added, entries_removed = set_diff(base_root["entry_points"], target_root["entry_points"])
    return {
        "entities": entities,
        "fields": fields,
        "body_functions": body_functions,
        "relations": relations,
        "dependency_roots_added": roots_added,
        "dependency_roots_removed": roots_removed,
        "entry_points_added": entries_added,
        "entry_points_removed": entries_removed,
        "collateral": collateral,
    }


def encode_delta(delta: dict, epoch: bytes) -> tuple[bytes, str]:
    def ids(values: list[str]) -> bytes:
        return encode_list([hex32(value) for value in values])

    def object_bytes(value: str | None) -> bytes:
        return bytes(32) if value is None else hex32(value)

    entities = [
        encode_record(
            [
                (1, hex32(entry[0])),
                (2, encode_uvar(entry[1])),
                (3, encode_uvar(entry[2])),
                (4, encode_uvar(entry[3])),
                (5, object_bytes(entry[4])),
                (6, object_bytes(entry[5])),
            ]
        )
        for entry in delta["entities"]
    ]
    fields = [
        encode_record(
            [
                (1, hex32(entry[0])),
                (2, encode_uvar(entry[1])),
                (3, encode_uvar(entry[2])),
                (4, encode_uvar(entry[3])),
                (5, ids(entry[4])),
                (6, ids(entry[5])),
            ]
        )
        for entry in delta["fields"]
    ]
    bodies = [
        encode_record(
            [
                (1, hex32(entry[0])),
                (2, hex32(entry[1])),
                (3, hex32(entry[2])),
                (4, encode_uvar(entry[3])),
                (5, encode_uvar(entry[4])),
                (6, encode_uvar(entry[5])),
                (7, encode_uvar(entry[6])),
            ]
        )
        for entry in delta["bodies"]
    ]
    relations = [
        encode_record(
            [(1, hex32(entry[0])), (2, hex32(entry[1])), (3, encode_uvar(entry[2])), (4, encode_uvar(entry[3]))]
        )
        for entry in delta["relations"]
    ]
    payload = encode_record(
        [
            (1, encode_uvar(1)),
            (2, hex32(delta["workspace_id"])),
            (3, hex32(delta["root_schema_epoch"])),
            (4, hex32(delta["base_root"])),
            (5, hex32(delta["target_root"])),
            (6, encode_list(entities)),
            (7, encode_list(fields)),
            (8, encode_list(bodies)),
            (9, encode_list(relations)),
            (10, ids(delta["dependency_roots_added"])),
            (11, ids(delta["dependency_roots_removed"])),
            (12, ids(delta["entry_points_added"])),
            (13, ids(delta["entry_points_removed"])),
            (14, ids(delta["collateral"])),
        ]
    )
    preimage = MAGIC + encode_uvar(1) + encode_uvar(CONTRACT_TAG) + epoch + sized(payload)
    digest = blake3.blake3(DOMAIN + preimage).digest()
    return preimage + digest, digest.hex()


class Reader:
    def __init__(self, data: bytes):
        self.data = data
        self.offset = 0

    def take(self, count: int) -> bytes:
        if self.offset + count > len(self.data):
            raise Failure("COMPARE_FORMAT_INVALID")
        value = self.data[self.offset : self.offset + count]
        self.offset += count
        return value

    def uvar(self) -> int:
        value, shift = 0, 0
        while True:
            byte = self.take(1)[0]
            value |= (byte & 0x7F) << shift
            if not byte & 0x80:
                return value
            shift += 7
            if shift > 63:
                raise Failure("COMPARE_FORMAT_INVALID")

    def sized(self) -> bytes:
        return self.take(self.uvar())


def read_id_list(data: bytes) -> list[bytes]:
    reader = Reader(data)
    items = [reader.sized() for _ in range(reader.uvar())]
    if any(len(item) != 32 for item in items):
        raise Failure("COMPARE_FORMAT_INVALID")
    return items


def strict_decode(stored: bytes, epoch: bytes) -> None:
    if len(stored) > MAX_DELTA_BYTES:
        raise Failure("COMPARE_RESOURCE_LIMIT")
    reader = Reader(stored)
    if reader.take(8) != MAGIC:
        raise Failure("COMPARE_FORMAT_INVALID")
    if reader.uvar() != 1:
        raise Failure("COMPARE_VERSION_UNSUPPORTED")
    if reader.uvar() != CONTRACT_TAG:
        raise Failure("COMPARE_FORMAT_INVALID")
    if reader.take(32) != epoch:
        raise Failure("COMPARE_FORMAT_INVALID")
    payload = reader.sized()
    trailer = reader.take(32)
    if reader.offset != len(stored):
        raise Failure("COMPARE_FORMAT_INVALID")
    if blake3.blake3(DOMAIN + stored[:-32]).digest() != trailer:
        raise Failure("COMPARE_DIGEST_MISMATCH")
    record = Reader(payload)
    count = record.uvar()
    if count != 14:
        raise Failure("COMPARE_FORMAT_INVALID")
    fields: dict[int, bytes] = {}
    for _ in range(count):
        tag = record.uvar()
        fields[tag] = record.sized()
    entities = Reader(fields[6])
    for _ in range(entities.uvar()):
        entry = Reader(entities.sized())
        entry.uvar()
        values: dict[int, bytes] = {}
        for _ in range(6):
            tag = entry.uvar()
            values[tag] = entry.sized()
        change = Reader(values[2]).uvar()
        base_kind = Reader(values[3]).uvar()
        target_kind = Reader(values[4]).uvar()
        base_object, target_object = values[5], values[6]
        zero = bytes(32)
        ok = {
            ADDED: base_kind == 0 and base_object == zero and target_kind != 0,
            REMOVED: target_kind == 0 and target_object == zero and base_kind != 0,
            CHANGED: base_kind != 0 and base_kind == target_kind and base_object != target_object,
            METADATA_ONLY: base_kind != 0 and base_kind == target_kind and base_object != target_object,
            RETYPED: base_kind != 0 and target_kind != 0 and base_kind != target_kind,
        }.get(change, False)
        if not ok:
            raise Failure("COMPARE_FORMAT_INVALID")
    # Section 2: the closed field grammar (contract section 2) with
    # disjoint added/removed sets, mirroring the Rust decoder.
    field_reader = Reader(fields[7])
    for _ in range(field_reader.uvar()):
        entry = Reader(field_reader.sized())
        entry.uvar()
        fvalues: dict[int, bytes] = {}
        for _ in range(6):
            tag = entry.uvar()
            fvalues[tag] = entry.sized()
        kind = Reader(fvalues[2]).uvar()
        field = Reader(fvalues[3]).uvar()
        flags = Reader(fvalues[4]).uvar()
        if not valid_field_grammar(kind, field, flags):
            raise Failure("COMPARE_FORMAT_INVALID")
        added = read_id_list(fvalues[5])
        removed = read_id_list(fvalues[6])
        if set(added) & set(removed):
            raise Failure("COMPARE_FORMAT_INVALID")
    # Payload field 5: equal roots admit only the empty delta. Decode
    # authenticates nothing else; authority comes only from re-derivation.
    if fields[4] == fields[5]:
        for tag in (6, 7, 8, 9, 10, 11, 12, 13, 14):
            if fields[tag] != b"\x00":
                raise Failure("COMPARE_FORMAT_INVALID")
    # Added and removed root sets are disjoint by construction.
    if set(read_id_list(fields[10])) & set(read_id_list(fields[11])):
        raise Failure("COMPARE_FORMAT_INVALID")
    if set(read_id_list(fields[12])) & set(read_id_list(fields[13])):
        raise Failure("COMPARE_FORMAT_INVALID")


def main() -> int:
    accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    problems: list[str] = []
    if accepted.get("contract") != "sley2-semantic-comparison-v1":
        problems.append("accepted-contract")
    epochs = set()
    for vector in accepted.get("vectors", []):
        name = vector["id"]
        stored = bytes.fromhex(vector["stored_hex"])
        # MAGIC (8) + uvar(1) (1) + uvar(510) (2) precede the 32-byte epoch,
        # which must equal the frozen pin, never the artifact's own word.
        if stored[11:43] != DELTA_SCHEMA_EPOCH:
            problems.append(f"{name}:delta-epoch-unpinned")
        epochs.add(stored[11:43])
        expected = vector["delta"]
        try:
            actual = compare(vector["base"], vector["target"])
        except Failure as failure:
            problems.append(f"{name}:unexpected-failure:{failure.code}")
            continue
        for key in ("entities", "fields", "relations", "dependency_roots_added", "dependency_roots_removed", "entry_points_added", "entry_points_removed", "collateral"):
            if actual[key] != expected[key]:
                problems.append(f"{name}:{key}")
        if [entry[0] for entry in expected["bodies"]] != actual["body_functions"]:
            problems.append(f"{name}:bodies")
        for entry in expected["bodies"]:
            if entry[1] == entry[2] or len(bytes.fromhex(entry[1])) != 32:
                problems.append(f"{name}:body-fingerprints")
        encoded, digest = encode_delta(expected, DELTA_SCHEMA_EPOCH)
        if encoded != stored:
            problems.append(f"{name}:stored-bytes")
        if digest != vector["semantic_delta_id"]:
            problems.append(f"{name}:semantic-delta-id")
        try:
            strict_decode(stored, DELTA_SCHEMA_EPOCH)
        except Failure as failure:
            problems.append(f"{name}:strict-decode:{failure.code}")
    if len(epochs) != 1 or next(iter(epochs)) != DELTA_SCHEMA_EPOCH:
        problems.append("delta-epoch-not-uniform")
    epoch = DELTA_SCHEMA_EPOCH
    for mutation in rejected.get("mutations", []):
        try:
            strict_decode(bytes.fromhex(mutation["input_hex"]), epoch)
        except Failure as failure:
            if failure.code != mutation["expected_code"]:
                problems.append(f"{mutation['id']}:code:{failure.code}")
        else:
            problems.append(f"{mutation['id']}:accepted")
    print(
        json.dumps(
            {
                "contract": "s20-510-semantic-comparison-oracle-v1",
                "mutations": len(rejected.get("mutations", [])),
                "problems": problems,
                "result": "PASS" if not problems else "FAIL",
                "vectors": len(accepted.get("vectors", [])),
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())
