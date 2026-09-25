#!/usr/bin/env python3
"""Independently reproduce the S20-250 full direct edges and closure judgment.

The oracle re-derives, from the frozen fixture's compact JSON bodies, the
canonical direct edge set of `COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` section
5 plus the restricted profile's section 7.1 rows for the constructs the
fixture exercises (identity-valued fields, a parameter-returning block), the
section 7.2 recursion over nested type expressions and nested constants, and
the closure rules C1 through C11 in contract order with their first-failure
codes. It shares no code with the Rust implementation.
"""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ACCEPTED = ROOT / "conformance/complete-entity-impact/v1/accepted.json"
REJECTED = ROOT / "conformance/complete-entity-impact/v1/rejected.json"

OWNERSHIP, TYPE_REFERENCE, VALUE_REFERENCE, CONTROL_FLOW, CALL, EFFECT = 1, 2, 3, 4, 5, 6
CAPABILITY, CONTRACT, INITIALIZER, TEST_TARGET, ADAPTER, DEFINITION_MEMBER = 7, 8, 9, 10, 11, 12

CODES = {
    "IMPACT_SET_NOT_CANONICAL": 25009,
    "IMPACT_UNRESOLVED_ENTITY": 25010,
    "IMPACT_WRONG_ENTITY_KIND": 25011,
    "IMPACT_ROOT_INVENTORY_MISMATCH": 25014,
    "IMPACT_ROOT_WORKSPACE_MISSING": 25015,
    "IMPACT_ROOT_WORKSPACE_AMBIGUOUS": 25016,
    "IMPACT_ROOT_PACKAGE_MEMBERSHIP": 25017,
    "IMPACT_ROOT_NAMESPACE_ROOT": 25018,
    "IMPACT_ROOT_NAMESPACE_TREE": 25019,
    "IMPACT_ROOT_MEMBER_OWNERSHIP": 25020,
    "IMPACT_ROOT_EXPORT_UNSCOPED": 25021,
    "IMPACT_ROOT_ENTRY_POINTS_MISMATCH": 25022,
    "IMPACT_ROOT_DEPENDENCY_ROOTS_MISMATCH": 25023,
    "IMPACT_ROOT_DEPENDENCY_BINDING_UNOWNED": 25024,
}

# (field, edge kind, required dependency kind or None for any modeled, shape)
# in the exact order the contract rows and the restricted profile rows apply.
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
        ("return_parameter", VALUE_REFERENCE, 6, "one"),
    ],
    9: [],
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
SIX_BODY_SETS = {
    1: ["packages", "capability_requirements", "contracts", "tests"],
    2: ["dependencies", "exports"],
    3: ["members"],
    17: ["requirements"],
}
FORBIDDEN_MEMBER_KINDS = {1, 2, 6, 7, 8, 18}


class Failure(Exception):
    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def sorted_unique(values: list[str]) -> bool:
    return all(values[index] < values[index + 1] for index in range(len(values) - 1))


def require_canonical(values: list[str]) -> None:
    if not sorted_unique(values):
        raise Failure("IMPACT_SET_NOT_CANONICAL")


def references(entity: dict, field: str, shape: str) -> list[str]:
    value = entity.get(field)
    if shape == "list":
        return list(value or [])
    if shape == "option":
        return [] if value is None else [value]
    return [value]


# Type-bearing fields, per fingerprint profile section 7.1, that the fixture
# serializes as whole expressions. `many` is a list of expressions, and a null
# entry (a variant case with no payload) carries nothing.
TYPE_FIELDS: dict[int, list[tuple[str, str]]] = {
    4: [("member_types", "many")],
    5: [("result_type", "one")],
    6: [("value_type", "one")],
    10: [("value_type", "one")],
    11: [
        ("scope_type", "one"),
        ("request_type", "one"),
        ("response_type", "one"),
        ("failure_type", "one"),
    ],
    15: [("request_type", "one"), ("response_type", "one"), ("failure_type", "one")],
}
# Constant-bearing fields of the same table.
CONST_FIELDS: dict[int, list[tuple[str, str]]] = {
    9: [("value", "one")],
    12: [("allowed_scopes", "many")],
    14: [("inputs", "many"), ("expected", "one"), ("observations", "many")],
}


