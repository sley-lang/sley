#!/usr/bin/env python3
"""Generate the S20-420 JSON bridge method tables from the frozen SMP1 method tables."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SMP1.md"
V1_TABLE = ROOT / "conformance/smp1-json-bridge/v1/methods.json"
V2_TABLE = ROOT / "conformance/smp1-json-bridge/v2/methods.json"
ROW = re.compile(r"^\| (\d{3}) \| `([a-z._]+)` \| (.*?) \| (.*?) \| (.*?) \|$")
V1_SECTION = "### Protocol version 1"
V2_SECTION = "### Protocol version 2 additions"
V2_SECTION_END = "## 5. Bounded context"
FAMILIES = {
    1: "session",
    2: "repository",
    3: "query",
    4: "candidate",
    5: "transaction",
    6: "runtime",
}
EXPECTED_V1_METHODS = 41
EXPECTED_V2_ADDITIONS = ((306, "entity.version"), (307, "entity.signature"))
V2_OWNER = "S20-310"


def _section_lines(spec_text: str) -> tuple[list[str], list[str]]:
    """The bounded version-1 rows region and version-2 additions region.

    Each table lives under exactly one unique heading; a method row outside
    the declared regions is a drift failure, never silently absorbed.
    """
    lines = spec_text.splitlines()
    v1_at = [index for index, line in enumerate(lines) if line == V1_SECTION]
    v2_at = [index for index, line in enumerate(lines) if line == V2_SECTION]
    end_at = [index for index, line in enumerate(lines) if line == V2_SECTION_END]
    if len(v1_at) != 1:
        raise SystemExit(f"expected exactly one {V1_SECTION!r} section, found {len(v1_at)}")
    if len(v2_at) != 1:
        raise SystemExit(f"expected exactly one {V2_SECTION!r} section, found {len(v2_at)}")
    if len(end_at) != 1:
        raise SystemExit(f"expected exactly one {V2_SECTION_END!r} anchor, found {len(end_at)}")
    if not v1_at[0] < v2_at[0] < end_at[0]:
        raise SystemExit("protocol version sections are out of order")
    return lines[v1_at[0] + 1 : v2_at[0]], lines[v2_at[0] + 1 : end_at[0]]


def _parse_row(line: str) -> dict:
    match = ROW.match(line)
    if not match:
        raise SystemExit(f"unparsable method row: {line!r}")
    tag = int(match.group(1))
    request, response, owner = match.group(3), match.group(4), match.group(5)
    return {
        "tag": tag,
        "name": match.group(2),
        "family": FAMILIES[tag // 100],
        "reserved": request == "reserved" and response == "reserved",
        "owner": owner,
    }


def parse_tables(spec_text: str) -> tuple[list[dict], list[dict]]:
    """Parse and validate the bounded version tables from contract text."""
    v1_region, v2_region = _section_lines(spec_text)
    v1_methods = [_parse_row(line) for line in v1_region if ROW.match(line)]
    v2_additions = [_parse_row(line) for line in v2_region if ROW.match(line)]
    # Method rows outside the declared table sections are rejected: the
    # tables are the sections, not every matching row in the file.
    inside = sum(1 for line in v1_region + v2_region if ROW.match(line))
    total = sum(1 for line in spec_text.splitlines() if ROW.match(line))
    if total != inside:
        raise SystemExit(f"method rows outside the declared table sections: {total} != {inside}")
    if len(v1_methods) != EXPECTED_V1_METHODS:
        raise SystemExit(f"expected {EXPECTED_V1_METHODS} v1 methods, found {len(v1_methods)}")
    v1_tags = [method["tag"] for method in v1_methods]
    v1_names = [method["name"] for method in v1_methods]
    if v1_tags != sorted(v1_tags) or len(set(v1_tags)) != len(v1_tags) or len(set(v1_names)) != len(v1_names):
        raise SystemExit("v1 method table is not strictly increasing with unique names")
    if [(method["tag"], method["name"]) for method in v2_additions] != list(EXPECTED_V2_ADDITIONS):
        raise SystemExit(
            "v2 additions must be exactly 306 entity.version and 307 entity.signature in order, "
            f"found {[(m['tag'], m['name']) for m in v2_additions]}"
        )
    for method in v2_additions:
        if method["owner"] != V2_OWNER:
            raise SystemExit(f"v2 addition {method['tag']} must be owned by {V2_OWNER}")
        if method["reserved"]:
            raise SystemExit(f"v2 addition {method['tag']} must not be reserved")
    if {method["tag"] for method in v2_additions} & set(v1_tags):
        raise SystemExit("v2 additions overlap the v1 table")
    return v1_methods, v2_additions


def _table(methods: list[dict]) -> dict:
    return {
        "contract": "docs/spec/SMP1_JSON_BRIDGE_V1.md",
        "source": "docs/spec/SMP1.md",
        "method_count": len(methods),
        "reserved_count": sum(1 for method in methods if method["reserved"]),
        "methods": methods,
    }


def build(protocol_version: int = 1) -> dict:
    """The method table for an explicit protocol version (default 1).

    The default stays the frozen version 1 table exactly; version 2 is the
    sorted union of version 1 and the bounded additions, emitted only on
    explicit selection.
    """
    if protocol_version not in (1, 2):
        raise SystemExit(f"unsupported protocol version: {protocol_version}")
    v1_methods, v2_additions = parse_tables(SPEC.read_text(encoding="utf-8"))
    if protocol_version == 1:
        return _table(v1_methods)
    union = sorted(v1_methods + v2_additions, key=lambda method: method["tag"])
    if len(union) != EXPECTED_V1_METHODS + len(v2_additions):
        raise SystemExit("version 2 union lost rows")
    return _table(union)


def table_path(protocol_version: int) -> Path:
    return V1_TABLE if protocol_version == 1 else V2_TABLE


def render(table: dict) -> str:
    return json.dumps(table, indent=2, sort_keys=True) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if the table drifted")
    parser.add_argument(
        "--protocol-version",
        type=int,
        choices=(1, 2),
        default=1,
        help="exact contract table to write or check (default: 1)",
    )
    args = parser.parse_args()
    table_file = table_path(args.protocol_version)
    text = render(build(args.protocol_version))
    if args.check:
        current = table_file.read_text(encoding="utf-8") if table_file.exists() else None
        if current != text:
            print(json.dumps({"table": str(table_file.relative_to(ROOT)), "result": "FAIL", "problem": "drift"}))
            return 1
        print(json.dumps({"table": str(table_file.relative_to(ROOT)), "result": "PASS"}))
        return 0
    table_file.parent.mkdir(parents=True, exist_ok=True)
    table_file.write_text(text, encoding="utf-8")
    print(json.dumps({"table": str(table_file.relative_to(ROOT)), "result": "WRITTEN"}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
