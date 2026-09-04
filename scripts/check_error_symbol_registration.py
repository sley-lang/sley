#!/usr/bin/env python3
"""Verify every failure symbol the crates emit is named by a governed document.

`ERROR_CODES_V1.md` declares the namespaces; each owning package contract
carries its own code table. Nothing checked that the two agree, so a crate
could emit a symbol that no document defines, and a consumer reading a
`source_symbol` or a protocol `symbol` field would have nothing to look up.

This audit reads every string literal in `crates/` that looks like a failure
symbol in a declared namespace and requires it to appear in some document
under `docs/`. It judges nothing else: a symbol's meaning, numeric code, and
freeze state stay with its owning contract and that contract's checker.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
NAMESPACE_SOURCE = ROOT / "docs/spec/ERROR_CODES_V1.md"
CONTRACT = "sley2.error-symbol-registration.v1"
# A symbol a document names as a family wildcard covers its members.
WILDCARD = re.compile(r"`([A-Z][A-Z0-9]*)_\*`")
SYMBOL = re.compile(r'"([A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+)"')


def namespaces() -> set[str]:
    return set(WILDCARD.findall(NAMESPACE_SOURCE.read_text(encoding="utf-8")))


def documented() -> str:
    return "\n".join(
        path.read_text(encoding="utf-8", errors="ignore")
        for path in sorted((ROOT / "docs").rglob("*.md"))
    )


def emitted(declared: set[str]) -> dict[str, str]:
    found: dict[str, str] = {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        for symbol in SYMBOL.findall(path.read_text(encoding="utf-8", errors="ignore")):
            if symbol.split("_", 1)[0] in declared:
                found.setdefault(symbol, str(path.relative_to(ROOT)))
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    declared = namespaces()
    docs = documented()
    symbols = emitted(declared)
    unregistered = sorted(symbol for symbol in symbols if symbol not in docs)
    result = {
        "contract": CONTRACT,
        "declared_namespaces": len(declared),
        "emitted_symbols": len(symbols),
        "unregistered": [{"symbol": s, "first_seen": symbols[s]} for s in unregistered],
        "scope": "REGISTRATION ONLY; MEANING, NUMBER, AND FREEZE STAY WITH THE OWNING CONTRACT",
        "result": "PASS" if not unregistered else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not unregistered else 1


if __name__ == "__main__":
    sys.exit(main())