class Walker:
    """Section 7.2 recursion, derived here rather than read from the vector.

    The fixture carries whole type expressions and constants, so this walker
    applies the nested rules itself: a `Named` type reaches its definition, an
    adapter handle and a capability token reach their imports, a function type
    reaches its effects, a record or variant constant reaches its definition,
    and a function-reference constant reaches its function. Every endpoint is
    resolved and kind-checked exactly like a top-level field.
    """

    def __init__(self, kinds: dict[str, int], edges: set[tuple[str, str, int]]):
        self.kinds = kinds
        self.edges = edges

    def add(self, dependent: str, dependency: str, edge_kind: int, expected: int | None) -> None:
        actual = self.kinds.get(dependency)
        if actual is None:
            raise Failure("IMPACT_UNRESOLVED_ENTITY")
        if expected is not None and actual != expected:
            raise Failure("IMPACT_WRONG_ENTITY_KIND")
        self.edges.add((dependent, dependency, edge_kind))

    def type_expr(self, dependent: str, node: dict | None) -> None:
        if node is None:
            return
        tag = node["t"]
        if tag == 9:
            for item in node["items"]:
                self.type_expr(dependent, item)
        elif tag == 10:
            self.add(dependent, node["definition"], TYPE_REFERENCE, 4)
            for argument in node["arguments"]:
                self.type_expr(dependent, argument)
        elif tag in (11, 13, 18):
            self.type_expr(dependent, node["item"])
        elif tag == 12:
            self.type_expr(dependent, node["key"])
            self.type_expr(dependent, node["value"])
        elif tag == 14:
            self.type_expr(dependent, node["ok"])
            self.type_expr(dependent, node["error"])
        elif tag == 15:
            for parameter in node["parameters"]:
                self.type_expr(dependent, parameter)
            self.type_expr(dependent, node["result"])
            for effect in node["effects"]:
                self.add(dependent, effect, EFFECT, 11)
        elif tag == 16:
            self.add(dependent, node["adapter"], ADAPTER, 15)
        elif tag == 17:
            self.add(dependent, node["capability"], CAPABILITY, 12)

    def const_value(self, dependent: str, node: dict | None) -> None:
        if node is None:
            return
        self.type_expr(dependent, node["type"])
        data = node["data"]
        tag = data["t"]
        if tag == 9:
            for item in data["items"]:
                self.const_value(dependent, item)
        elif tag == 10:
            self.add(dependent, data["definition"], DEFINITION_MEMBER, 4)
            for field in data["fields"]:
                self.const_value(dependent, field)
        elif tag == 11:
            self.add(dependent, data["definition"], DEFINITION_MEMBER, 4)
            self.const_value(dependent, data["payload"])
        elif tag == 12:
            for entry in data["entries"]:
                self.const_value(dependent, entry["key"])
                self.const_value(dependent, entry["value"])
        elif tag == 13:
            self.const_value(dependent, data["item"])
        elif tag == 14:
            self.const_value(dependent, data["item"])
        elif tag == 15:
            self.add(dependent, data["function"], CALL, 5)
            for argument in data["type_arguments"]:
                self.type_expr(dependent, argument)


