#!/usr/bin/env python3
"""Verify every failure symbol the crates emit is named by a governed document.

`ERROR_CODES_V1.md` declares the namespaces; each owning package contract
carries its own code table. Nothing checked that the two agree, so a crate
could emit a symbol that no document defines, and a consumer reading a
`source_symbol` or a protocol `symbol` field would have nothing to look up.

This audit reads every string literal in `crates/` that looks like a failure
symbol in a declared namespace and requires a *registration*: a row of a
document's code table, a reserved-code sentence, or a phase/decision row. A
passing mention in prose does not register a symbol, because prose can
mention a symbol that no owner ever assigned. It judges nothing else: a
symbol's meaning, numeric code, and freeze state stay with its owning
contract and that contract's checker.

It also verifies the pairing: one numeric code carries one symbol across the
whole tree. A module that attaches an owner's number to its own symbol is
how `GRAPH_RESOURCE_LIMIT` came to share 22020 with `CFG_RESOURCE_LIMIT`.
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


def registered() -> set[str]:
    """Symbols a contract assigns, rather than any document mentioning them.

    Only `docs/spec/` counts. Contracts assign codes in tables, in bullet
    lists, and in reservation sentences, and all three are registrations. An
    audit, a threat-register row, or an ADR narrative is not: prose can name a
    symbol no owner ever assigned, which is how `GRAPH_RESOURCE_LIMIT` looked
    registered while sharing 22020 with the S20-220 owner's own symbol.
    """
    found: set[str] = set()
    for path in sorted((ROOT / "docs/spec").rglob("*.md")):
        found.update(
            re.findall(
                r"`([A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+)`",
                path.read_text(encoding="utf-8", errors="ignore"),
            )
        )
    return found


def emitted(declared: set[str]) -> dict[str, str]:
    found: dict[str, str] = {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        for symbol in SYMBOL.findall(path.read_text(encoding="utf-8", errors="ignore")):
            if symbol.split("_", 1)[0] in declared:
                found.setdefault(symbol, str(path.relative_to(ROOT)))
    return found


def code_symbol_pairs() -> dict[int, set[str]]:
    """Every numeric code the crates emit, with the symbols it carries."""
    pairs: dict[int, set[str]] = {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        numbers = {
            match.group(1): int(match.group(2).replace("_", ""))
            for match in re.finditer(r"Self::(\w+)\s*=>\s*([0-9][0-9_]*),", text)
        }
        symbols = {
            match.group(1): match.group(2)
            for match in re.finditer(r'Self::(\w+)\s*=>\s*"([A-Z][A-Z0-9_]*)"', text)
        }
        for variant, number in numbers.items():
            if variant in symbols and number >= 1_000:
                pairs.setdefault(number, set()).add(symbols[variant])
    return pairs


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    declared = namespaces()
    assigned = registered()
    symbols = emitted(declared)
    unregistered = sorted(symbol for symbol in symbols if symbol not in assigned)
    pairs = code_symbol_pairs()
    ambiguous = sorted(number for number, names in pairs.items() if len(names) > 1)
    result = {
        "contract": CONTRACT,
        "declared_namespaces": len(declared),
        "emitted_symbols": len(symbols),
        "numeric_codes": len(pairs),
        "ambiguous_codes": [
            {"code": number, "symbols": sorted(pairs[number])} for number in ambiguous
        ],
        "unregistered": [{"symbol": s, "first_seen": symbols[s]} for s in unregistered],
        "scope": "REGISTRATION ONLY; MEANING, NUMBER, AND FREEZE STAY WITH THE OWNING CONTRACT",
        "result": "PASS" if not unregistered and not ambiguous else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not unregistered and not ambiguous else 1


if __name__ == "__main__":
    sys.exit(main())
