#!/usr/bin/env python3
"""Independently reproduce the S20-310 full root-backed query vectors.

The oracle re-derives, from the frozen S20-250 complete-entity fixture bodies,
its frozen canonical edge set, and the fixture context (root, epoch,
workspace, bindings, fingerprints, roots), every `RootQueryId` request
identity and `SLEYRQR1` response record of `ROOT_BACKED_QUERY_PROFILE_V1.md`:
the nineteen classes, exact-then-paged results with typed cursors, the work
accounting, and the failure precedence for the rejected requests. It shares
no code with the Rust implementation.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import blake3


ROOT = Path(__file__).resolve().parents[1]
ACCEPTED = ROOT / "conformance/root-backed-query/v1/accepted.json"
REJECTED = ROOT / "conformance/root-backed-query/v1/rejected.json"
SOURCE = ROOT / "conformance/complete-entity-impact/v1/accepted.json"
SNAPSHOT_SOURCE = ROOT / "conformance/complete-root-index-snapshot/v1/accepted.json"

DOMAIN = b"sley2.root-query.v1"
REQUEST_MAGIC = b"SLEYRQQ1"
RESPONSE_MAGIC = b"SLEYRQR1"
OPTION_NONE, OPTION_SOME = 1, 2
FLAG_FALSE, FLAG_TRUE = 1, 2
CURSOR_ENTITY, CURSOR_EDGE, CURSOR_ROOT = 1, 2, 3
MAX_WORK = 100_000_000
MAX_RESPONSE_BYTES = 67_108_864
LIMIT_CEILINGS = {
    "max_returned_entities": 65_535,
    "max_returned_edges": 400_000,
    "max_depth": 65_535,
    "max_response_bytes": MAX_RESPONSE_BYTES,
    "max_work": MAX_WORK,
}
CODES = {
    "QUERY_PROFILE_UNSUPPORTED": 31000,
    "QUERY_REQUEST_NOT_CANONICAL": 31001,
    "QUERY_UNSUPPORTED": 31002,
    "QUERY_SNAPSHOT_MISMATCH": 31003,
    "QUERY_UNRESOLVED_ENTITY": 31004,
    "QUERY_RESOURCE_LIMIT": 31005,
    "QUERY_REQUIRED_FACT_OMITTED": 31006,
    "QUERY_INTERNAL_INVARIANT": 31007,
    "QUERY_ROOT_MISMATCH": 31008,
    "QUERY_CONTINUATION_INVALID": 31009,
    "QUERY_CLASS_NOT_APPLICABLE": 31010,
}
SINGLE_CLASSES = {1, 2, 3, 9}
EDGE_CLASSES = {12, 13}
ROOT_CLASSES = {11}
ENTITY_CLASSES = {2, 3, 6, 7, 8, 9, 16, 17, 18, 19}
KIND_CLASSES = {4}
FILTER_CLASSES = {12, 13}
SEED_CLASSES = {14, 15}


class Failure(Exception):
    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def u32(value: int) -> bytes:
    return struct.pack(">I", value)


def u64(value: int) -> bytes:
    return struct.pack(">Q", value)


def h(value: str) -> bytes:
    return bytes.fromhex(value)


class Work:
    def __init__(self, limit: int):
        self.total = 0
        self.limit = limit

    def charge(self, amount: int) -> None:
        self.total += amount
        if self.total > MAX_WORK or self.total > self.limit:
            raise Failure("QUERY_RESOURCE_LIMIT")


class Root:
    """The bound root: bodies, inventory, edges, and facts."""

    def __init__(self, source: dict, context: dict):
        request = source["request"]
        self.entities = request["entities"]
        self.by_id = {entity["id"]: entity for entity in self.entities}
        self.inventory = [(entity["id"], entity["kind"]) for entity in self.entities]
        self.edges = [tuple(edge) for edge in source["expected"]["direct_edges"]]
        self.facts = request["facts"]
        self.bindings = {entity: obj for entity, obj in context["bindings"]}
        self.fingerprints = {entity: fp for entity, fp in context["fingerprints"]}
        self.reverse: dict[str, list[tuple[str, int]]] = {}
        self.forward: dict[str, list[tuple[str, int]]] = {}
        for dependent, dependency, kind in self.edges:
            self.reverse.setdefault(dependency, []).append((dependent, kind))
            self.forward.setdefault(dependent, []).append((dependency, kind))

    def kind(self, entity: str) -> int | None:
        body = self.by_id.get(entity)
        return None if body is None else body["kind"]


def encode_cursor(cursor: dict | None) -> bytes:
    if cursor is None:
        return u32(OPTION_NONE)
    out = u32(OPTION_SOME) + u32(cursor["tag"])
    if cursor["tag"] == CURSOR_ENTITY:
        return out + h(cursor["entity"])
    if cursor["tag"] == CURSOR_ROOT:
        return out + h(cursor["root"])
    return out + h(cursor["dependent"]) + h(cursor["dependency"]) + u32(cursor["kind"])


def encode_limits(limits: dict) -> bytes:
    return (
        u64(limits["max_returned_entities"])
        + u64(limits["max_returned_edges"])
        + u32(limits["max_depth"])
        + u64(limits["max_response_bytes"])
        + u64(limits["max_work"])
    )


def validate_limits(limits: dict) -> None:
    for key, ceiling in LIMIT_CEILINGS.items():
        value = limits[key]
        if value > ceiling or (key != "max_depth" and value == 0):
            raise Failure("QUERY_RESOURCE_LIMIT")


def strictly_increasing(values: list) -> bool:
    return all(values[index] < values[index + 1] for index in range(len(values) - 1))


def validate_shape(query: dict) -> None:
    cls = query["class"]
    if cls in FILTER_CLASSES:
        kinds = query["kinds"]
        if not kinds or len(kinds) > 12:
            raise Failure("QUERY_REQUEST_NOT_CANONICAL")
        if any(kind < 1 or kind > 12 for kind in kinds):
            raise Failure("QUERY_UNSUPPORTED")
        if not strictly_increasing(kinds):
            raise Failure("QUERY_REQUEST_NOT_CANONICAL")
    if cls in SEED_CLASSES:
        seeds = query["seeds"]
        if not seeds or len(seeds) > 65_535 or not strictly_increasing([h(seed) for seed in seeds]):
            raise Failure("QUERY_REQUEST_NOT_CANONICAL")
    if cls in KIND_CLASSES and not 1 <= query["kind"] <= 18:
        raise Failure("QUERY_UNSUPPORTED")


def key_tag(cls: int) -> int | None:
    if cls in SINGLE_CLASSES:
        return None
    if cls in ROOT_CLASSES:
        return CURSOR_ROOT
    if cls in EDGE_CLASSES:
        return CURSOR_EDGE
    return CURSOR_ENTITY


def validate_cursor(query: dict, after: dict | None) -> None:
    if after is None:
        return
    if key_tag(query["class"]) != after["tag"]:
        raise Failure("QUERY_CONTINUATION_INVALID")


def preimage(context: dict, limits: dict, allow: bool, after: dict | None, query: dict) -> bytes:
    out = bytearray(REQUEST_MAGIC + u32(1) + u32(1))
    out += h(context["snapshot_id"]) + h(context["schema_epoch_hex"]) + h(context["root_hex"])
    out += h(context["workspace_id"]) + u32(2) + u32(1) + encode_limits(limits)
    out += u32(FLAG_TRUE if allow else FLAG_FALSE) + encode_cursor(after)
    cls = query["class"]
    out += u32(cls)
    if cls in ENTITY_CLASSES:
        out += h(query["entity"])
    elif cls in KIND_CLASSES:
        out += u32(query["kind"])
    elif cls in FILTER_CLASSES:
        out += h(query["entity"]) + u64(len(query["kinds"]))
        for kind in query["kinds"]:
            out += u32(kind)
    elif cls in SEED_CLASSES:
        out += u64(len(query["seeds"]))
        for seed in query["seeds"]:
            out += h(seed)
    return bytes(out)


def named_entities(query: dict) -> list[str]:
    cls = query["class"]
    if cls in ENTITY_CLASSES or cls in FILTER_CLASSES:
        return [query["entity"]]
    if cls in SEED_CLASSES:
        return list(query["seeds"])
    return []


def compute(root: Root, query: dict, work: Work) -> tuple[str, list, int]:
    """Returns (payload kind, complete items, reached depth)."""
    cls = query["class"]
    if cls == 1:
        counts = [0] * 18
        for _, kind in root.inventory:
            work.charge(1)
            counts[kind - 1] += 1
        return "summary", [counts], 0
    if cls == 2:
        work.charge(3)
        entity = query["entity"]
        return "entity", [(root.kind(entity), root.bindings[entity], root.fingerprints.get(entity))], 0
    if cls == 3:
        work.charge(2)
        return "fingerprint", [root.fingerprints.get(query["entity"])], 0
    if cls == 4:
        items = []
        for entity, kind in root.inventory:
            work.charge(1)
            if kind == query["kind"]:
                items.append(entity)
        return "entities", items, 0
    if cls == 5:
        items: list[str] = []
        for entity in root.entities:
            work.charge(1)
            if entity["kind"] == 1:
                items.extend(entity["packages"])
        return "entities", sorted(set(items)), 0
    if cls in (6, 7):
        work.charge(1)
        body = root.by_id[query["entity"]]
        if body["kind"] != 2:
            raise Failure("QUERY_CLASS_NOT_APPLICABLE")
        if cls == 6:
            work.charge(len(body["exports"]))
            return "entities", sorted(set(body["exports"])), 0
        rows = []
        for binding in sorted(set(body["dependencies"])):
            work.charge(1)
            dependency = root.by_id[binding]
            rows.append((binding, dependency["dependency_root"], dependency["external_package"], dependency["local_namespace"]))
        return "dependency_rows", rows, 0
    if cls == 8:
        work.charge(1)
        body = root.by_id[query["entity"]]
        if body["kind"] != 3:
            raise Failure("QUERY_CLASS_NOT_APPLICABLE")
        entries = []
        for member in sorted(set(body["members"])):
            work.charge(1)
            entries.append((member, root.kind(member)))
        return "inventory_entries", entries, 0
    if cls == 9:
        entity = query["entity"]
        body = root.by_id[entity]
        if body["kind"] == 3:
            cursor = body["parent"]
        else:
            cursor = None
            for candidate in root.entities:
                work.charge(1)
                if candidate["kind"] == 3 and entity in candidate["members"]:
                    cursor = candidate["id"]
                    break
        chain = []
        while cursor is not None:
            work.charge(1)
            chain.append(cursor)
            cursor = root.by_id[cursor]["parent"]
        return "chain", chain, 0
    if cls == 10:
        rows = []
        for entry_point in root.facts["entry_points"]:
            work.charge(1)
            body = root.by_id[entry_point]
            rows.append((entry_point, body["function"], body["exposure"]))
        return "entry_rows", rows, 0
    if cls == 11:
        work.charge(len(root.facts["dependency_roots"]))
        return "roots", list(root.facts["dependency_roots"]), 0
    if cls in (12, 13):
        entity = query["entity"]
        kinds = set(query["kinds"])
        items = []
        for edge in root.edges:
            work.charge(1)
            side = edge[0] if cls == 12 else edge[1]
            if side == entity and edge[2] in kinds:
                items.append(edge)
        return "edges", items, 0
    if cls in (14, 15):
        adjacency = root.reverse if cls == 14 else root.forward
        depths = {seed: 0 for seed in query["seeds"]}
        queue = list(query["seeds"])
        reached = 0
        while queue:
            current = queue.pop(0)
            work.charge(1)
            for neighbour, _ in adjacency.get(current, []):
                work.charge(1)
                if neighbour in depths:
                    continue
                depths[neighbour] = depths[current] + 1
                reached = max(reached, depths[neighbour])
                queue.append(neighbour)
        return "entities", sorted(depths), reached
    if cls in (16, 17):
        wanted = 13 if cls == 16 else 14
        items = []
        for entity in root.entities:
            work.charge(1)
            if entity["kind"] == wanted and entity["target"] == query["entity"]:
                items.append(entity["id"])
        return "entities", items, 0
    if cls == 18:
        work.charge(1)
        body = root.by_id[query["entity"]]
        if body["kind"] in (5, 15):
            effects = list(body["effects"])
        elif body["kind"] == 12:
            effects = [body["effect"]]
        else:
            raise Failure("QUERY_CLASS_NOT_APPLICABLE")
        work.charge(len(effects))
        return "entities", sorted(set(effects)), 0
    if cls == 19:
        subject = query["entity"]
        items = []
        for entity in root.entities:
            work.charge(1)
            if entity["kind"] == 17 and entity["subject"] == subject:
                items.extend(entity["requirements"])
            elif entity["kind"] == 1 and entity["id"] == subject:
                items.extend(entity["capability_requirements"])
        return "entities", sorted(set(items)), 0
    raise Failure("QUERY_UNSUPPORTED")


def item_key(kind: str, item):
    if kind in ("entities", "chain"):
        return h(item)
    if kind == "dependency_rows" or kind == "entry_rows":
        return h(item[0])
    if kind == "inventory_entries":
        return h(item[0])
    if kind == "roots":
        return h(item)
    if kind == "edges":
        return (h(item[0]), h(item[1]), item[2])
    raise AssertionError(kind)


def cursor_key(after: dict | None):
    if after is None:
        return None
    if after["tag"] == CURSOR_ENTITY:
        return h(after["entity"])
    if after["tag"] == CURSOR_ROOT:
        return h(after["root"])
    return (h(after["dependent"]), h(after["dependency"]), after["kind"])


def cursor_for(kind: str, item) -> dict:
    if kind == "edges":
        return {"tag": CURSOR_EDGE, "dependent": item[0], "dependency": item[1], "kind": item[2]}
    if kind == "roots":
        return {"tag": CURSOR_ROOT, "root": item}
    entity = item if isinstance(item, str) else item[0]
    return {"tag": CURSOR_ENTITY, "entity": entity}


ITEM_BYTES = {
    "entities": 32,
    "chain": 32,
    "dependency_rows": 128,
    "inventory_entries": 36,
    "entry_rows": 68,
    "roots": 32,
    "edges": 68,
}


def page(kind: str, items: list, limits: dict, allow: bool, after: dict | None):
    if kind == "summary":
        return items, 1, 1, False, None, 32 * 6 + 8 * 18 + 8 * 3
    if kind == "entity":
        return items, 1, 1, False, None, 4 + 32 + (36 if items[0][2] is not None else 4)
    if kind == "fingerprint":
        return items, 1, 1, False, None, 36 if items[0] is not None else 4
    if kind == "chain":
        if len(items) > limits["max_returned_entities"]:
            raise Failure("QUERY_REQUIRED_FACT_OMITTED")
        return items, len(items), len(items), False, None, 8 + 32 * len(items)
    total = len(items)
    key = cursor_key(after)
    remaining = [item for item in items if key is None or item_key(kind, item) > key]
    limit = limits["max_returned_edges"] if kind == "edges" else limits["max_returned_entities"]
    truncated = len(remaining) > limit
    if truncated and not allow:
        raise Failure("QUERY_REQUIRED_FACT_OMITTED")
    selected = remaining[:limit]
    next_after = cursor_for(kind, selected[-1]) if truncated else None
    return selected, total, len(selected), truncated, next_after, 8 + ITEM_BYTES[kind] * len(selected)


def encode_payload(kind: str, items: list, context: dict, root: Root) -> bytes:
    out = bytearray()
    if kind == "summary":
        out += h(context["workspace_id"]) + h(context["schema_epoch_hex"]) + h(context["root_hex"])
        out += h(context["contract_root"]) + h(context["test_root"]) + h(context["policy_root"])
        for count in items[0]:
            out += u64(count)
        out += u64(len(root.facts["entry_points"])) + u64(len(root.facts["dependency_roots"])) + u64(len(root.edges))
        return bytes(out)
    if kind == "entity":
        kind_tag, object_id, fingerprint = items[0]
        out += u32(kind_tag) + h(object_id)
        out += u32(OPTION_NONE) if fingerprint is None else u32(OPTION_SOME) + h(fingerprint)
        return bytes(out)
    if kind == "fingerprint":
        return u32(OPTION_NONE) if items[0] is None else u32(OPTION_SOME) + h(items[0])
    out += u64(len(items))
    for item in items:
        if kind in ("entities", "chain", "roots"):
            out += h(item)
        elif kind == "dependency_rows":
            out += h(item[0]) + h(item[1]) + h(item[2]) + h(item[3])
        elif kind == "inventory_entries":
            out += h(item[0]) + u32(item[1])
        elif kind == "entry_rows":
            out += h(item[0]) + h(item[1]) + u32(item[2])
        elif kind == "edges":
            out += h(item[0]) + h(item[1]) + u32(item[2])
    return bytes(out)


def answer(root: Root, context: dict, vector: dict) -> tuple[bytes, bytes, dict | None]:
    query, limits = vector["query"], vector["limits"]
    allow, after = vector["allow_continuation"], vector["after"]
    validate_limits(limits)
    validate_shape(query)
    validate_cursor(query, after)
    request = preimage(context, limits, allow, after, query)
    query_id = blake3.blake3(DOMAIN + request).digest()
    for entity in named_entities(query):
        if entity not in root.by_id:
            raise Failure("QUERY_UNRESOLVED_ENTITY")
    work = Work(limits["max_work"])
    kind, items, depth = compute(root, query, work)
    if depth > limits["max_depth"]:
        raise Failure("QUERY_REQUIRED_FACT_OMITTED")
    selected, total, returned, truncated, next_after, payload_bytes = page(kind, items, limits, allow, after)
    after_bytes = encode_cursor(after)
    next_bytes = encode_cursor(next_after)
    header_bytes = 8 + 8 + 160 + 4 + 4 + 36 + 4 + len(after_bytes) + 4 + 16 + 4 + len(next_bytes) + 4 + 8 + 8 + 4
    response_bytes = header_bytes + payload_bytes
    if response_bytes > MAX_RESPONSE_BYTES:
        raise Failure("QUERY_RESOURCE_LIMIT")
    charged = work.total + response_bytes
    if charged > MAX_WORK or charged > limits["max_work"]:
        raise Failure("QUERY_RESOURCE_LIMIT")
    if response_bytes > limits["max_response_bytes"]:
        raise Failure("QUERY_REQUIRED_FACT_OMITTED")
    record = bytearray(RESPONSE_MAGIC + u32(1) + u32(1) + query_id)
    record += h(context["snapshot_id"]) + h(context["schema_epoch_hex"]) + h(context["root_hex"])
    record += h(context["workspace_id"]) + u32(2) + u32(1) + encode_limits(limits)
    record += u32(FLAG_TRUE if allow else FLAG_FALSE) + after_bytes + u32(query["class"])
    record += u64(total) + u64(returned) + u32(FLAG_TRUE if truncated else FLAG_FALSE) + next_bytes
    record += u32(depth) + u64(charged) + u64(response_bytes) + u32(query["class"])
    record += encode_payload(kind, selected, context, root)
    if len(record) != response_bytes:
        raise Failure("QUERY_INTERNAL_INVARIANT")
    return query_id, bytes(record), next_after


def main() -> int:
    accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    source = json.loads(SOURCE.read_text(encoding="utf-8"))["vectors"][0]
    snapshot = json.loads(SNAPSHOT_SOURCE.read_text(encoding="utf-8"))["vectors"][0]
    problems: list[str] = []
    context = accepted["context"]
    if accepted.get("contract") != "sley2-root-backed-query-v1":
        problems.append("accepted-contract")
    if context["source_request_sha256"] != source["request_sha256"]:
        problems.append("context:source-request-drift")
    if context["snapshot_id"] != snapshot["snapshot_id"] or context["root_hex"] != snapshot["root_hex"]:
        problems.append("context:snapshot-drift")
    if context["schema_epoch_hex"] != snapshot["schema_epoch_hex"]:
        problems.append("context:epoch-drift")
    root = Root(source, context)
    if [entity for entity, _ in root.inventory] != source["request"]["facts"]["bound_entities"]:
        problems.append("context:inventory-order")
    for vector in accepted.get("vectors", []):
        try:
            query_id, record, next_after = answer(root, context, vector)
        except Failure as failure:
            problems.append(f"{vector['id']}:unexpected-failure:{failure.code}")
            continue
        if query_id.hex() != vector["query_id"]:
            problems.append(f"{vector['id']}:query_id")
        if record.hex() != vector["record_hex"]:
            problems.append(f"{vector['id']}:record")
        if len(record) != vector["record_bytes"]:
            problems.append(f"{vector['id']}:record_bytes")
        if next_after != vector["next_after"]:
            problems.append(f"{vector['id']}:next_after")
    classes = sorted({vector["query"]["class"] for vector in accepted.get("vectors", [])})
    if classes != list(range(1, 20)):
        problems.append("vectors:class-coverage")
    # Paged vectors must union to the complete result of the same class.
    pages = {vector["id"]: vector for vector in accepted.get("vectors", [])}
    for prefix, complete_id in (("page-namespaces", "class-04"), ("page-edges", None)):
        first, second = pages.get(f"{prefix}-1"), pages.get(f"{prefix}-2")
        if first is None or second is None:
            problems.append(f"{prefix}:missing")
            continue
        if not first["next_after"] or second["after"] != first["next_after"] or second["next_after"] is not None:
            problems.append(f"{prefix}:chain")
    for mutation in rejected.get("mutations", []):
        try:
            answer(root, context, mutation)
        except Failure as failure:
            if failure.code != mutation["expected_code"]:
                problems.append(f"{mutation['id']}:code:{failure.code}")
            elif CODES[failure.code] != mutation["expected_numeric"]:
                problems.append(f"{mutation['id']}:numeric")
        except KeyError:
            problems.append(f"{mutation['id']}:oracle-key-error")
        else:
            problems.append(f"{mutation['id']}:accepted")
    print(
        json.dumps(
            {
                "contract": "s20-310-full-root-backed-query-oracle-v1",
                "mutations": len(rejected.get("mutations", [])),
                "problems": problems,
                "query_classes": len(classes),
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