def direct_edges(entities: list[dict], kinds: dict[str, int]) -> list[tuple[str, str, int]]:
    edges: set[tuple[str, str, int]] = set()
    walker = Walker(kinds, edges)
    for entity in entities:
        kind = entity["kind"]
        rows = EDGE_ROWS.get(kind, [])
        if kind == 6:
            expected = 5 if entity["role"] == "function" else 7
            rows = [("owner", OWNERSHIP, expected, "one")]
        for field, edge_kind, expected, shape in rows:
            for dependency in references(entity, field, shape):
                actual = kinds.get(dependency)
                if actual is None:
                    raise Failure("IMPACT_UNRESOLVED_ENTITY")
                if expected is not None and actual != expected:
                    raise Failure("IMPACT_WRONG_ENTITY_KIND")
                edges.add((entity["id"], dependency, edge_kind))
        for field, shape in TYPE_FIELDS.get(kind, []):
            nodes = entity.get(field) if shape == "many" else [entity.get(field)]
            for node in nodes or []:
                walker.type_expr(entity["id"], node)
        for field, shape in CONST_FIELDS.get(kind, []):
            nodes = entity.get(field) if shape == "many" else [entity.get(field)]
            for node in nodes or []:
                walker.const_value(entity["id"], node)
    return sorted(edges)


def namespace_tree(root: str, namespaces: dict[str, dict]) -> set[str]:
    tree = {root}
    queue = [root]
    while queue:
        current = queue.pop(0)
        namespace = namespaces.get(current)
        if namespace is None:
            raise Failure("IMPACT_ROOT_NAMESPACE_TREE")
        for member in namespace["members"]:
            if member in namespaces and member not in tree:
                tree.add(member)
                queue.append(member)
    return tree


def judge(request: dict) -> dict:
    entities = request["entities"]
    facts = request["facts"]
    require_canonical(facts["bound_entities"])
    require_canonical(facts["entry_points"])
    require_canonical(facts["dependency_roots"])
    ids = [entity["id"] for entity in entities]
    require_canonical(ids)
    kinds = {entity["id"]: entity["kind"] for entity in entities}
    # C1
    if ids != facts["bound_entities"]:
        raise Failure("IMPACT_ROOT_INVENTORY_MISMATCH")
    for entity in entities:
        for field in SIX_BODY_SETS.get(entity["kind"], []):
            require_canonical(entity[field])
    # C2
    edges = direct_edges(entities, kinds)
    workspaces = [entity for entity in entities if entity["kind"] == 1]
    packages = {entity["id"]: entity for entity in entities if entity["kind"] == 2}
    namespaces = {entity["id"]: entity for entity in entities if entity["kind"] == 3}
    entry_points = [entity["id"] for entity in entities if entity["kind"] == 16]
    bindings = {entity["id"]: entity for entity in entities if entity["kind"] == 18}
    # C3
    if not workspaces:
        raise Failure("IMPACT_ROOT_WORKSPACE_MISSING")
    if len(workspaces) > 1:
        raise Failure("IMPACT_ROOT_WORKSPACE_AMBIGUOUS")
    workspace = workspaces[0]
    # C4
    if workspace["packages"] != sorted(packages) or any(
        package["workspace"] != workspace["id"] for package in packages.values()
    ):
        raise Failure("IMPACT_ROOT_PACKAGE_MEMBERSHIP")
    # C5
    roots: set[str] = set()
    for root in [workspace["root_namespace"]] + [package["root_namespace"] for package in packages.values()]:
        namespace = namespaces.get(root)
        if namespace is None or namespace["parent"] is not None or root in roots:
            raise Failure("IMPACT_ROOT_NAMESPACE_ROOT")
        roots.add(root)
    for namespace in namespaces.values():
        if namespace["parent"] is None and namespace["id"] not in roots:
            raise Failure("IMPACT_ROOT_NAMESPACE_ROOT")
    # C6
    for namespace in namespaces.values():
        parent = namespace["parent"]
        if parent is not None:
            parent_namespace = namespaces.get(parent)
            if parent_namespace is None or namespace["id"] not in parent_namespace["members"]:
                raise Failure("IMPACT_ROOT_NAMESPACE_TREE")
        for member in namespace["members"]:
            child = namespaces.get(member)
            if child is not None and child["parent"] != namespace["id"]:
                raise Failure("IMPACT_ROOT_NAMESPACE_TREE")
        cursor = namespace["parent"]
        steps = 0
        while cursor is not None:
            steps += 1
            if steps > len(namespaces):
                raise Failure("IMPACT_ROOT_NAMESPACE_TREE")
            cursor = namespaces[cursor]["parent"]
    # C7
    owner_of: dict[str, str] = {}
    for namespace in namespaces.values():
        for member in namespace["members"]:
            if kinds[member] in FORBIDDEN_MEMBER_KINDS or member in owner_of:
                raise Failure("IMPACT_ROOT_MEMBER_OWNERSHIP")
            owner_of[member] = namespace["id"]
    # C8
    trees: dict[str, set[str]] = {}
    for package in packages.values():
        tree = namespace_tree(package["root_namespace"], namespaces)
        for export in package["exports"]:
            if owner_of.get(export) not in tree:
                raise Failure("IMPACT_ROOT_EXPORT_UNSCOPED")
        trees[package["id"]] = tree
    # C9
    if entry_points != facts["entry_points"]:
        raise Failure("IMPACT_ROOT_ENTRY_POINTS_MISMATCH")
    # C10
    if sorted({binding["dependency_root"] for binding in bindings.values()}) != facts["dependency_roots"]:
        raise Failure("IMPACT_ROOT_DEPENDENCY_ROOTS_MISMATCH")
    # C11
    binding_owner: dict[str, str] = {}
    for package in packages.values():
        for dependency in package["dependencies"]:
            if dependency in binding_owner:
                raise Failure("IMPACT_ROOT_DEPENDENCY_BINDING_UNOWNED")
            binding_owner[dependency] = package["id"]
    for binding in bindings.values():
        owner = binding_owner.get(binding["id"])
        if owner is None or binding["local_namespace"] not in trees[owner]:
            raise Failure("IMPACT_ROOT_DEPENDENCY_BINDING_UNOWNED")
    reverse: dict[str, list[str]] = {}
    for dependent, dependency, _ in edges:
        reverse.setdefault(dependency, []).append(dependent)
    return {
        "direct_edges": [list(edge) for edge in edges],
        "workspace": workspace["id"],
        "packages": len(packages),
        "namespaces": len(namespaces),
        "bound_entities": len(ids),
        "reverse": reverse,
    }


