#!/usr/bin/env python3
"""Generate the S20-420 JSON bridge method table from the frozen SMP1 method table."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SMP1.md"
TABLE = ROOT / "conformance/smp1-json-bridge/v1/methods.json"
ROW = re.compile(r"^\| (\d{3}) \| `([a-z._]+)` \| (.*?) \| (.*?) \| (.*?) \|$")
FAMILIES = {
    1: "session",
    2: "repository",
    3: "query",
    4: "candidate",
    5: "transaction",
    6: "runtime",
}
EXPECTED_METHODS = 41


def build() -> dict:
    methods = []
    for line in SPEC.read_text(encoding="utf-8").splitlines():
        match = ROW.match(line)
        if not match:
            continue
        tag = int(match.group(1))
        request, response, owner = match.group(3), match.group(4), match.group(5)
        methods.append(
            {
                "tag": tag,
                "name": match.group(2),
                "family": FAMILIES[tag // 100],
                "reserved": request == "reserved" and response == "reserved",
                "owner": owner,
            }
        )
    if len(methods) != EXPECTED_METHODS:
        raise SystemExit(f"expected {EXPECTED_METHODS} methods, found {len(methods)}")
    tags = [method["tag"] for method in methods]
    names = [method["name"] for method in methods]
    if tags != sorted(tags) or len(set(tags)) != len(tags) or len(set(names)) != len(names):
        raise SystemExit("method table is not strictly increasing with unique names")
    return {
        "contract": "docs/spec/SMP1_JSON_BRIDGE_V1.md",
        "source": "docs/spec/SMP1.md",
        "method_count": len(methods),
        "reserved_count": sum(1 for method in methods if method["reserved"]),
        "methods": methods,
    }


def render(table: dict) -> str:
    return json.dumps(table, indent=2, sort_keys=True) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if the table drifted")
    args = parser.parse_args()
    text = render(build())
    if args.check:
        current = TABLE.read_text(encoding="utf-8") if TABLE.exists() else None
        if current != text:
            print(json.dumps({"table": str(TABLE.relative_to(ROOT)), "result": "FAIL", "problem": "drift"}))
            return 1
        print(json.dumps({"table": str(TABLE.relative_to(ROOT)), "result": "PASS"}))
        return 0
    TABLE.parent.mkdir(parents=True, exist_ok=True)
    TABLE.write_text(text, encoding="utf-8")
    print(json.dumps({"table": str(TABLE.relative_to(ROOT)), "result": "WRITTEN"}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
