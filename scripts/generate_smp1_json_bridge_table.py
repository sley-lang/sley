#!/usr/bin/env python3
"""Generate the S20-420 JSON bridge method tables from the frozen SMP1 method tables.

Versions 1 and 2 come from `docs/spec/SMP1.md` alone; version 3 unions those
frozen tables with the bounded additions in the native draft contract
(`docs/spec/NATIVE_TEST_ADMISSION_V1.md` appendix D)."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SMP1.md"
NATIVE_SPEC = ROOT / "docs/spec/NATIVE_TEST_ADMISSION_V1.md"
V1_TABLE = ROOT / "conformance/smp1-json-bridge/v1/methods.json"
V2_TABLE = ROOT / "conformance/smp1-json-bridge/v2/methods.json"
V3_TABLE = ROOT / "conformance/smp1-json-bridge/v3/methods.json"
ROW = re.compile(r"^\| (\d{3}) \| `([a-z._]+)` \| (.*?) \| (.*?) \| (.*?) \|$")
V1_SECTION = "### Protocol version 1"
V2_SECTION = "### Protocol version 2 additions"
V2_SECTION_END = "## 5. Bounded context"
# The version 3 additions live in the native draft family (still awaiting
# owner review), not in the frozen SMP1 contract: SMP1.md stays revision 13
# while the native contract owns the five rows below (two live selection
# reads since N7c revision 3, report paging live since N7d-1 revision 4,
# replay/status live since N7d-2 revision 5).
V3_SECTION = "## Appendix D. SMP v3 additions table (machine-readable, revision 5)"
V3_SECTION_END = "## 7. Required implementation evidence"
FAMILIES = {
    1: "session",
    2: "repository",
    3: "query",
    4: "candidate",
    5: "transaction",
    6: "runtime",
}
EXPECTED_V1_METHODS = 41
EXPECTED_V1_TAGS = (
    [100, 101, 102, 103, 104]
    + list(range(200, 215))
    + [300, 301, 302, 303, 304, 305]
    + [400, 401, 402, 403, 404]
    + [500, 501, 502, 503, 504]
    + [600, 601, 602, 603, 604]
)
EXPECTED_V2_ADDITIONS = ((306, "entity.version"), (307, "entity.signature"))
V2_OWNER = "S20-310"
# The version 3 additions are exactly the five native rows owned by the
# S20-620 test-selection seam: the two selection reads live since N7c
# revision 3, report paging live since N7d-1 revision 4, and the
# replay/status rows live since N7d-2 revision 5.
EXPECTED_V3_ADDITIONS = (
    (601, "tests.selected"),
    (602, "tests.affected"),
    (605, "tests.report_read"),
    (606, "tests.replay"),
    (607, "tests.attempt_status"),
)
# Tags live at version 3 (reserved flips false); every other native row
# stays reserved. A future semantics slice flips a row live by contract
# revision, never by editing this set beside the contract.
EXPECTED_V3_LIVE = (601, 602, 605, 606, 607)
# Live rows name their Appendix C typed records exactly; the bridge carries
# no bodies, so this pin is the only machine check on the record names.
EXPECTED_V3_LIVE_BODIES = {
    601: ("tests.selected.request", "tests.selected.response"),
    602: ("tests.affected.request", "tests.affected.response"),
    605: ("tests.report_read.request", "tests.report_read.response"),
    606: ("tests.replay.request", "tests.replay.response"),
    607: ("tests.attempt_status.request", "tests.attempt_status.response"),
}
V3_OWNER = "S20-620"


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


def _v3_section_lines(native_text: str) -> list[str]:
    """The bounded version-3 additions region in the native draft contract.

    The region lives under exactly one unique heading and ends at the next
    section; a method row outside it is a drift failure, never silently
    absorbed.
    """
    lines = native_text.splitlines()
    start_at = [index for index, line in enumerate(lines) if line == V3_SECTION]
    end_at = [index for index, line in enumerate(lines) if line == V3_SECTION_END]
    if len(start_at) != 1:
        raise SystemExit(f"expected exactly one {V3_SECTION!r} section, found {len(start_at)}")
    if len(end_at) != 1:
        raise SystemExit(f"expected exactly one {V3_SECTION_END!r} anchor, found {len(end_at)}")
    if not start_at[0] < end_at[0]:
        raise SystemExit("native v3 table sections are out of order")
    return lines[start_at[0] + 1 : end_at[0]]


def parse_v3_additions(native_text: str) -> list[dict]:
    """Parse and validate the bounded version-3 additions rows."""
    region = _v3_section_lines(native_text)
    region_rows = [line for line in region if ROW.match(line)]
    additions = [_parse_row(line) for line in region_rows]
    inside = len(region_rows)
    total = sum(1 for line in native_text.splitlines() if ROW.match(line))
    if total != inside:
        raise SystemExit(f"native method rows outside the v3 table section: {total} != {inside}")
    if [(method["tag"], method["name"]) for method in additions] != list(EXPECTED_V3_ADDITIONS):
        raise SystemExit(
            "v3 additions must be exactly 601 tests.selected, 602 tests.affected, "
            "605 tests.report_read, 606 tests.replay, and 607 tests.attempt_status "
            f"in order, found {[(m['tag'], m['name']) for m in additions]}"
        )
    for method, line in zip(additions, region_rows):
        if method["owner"] != V3_OWNER:
            raise SystemExit(f"v3 addition {method['tag']} must be owned by {V3_OWNER}")
        match = ROW.match(line)
        assert match is not None
        bodies = (match.group(3).strip("`"), match.group(4).strip("`"))
        if method["tag"] in EXPECTED_V3_LIVE:
            if method["reserved"]:
                raise SystemExit(f"v3 addition {method['tag']} is live and must not be reserved")
            if bodies != EXPECTED_V3_LIVE_BODIES[method["tag"]]:
                raise SystemExit(f"v3 addition {method['tag']} must name its Appendix C records, found {bodies}")
        elif not method["reserved"]:
            raise SystemExit(f"v3 addition {method['tag']} must stay reserved")
    return additions


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
    if v1_tags != list(EXPECTED_V1_TAGS):
        raise SystemExit(f"v1 method table is not the exact frozen tag inventory, found {v1_tags}")
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
    union_names = [method["name"] for method in v1_methods + v2_additions]
    if len(set(union_names)) != len(union_names):
        raise SystemExit("method names are not unique across the v1 plus v2 union")
    return v1_methods, v2_additions


def _table(methods: list[dict], v3_source: str | None = None) -> dict:
    table = {
        "contract": "docs/spec/SMP1_JSON_BRIDGE_V1.md",
        "source": "docs/spec/SMP1.md",
        "method_count": len(methods),
        "reserved_count": sum(1 for method in methods if method["reserved"]),
        "methods": methods,
    }
    if v3_source is not None:
        table["v3_source"] = v3_source
    return table


def build(protocol_version: int = 1) -> dict:
    """The method table for an explicit protocol version (default 1).

    The default stays the frozen version 1 table exactly; version 2 is the
    sorted union of version 1 and the bounded additions, emitted only on
    explicit selection. Version 3 unions the frozen SMP1 tables with the
    bounded native-contract additions, emitted only on explicit selection;
    the native rows' provenance rides the `v3_source` header beside the
    shared schema.
    """
    if protocol_version not in (1, 2, 3):
        raise SystemExit(f"unsupported protocol version: {protocol_version}")
    v1_methods, v2_additions = parse_tables(SPEC.read_text(encoding="utf-8"))
    if protocol_version == 1:
        return _table(v1_methods)
    if protocol_version == 2:
        union = sorted(v1_methods + v2_additions, key=lambda method: method["tag"])
        if len(union) != EXPECTED_V1_METHODS + len(v2_additions):
            raise SystemExit("version 2 union lost rows")
        return _table(union)
    v3_additions = parse_v3_additions(NATIVE_SPEC.read_text(encoding="utf-8"))
    # The live native rows override the frozen reserved rows at version 3
    # only: versions 1 and 2 generate from the frozen tables alone. The
    # frozen rows must still be reserved; a frozen contract that flips them
    # live under us is drift, never silently adopted.
    live = [method for method in v3_additions if method["tag"] in EXPECTED_V3_LIVE]
    pending = [method for method in v3_additions if method["tag"] not in EXPECTED_V3_LIVE]
    frozen_tags = {method["tag"]: method for method in v1_methods + v2_additions}
    for method in live:
        frozen = frozen_tags.get(method["tag"])
        if frozen is None:
            # A live row with no frozen ancestor (605 report paging) is
            # appended fresh: nothing reserved flips under us, so only the
            # contract-section parse above constrains it.
            continue
        if not frozen["reserved"] or frozen["name"] != method["name"]:
            raise SystemExit(f"v3 live row {method['tag']} is not the frozen reserved row")
    base = [method for method in v1_methods + v2_additions if method["tag"] not in EXPECTED_V3_LIVE]
    union = sorted(base + live + pending, key=lambda method: method["tag"])
    fresh = [method for method in live if method["tag"] not in frozen_tags]
    if len(union) != EXPECTED_V1_METHODS + len(v2_additions) + len(pending) + len(fresh):
        raise SystemExit("version 3 union lost rows")
    if {method["tag"] for method in pending} & set(frozen_tags):
        raise SystemExit("v3 additions overlap the frozen tables")
    union_names = [method["name"] for method in union]
    if len(set(union_names)) != len(union_names):
        raise SystemExit("method names are not unique across the v1 plus v2 plus v3 union")
    return _table(union, v3_source=str(NATIVE_SPEC.relative_to(ROOT)))


def table_path(protocol_version: int) -> Path:
    if protocol_version == 1:
        return V1_TABLE
    if protocol_version == 2:
        return V2_TABLE
    return V3_TABLE


def render(table: dict) -> str:
    return json.dumps(table, indent=2, sort_keys=True) + "\n"


def sums_path(protocol_version: int) -> Path:
    return table_path(protocol_version).parent / "SHA256SUMS"


def mint_sums(version_dir: Path) -> str:
    """Re-mint the version dir SHA256SUMS over every JSON it carries.

    Repair round 7 (SMP1 sums procedure): the table writer owns the sums of
    its version dir, so a future table change can never silently stale them;
    the conformance report still verifies them independently
    (CONFORMANCE_SUMS_MISMATCH stays fail-closed).
    """
    lines = []
    for member in sorted(version_dir.glob("*.json")):
        digest = hashlib.sha256(member.read_bytes()).hexdigest()
        lines.append(f"{digest}  {member.name}")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if the table drifted")
    parser.add_argument(
        "--protocol-version",
        type=int,
        choices=(1, 2, 3),
        default=1,
        help="exact contract table to write or check (default: 1)",
    )
    args = parser.parse_args()
    table_file = table_path(args.protocol_version)
    text = render(build(args.protocol_version))
    sums_file = sums_path(args.protocol_version)
    if args.check:
        current = table_file.read_text(encoding="utf-8") if table_file.exists() else None
        if current != text:
            print(json.dumps({"table": str(table_file.relative_to(ROOT)), "result": "FAIL", "problem": "drift"}))
            return 1
        print(json.dumps({"table": str(table_file.relative_to(ROOT)), "result": "PASS"}))
        return 0
    table_file.parent.mkdir(parents=True, exist_ok=True)
    table_file.write_text(text, encoding="utf-8")
    sums_file.write_text(mint_sums(table_file.parent), encoding="utf-8")
    print(json.dumps({"table": str(table_file.relative_to(ROOT)), "result": "WRITTEN"}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