def transitive(reverse: dict[str, list[str]], seeds: list[str]) -> list[str]:
    visited = set(seeds)
    queue = list(seeds)
    while queue:
        current = queue.pop(0)
        for dependent in reverse.get(current, []):
            if dependent not in visited:
                visited.add(dependent)
                queue.append(dependent)
    return sorted(visited)


def main() -> int:
    accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    problems: list[str] = []
    if accepted.get("contract") != "sley2-complete-entity-impact-v1":
        problems.append("accepted-contract")
    for vector in accepted.get("vectors", []):
        expected = vector["expected"]
        try:
            actual = judge(vector["request"])
        except Failure as failure:
            problems.append(f"{vector['id']}:unexpected-failure:{failure.code}")
            continue
        for key in ("direct_edges", "workspace", "packages", "namespaces", "bound_entities"):
            if actual[key] != expected[key]:
                problems.append(f"{vector['id']}:{key}")
        function = next(
            entity["id"] for entity in vector["request"]["entities"] if entity["kind"] == 5
        )
        if transitive(actual["reverse"], [function]) != expected["transitive_impact_of_function"]:
            problems.append(f"{vector['id']}:transitive_impact_of_function")
    for mutation in rejected.get("mutations", []):
        try:
            judge(mutation["request"])
        except Failure as failure:
            if failure.code != mutation["expected_code"]:
                problems.append(f"{mutation['id']}:code:{failure.code}")
            elif CODES[failure.code] != mutation["expected_numeric"]:
                problems.append(f"{mutation['id']}:numeric")
        else:
            problems.append(f"{mutation['id']}:accepted")
    print(
        json.dumps(
            {
                "contract": "s20-250-full-complete-entity-impact-oracle-v1",
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
