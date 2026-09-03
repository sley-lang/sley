#!/usr/bin/env python3
"""Independently reproduce the S20-520 merge judgment over the frozen corpus.

For every corpus case the oracle recomputes both S20-510 deltas from the
compact root descriptions (reusing the comparison oracle), applies judgment
rules J1 through J8, set-valued field composition, and the non-ownership
collateral rule of `MERGE_V1.md`, and checks the outcome: for a merged case
the merged entity bodies (reused objects by identity, composed bodies by
value) and the metadata-overridden set; for a conflict the entries, the
canonical conflict bytes, and `MergeConflictId`. It also decodes every
rejection input strictly. Run under the frozen oracle environment
(`uv run --project oracle/scb1 --frozen`) for BLAKE3.
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import blake3


ROOT = Path(__file__).resolve().parents[1]
ACCEPTED = ROOT / "conformance/merge/v1/accepted.json"
REJECTED = ROOT / "conformance/merge/v1/rejected.json"
DOMAIN = b"sley2.merge-conflict.v1"
MAGIC = b"SLEYSCB1"
CONTRACT_TAG = 520
OWNERSHIP = 1
ADDED, REMOVED, CHANGED, RETYPED, METADATA_ONLY = 1, 2, 3, 4, 5
REASON = {
    "AddAdd": 1,
    "DeleteEdit": 2,
    "FieldEdit": 3,
    "KindEdit": 4,
    "Collateral": 5,
    "Closure": 6,
    "RootAnchor": 7,
    "PolicyRoot": 8,
    "MetadataEdit": 9,
}
SET_FIELDS = {(1, 1), (1, 3), (1, 4), (1, 5), (2, 3), (2, 4), (3, 2), (4, 3), (5, 4), (5, 7), (12, 3), (15, 6), (17, 2)}
FIELD_KEYS = {
    1: {1: "packages", 2: "root_namespace", 3: "capability_requirements", 4: "contracts", 5: "tests"},
    2: {1: "workspace", 2: "root_namespace", 3: "dependencies", 4: "exports"},
    3: {1: "parent", 2: "members"},
    4: {1: "type_parameters", 2: "form", 3: "invariants", 4: "visibility"},
    5: {1: "type_parameters", 2: "parameters", 3: "result_type", 4: "effects", 5: "entry_block", 6: "blocks", 7: "contracts", 8: "visibility"},
    6: {1: "owner", 2: "role", 3: "ordinal", 4: "value_type"},
    7: {1: "function", 2: "parameters", 3: "operations", 4: "terminator", 5: "reachability"},
    9: {1: "value"},
    10: {1: "value_type", 2: "initializer", 3: "visibility"},
    11: {1: "effect_kind", 2: "types.0", 3: "types.1", 4: "types.2", 5: "types.3", 6: "visibility"},
    12: {1: "effect", 2: "allowed_scopes", 3: "constraint_contracts"},
    13: {1: "target", 2: "contract_kind", 3: "predicate", 4: "bindings", 5: "resource_limits"},
    14: {1: "target", 2: "inputs", 3: "environment", 4: "expected", 5: "observations", 6: "limits"},
    15: {1: "adapter_id", 2: "abi_version", 3: "types.0", 4: "types.1", 5: "types.2", 6: "effects"},
    16: {1: "function", 2: "exposure"},
    17: {1: "subject", 2: "requirements"},
    18: {1: "dependency_root", 2: "external_package", 3: "local_namespace"},
}

spec = importlib.util.spec_from_file_location("comparison", ROOT / "scripts/check_semantic_comparison_vector.py")
comparison = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(comparison)


class Failure(Exception):
    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def body_of(entity: dict) -> dict:
    return {key: value for key, value in entity.items() if key not in ("id", "object")}


def set_field(body: dict, key: str, values: list[str]) -> dict:
    out = dict(body)
    out[key] = sorted(values)
    return out


def copy_field(dst: dict, src: dict, key: str) -> dict:
    out = dict(dst)
    if "." in key:
        base, index = key.split(".")
        types = list(out[base])
        types[int(index)] = src[base][int(index)]
        out[base] = types
    elif key == "terminator":
        out["terminator_kind"] = src["terminator_kind"]
        out["return_parameter"] = src["return_parameter"]
    else:
        out[key] = src[key]
    return out


def non_ownership_dependents(edges: set[tuple[str, str, int]], start: str) -> set[str]:
    reached: set[str] = set()
    queue = [start]
    while queue:
        current = queue.pop()
        for dependent, dependency, kind in edges:
            if dependency == current and kind != OWNERSHIP and dependent not in reached:
                reached.add(dependent)
                queue.append(dependent)
    return reached


def judge(ancestor: dict, ours: dict, theirs: dict) -> dict:
    base = {entity["id"]: entity for entity in ancestor["entities"]}
    a = {entity["id"]: entity for entity in ours["entities"]}
    b = {entity["id"]: entity for entity in theirs["entities"]}
    delta_a = comparison.compare(ancestor, ours)
    delta_b = comparison.compare(ancestor, theirs)
    entries_a = {entry[0]: entry for entry in delta_a["entities"]}
    entries_b = {entry[0]: entry for entry in delta_b["entities"]}
    fields_a: dict[str, dict[int, list]] = {}
    for entry in delta_a["fields"]:
        fields_a.setdefault(entry[0], {})[entry[2]] = entry
    fields_b: dict[str, dict[int, list]] = {}
    for entry in delta_b["fields"]:
        fields_b.setdefault(entry[0], {})[entry[2]] = entry
    conflicts: list[tuple] = []
    merged: dict[str, dict] = {entity_id: {"body": body_of(e), "object": e["object"], "composed": False} for entity_id, e in base.items()}
    overridden: list[str] = []
    touched_a: set[str] = set()
    touched_b: set[str] = set()

    def take(entity_id: str, side: dict) -> None:
        merged[entity_id] = {"body": body_of(side[entity_id]), "object": side[entity_id]["object"], "composed": False}

    for entity_id in sorted(set(entries_a) | set(entries_b)):
        ea, eb = entries_a.get(entity_id), entries_b.get(entity_id)
        kind = (base.get(entity_id) or a.get(entity_id) or b.get(entity_id))["kind"]
        ours_object = a.get(entity_id, {}).get("object")
        theirs_object = b.get(entity_id, {}).get("object")

        def conflict(reason: str, field: int = 0) -> None:
            conflicts.append((entity_id, REASON[reason], kind, field, ours_object, theirs_object, 0))

        if ea is not None and ea[1] in (CHANGED, RETYPED, REMOVED):
            touched_a.add(entity_id)
        if eb is not None and eb[1] in (CHANGED, RETYPED, REMOVED):
            touched_b.add(entity_id)
        if ea is not None and eb is None:
            if ea[1] == REMOVED:
                merged.pop(entity_id, None)
            else:
                take(entity_id, a)
            continue
        if ea is None and eb is not None:
            if eb[1] == REMOVED:
                merged.pop(entity_id, None)
            else:
                take(entity_id, b)
            continue
        assert ea is not None and eb is not None
        if ea[1] == eb[1] and ea[5] == eb[5]:
            if ea[1] == REMOVED:
                merged.pop(entity_id, None)
            else:
                take(entity_id, a)
            continue
        if REMOVED in (ea[1], eb[1]):
            conflict("DeleteEdit")
        elif ADDED in (ea[1], eb[1]):
            conflict("AddAdd")
        elif RETYPED in (ea[1], eb[1]):
            conflict("KindEdit")
        elif ea[1] == METADATA_ONLY and eb[1] == CHANGED:
            overridden.append(entity_id)
            take(entity_id, b)
        elif eb[1] == METADATA_ONLY and ea[1] == CHANGED:
            overridden.append(entity_id)
            take(entity_id, a)
        elif ea[1] == METADATA_ONLY and eb[1] == METADATA_ONLY:
            conflict("MetadataEdit")
        else:
            body = body_of(base[entity_id])
            failed = None
            fa, fb = fields_a.get(entity_id, {}), fields_b.get(entity_id, {})
            for field in sorted(set(fa) | set(fb)):
                key = FIELD_KEYS[kind][field]
                if field in fa and field not in fb:
                    body = copy_field(body, body_of(a[entity_id]), key)
                elif field in fb and field not in fa:
                    body = copy_field(body, body_of(b[entity_id]), key)
                elif (kind, field) in SET_FIELDS:
                    a_added, a_removed = set(fa[field][4]), set(fa[field][5])
                    b_added, b_removed = set(fb[field][4]), set(fb[field][5])
                    if a_added & b_removed or a_removed & b_added:
                        failed = field
                        break
                    composed = (set(base[entity_id][key]) | a_added | b_added) - a_removed - b_removed
                    body = set_field(body, key, sorted(composed))
                elif comparison.lookup(body_of(a[entity_id]), key) == comparison.lookup(body_of(b[entity_id]), key):
                    body = copy_field(body, body_of(a[entity_id]), key)
                else:
                    failed = field
                    break
            if failed is not None:
                conflict("FieldEdit", failed)
            else:
                merged[entity_id] = {"body": body, "object": None, "composed": True}

    edges_a = comparison.direct_edges(a)
    edges_b = comparison.direct_edges(b)
    for this_touched, other_entries, other_edges in ((touched_a, entries_b, edges_b), (touched_b, entries_a, edges_a)):
        for entity_id in sorted(this_touched):
            dependents = non_ownership_dependents(other_edges, entity_id)
            if any(other[1] in (ADDED, CHANGED) and other[0] in dependents for other in other_entries.values()):
                conflicts.append(
                    (entity_id, REASON["Collateral"], base.get(entity_id, {}).get("kind", 0), 0, a.get(entity_id, {}).get("object"), b.get(entity_id, {}).get("object"), 0)
                )
    conflicts = sorted(set(conflicts), key=lambda entry: (entry[0], entry[1], entry[3]))
    return {"conflicts": conflicts, "merged": merged, "metadata_overridden": sorted(overridden)}


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


def encode_conflict(conflict: dict, epoch: bytes) -> tuple[bytes, str]:
    def obj(value: str | None) -> bytes:
        return bytes(32) if value is None else bytes.fromhex(value)

    entries = [
        encode_record(
            [
                (1, bytes.fromhex(entry[0])),
                (2, encode_uvar(entry[1])),
                (3, encode_uvar(entry[2])),
                (4, encode_uvar(entry[3])),
                (5, obj(entry[4])),
                (6, obj(entry[5])),
                (7, encode_uvar(entry[6])),
            ]
        )
        for entry in conflict["conflicts"]
    ]
    payload = encode_record(
        [
            (1, encode_uvar(1)),
            (2, bytes.fromhex(conflict["workspace_id"])),
            (3, bytes.fromhex(conflict["ancestor_root"])),
            (4, bytes.fromhex(conflict["ours_root"])),
            (5, bytes.fromhex(conflict["theirs_root"])),
            (6, bytes.fromhex(conflict["ours_delta"])),
            (7, bytes.fromhex(conflict["theirs_delta"])),
            (8, encode_uvar(len(entries)) + b"".join(sized(entry) for entry in entries)),
        ]
    )
    preimage = MAGIC + encode_uvar(1) + encode_uvar(CONTRACT_TAG) + epoch + sized(payload)
    digest = blake3.blake3(DOMAIN + preimage).digest()
    return preimage + digest, digest.hex()


def strict_decode(stored: bytes, epoch: bytes) -> None:
    reader = comparison.Reader(stored)
    if reader.take(8) != MAGIC:
        raise Failure("MERGE_CONFLICT_FORMAT_INVALID")
    if reader.uvar() != 1:
        raise Failure("MERGE_CONFLICT_VERSION_UNSUPPORTED")
    if reader.uvar() != CONTRACT_TAG:
        raise Failure("MERGE_CONFLICT_FORMAT_INVALID")
    if reader.take(32) != epoch:
        raise Failure("MERGE_CONFLICT_FORMAT_INVALID")
    reader.sized()
    trailer = reader.take(32)
    if reader.offset != len(stored):
        raise Failure("MERGE_CONFLICT_FORMAT_INVALID")
    if blake3.blake3(DOMAIN + stored[:-32]).digest() != trailer:
        raise Failure("MERGE_CONFLICT_DIGEST_MISMATCH")


def main() -> int:
    accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    problems: list[str] = []
    if accepted.get("contract") != "sley2-merge-v1":
        problems.append("accepted-contract")
    epochs = set()
    for vector in accepted.get("vectors", []):
        name = vector["id"]
        try:
            actual = judge(vector["ancestor"], vector["ours"], vector["theirs"])
        except (comparison.Failure, Failure) as failure:
            problems.append(f"{name}:unexpected-failure:{failure.code}")
            continue
        expected = vector["expected"]
        if vector["outcome"] == "merged":
            if actual["conflicts"]:
                problems.append(f"{name}:unexpected-conflicts:{actual['conflicts']}")
                continue
            merged_entities = {entity["id"]: entity for entity in expected["merged"]["entities"]}
            if set(merged_entities) != set(actual["merged"]):
                problems.append(f"{name}:merged-entity-set")
                continue
            for entity_id, entry in actual["merged"].items():
                emitted = merged_entities[entity_id]
                if body_of(emitted) != entry["body"]:
                    problems.append(f"{name}:merged-body:{entity_id[:4]}")
                if not entry["composed"] and emitted["object"] != entry["object"]:
                    problems.append(f"{name}:merged-object:{entity_id[:4]}")
                if entry["composed"] and emitted["object"] in (vector["ours"], vector["theirs"]):
                    problems.append(f"{name}:composed-object-reused:{entity_id[:4]}")
            if sorted(expected["metadata_overridden"]) != actual["metadata_overridden"]:
                problems.append(f"{name}:metadata_overridden")
        else:
            conflict = expected["conflict"]
            emitted = [tuple(entry) for entry in conflict["conflicts"]]
            if emitted != actual["conflicts"]:
                problems.append(f"{name}:conflict-entries:{actual['conflicts']}")
            stored = bytes.fromhex(expected["stored_hex"])
            epoch = stored[11:43]
            epochs.add(epoch)
            encoded, digest = encode_conflict(conflict, epoch)
            if encoded != stored:
                problems.append(f"{name}:stored-bytes")
            if digest != expected["merge_conflict_id"]:
                problems.append(f"{name}:merge-conflict-id")
    if len(epochs) != 1:
        problems.append("conflict-epoch-not-uniform")
    epoch = next(iter(epochs)) if epochs else bytes(32)
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
                "contract": "s20-520-merge-oracle-v1",
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
