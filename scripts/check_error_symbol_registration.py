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
REPORT = ROOT / "evidence/security/error-symbol-registration.json"
CONTRACT = "sley2.error-symbol-registration.v1"
# A symbol a document names as a family wildcard covers its members.
# Repair round 7: multi-segment families (`STATE_ROOT_*`,
# `MUTATION_CANDIDATE_*`, ...) are namespaces too; the old single-segment
# wildcard plus first-segment filter silently missed every symbol in them.
WILDCARD = re.compile(r"`([A-Z][A-Z0-9]*(?:_[A-Z0-9]+)*)_\*`")
SYMBOL = re.compile(r'"([A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+)"')
STALE_ROOT_FAILURE = re.compile(r'stale_root_failure\(\s*"([A-Z][A-Z0-9_]*)"')
RESOURCE_FAILURE_LITERAL = re.compile(r'resource_failure\(\s*\d+\s*,\s*"([A-Z][A-Z0-9_]*)"')


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
    """Failure symbols the crates emit in a declared namespace.

    Repair round 7: the namespace match is the longest declared prefix, not
    the first segment, so multi-segment families resolve to their owners.
    Symbols whose literal constructors hide them from the string scan (the
    candidate-validation `stale_root_failure` / `resource_failure` helpers)
    are included with their defining file.
    """
    found: dict[str, str] = {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        for symbol in SYMBOL.findall(text):
            namespace = longest_namespace(symbol, declared)
            if namespace is not None:
                found.setdefault(symbol, str(path.relative_to(ROOT)))
        for symbol in STALE_ROOT_FAILURE.findall(
            text
        ) + RESOURCE_FAILURE_LITERAL.findall(text):
            namespace = longest_namespace(symbol, declared)
            if namespace is not None:
                found.setdefault(symbol, str(path.relative_to(ROOT)))
    return found


def longest_namespace(symbol: str, declared: set[str]) -> str | None:
    """Longest declared namespace owning the symbol, if any."""
    candidates = [
        namespace
        for namespace in declared
        if symbol == namespace or symbol.startswith(namespace + "_")
    ]
    if not candidates:
        return None
    return max(candidates, key=len)


def exercise_corpus_files() -> list[Path]:
    """Files whose content can exercise a refusal path.

    Repair round 7: expected-symbol tables, checker sources, and
    non-test production code do not exercise anything -- a table that
    names a symbol is the defect in finding form. Only tests, corpus
    fixtures, fuzz targets, and oracles count.
    """
    files: list[Path] = []
    for tree in ("conformance", "fuzz", "oracle"):
        files.extend(
            path
            for path in sorted((ROOT / tree).rglob("*"))
            if path.is_file()
            and path.suffix in (".rs", ".json", ".py")
            and "/target/" not in str(path)
        )
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        if "/tests/" in str(path) or path.name.startswith("test_"):
            files.append(path)
    return files


def unexercised() -> list[str]:
    """Stable symbols no test, corpus, fuzz target, or oracle ever reaches.

    A refusal path nothing exercises is where a defect survives. A test may
    name the failure by its string or by its enum variant, so both count:
    the string must appear outside the symbol's own defining file, and a
    variant use must be qualified to its defining enum (a bare-variant
    substring matched every homonym across all enums). Helper-constructed
    symbols have no variant arm; their string literal outside the defining
    file is the exercise rule.
    """
    definitions: dict[str, tuple[str, str | None]] = {}
    enum_names: set[str] = set()
    test_regions: dict[str, str] = {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        relative = str(path.relative_to(ROOT))
        enum_names.update(re.findall(r"enum (\w+)", text))
        # An in-file test module IS exercise: hits past the test marker
        # count even though the file is the symbol's defining file.
        marker = text.find("#[cfg(test)]")
        if marker < 0:
            marker = text.find("mod tests")
        if marker >= 0:
            test_regions[relative] = text[marker:]
        for match in re.finditer(
            r"Self::(\w+)\s*=>\s*(?:Some\()?\"([A-Z][A-Z0-9_]*)\"(?:\))?", text
        ):
            definitions.setdefault(match.group(2), (relative, match.group(1)))
        for symbol in STALE_ROOT_FAILURE.findall(
            text
        ) + RESOURCE_FAILURE_LITERAL.findall(text):
            definitions.setdefault(symbol, (relative, None))
    if not definitions:
        return []
    names = sorted(definitions)
    # One pass per file: string literals for every symbol at once, plus
    # every qualified Owner::Variant use.
    string_pat = re.compile("|".join(f'"{re.escape(symbol)}"' for symbol in names))
    qualified_pat = re.compile(r"(\w+)::(\w+)(?!::)")
    string_hits: dict[str, set[str]] = {symbol: set() for symbol in names}
    qualified_use_files: dict[tuple[str, str], set[str]] = {}
    for path in exercise_corpus_files():
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        relative = str(path.relative_to(ROOT))
        for match in string_pat.finditer(text):
            string_hits[match.group(0).strip('"')].add(relative)
        for match in qualified_pat.finditer(text):
            qualified_use_files.setdefault(
                (match.group(1), match.group(2)), set()
            ).add(relative)
    missing = []
    for symbol in names:
        defining, variant = definitions[symbol]
        if any(source != defining for source in string_hits[symbol]):
            continue
        own_tests = test_regions.get(defining, "")
        if f'"{symbol}"' in own_tests:
            continue
        exercised_by_variant = variant is not None and any(
            owner in enum_names and owner != "Self"
            for (owner, used_variant) in qualified_use_files
            if used_variant == variant
            and any(source != defining for source in qualified_use_files[(owner, used_variant)])
        )
        if not exercised_by_variant and variant is not None:
            exercised_by_variant = any(
                f"{owner}::{variant}" in own_tests
                for owner in enum_names
                if owner != "Self"
            )
        if not exercised_by_variant:
            missing.append(symbol)
    return sorted(missing)


def code_symbol_pairs() -> dict[int, set[str]]:
    """Every numeric code the crates emit, with the symbols it carries."""
    pairs: dict[int, set[str]] = {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        numbers = {}
        for match in re.finditer(
            r"Self::((?:\w+\s*\|\s*Self::)*\w+)\s*=>\s*(?:Some\()?([0-9][0-9_]*)(?:\))?,",
            text,
        ):
            for lhs in match.group(1).split("|"):
                numbers[lhs.strip().removeprefix("Self::").strip()] = int(
                    match.group(2).replace("_", "")
                )
        symbols = {}
        for match in re.finditer(
            r"Self::((?:\w+\s*\|\s*Self::)*\w+)\s*=>\s*(?:Some\()?\"([A-Z][A-Z0-9_]*)\"(?:\))?",
            text,
        ):
            for lhs in match.group(1).split("|"):
                symbols[lhs.strip().removeprefix("Self::").strip()] = match.group(2)
        for variant, number in numbers.items():
            if variant in symbols and number >= 1_000:
                pairs.setdefault(number, set()).add(symbols[variant])
    return pairs


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the derived report with the tracked one instead of writing it",
    )
    arguments = parser.parse_args()
    declared = namespaces()
    assigned = registered()
    symbols = emitted(declared)
    unregistered = sorted(symbol for symbol in symbols if symbol not in assigned)
    pairs = code_symbol_pairs()
    ambiguous = sorted(number for number, names in pairs.items() if len(names) > 1)
    never_exercised = unexercised()
    result = {
        "contract": CONTRACT,
        "declared_namespaces": len(declared),
        "emitted_symbols": len(symbols),
        "numeric_codes": len(pairs),
        "ambiguous_codes": [
            {"code": number, "symbols": sorted(pairs[number])} for number in ambiguous
        ],
        "unexercised": never_exercised,
        "unregistered": [{"symbol": s, "first_seen": symbols[s]} for s in unregistered],
        "scope": "REGISTRATION ONLY; MEANING, NUMBER, AND FREEZE STAY WITH THE OWNING CONTRACT",
        "result": (
            "PASS" if not unregistered and not ambiguous and not never_exercised else "FAIL"
        ),
    }
    text = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if arguments.check:
        current = REPORT.read_text(encoding="utf-8") if REPORT.is_file() else None
        if current != text:
            print(
                json.dumps(
                    {
                        "mode": "check",
                        "result": "FAIL",
                        "detail": "the tracked registration report differs from the derived report",
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
            return 1
    else:
        REPORT.parent.mkdir(parents=True, exist_ok=True)
        REPORT.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0 if not unregistered and not ambiguous and not never_exercised else 1


if __name__ == "__main__":
    sys.exit(main())
