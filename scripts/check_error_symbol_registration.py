#!/usr/bin/env python3
"""Verify every failure symbol the crates emit is named by a governed document.

`ERROR_CODES_V1.md` declares the namespaces; each owning package contract
carries its own code table. Nothing checked that the two agree, so a crate
could emit a symbol that no document defines, and a consumer reading a
`source_symbol` or a protocol `symbol` field would have nothing to look up.

This audit reads every failure-shaped string literal in `crates/` (any
namespace or none) plus every `Self::Variant => "SYMBOL"` definition arm
and the candidate-validation helper literals, and requires a
*registration*: a backticked `SCREAMING_CASE` token in `docs/spec/`. The
registration rule is deliberately textual: it counts tables, bullet lists,
reservation sentences, and prose mentions alike, because the implementation
cannot tell a code table from a paragraph. That is a known limitation, not
a vouch: a prose mention laundering an unassigned symbol reads registered,
so reviewers must still spot-check that named symbols are actually
assigned by an owning contract (the `GRAPH_RESOURCE_LIMIT` lesson).

It also verifies the pairing: one numeric code carries one symbol across the
whole tree. A module that attaches an owner's number to its own symbol is
how `GRAPH_RESOURCE_LIMIT` came to share 22020 with `CFG_RESOURCE_LIMIT`.

Invariant-audit repair round 9: the emission census no longer filters by
declared namespace. Symbols outside every namespace used to be invisible
to both `unregistered` and the headline counts; now every defined or used
failure symbol is measured, unassigned ones fail exactly like
in-namespace ones, and assigned-but-family-less ones are named in
`registered_without_declared_namespace` and fail too: a symbol with no
declared namespace has no owning contract for meaning, number, and
freeze, so it is out of contract even when some document names it.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from rust_source_regions import rust_test_text


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
# Success symbols are not failure emissions: the contract's terminal states
# name VALID as the one state that permits commit, so a success symbol must
# never be demanded to register as an error. Scoped to exact symbols, never
# a prefix, so no failure symbol can hide behind it.
SUCCESS_SYMBOLS = frozenset({"CANDIDATE_VALIDATION_VALID"})


def namespaces() -> set[str]:
    return set(WILDCARD.findall(NAMESPACE_SOURCE.read_text(encoding="utf-8")))


def registered() -> set[str]:
    """Symbols a document names, textually.

    Only `docs/spec/` counts. The rule is textual on purpose: any
    backticked `SCREAMING_CASE` token in tables, bullet lists, reservation
    sentences, or prose registers. Prose therefore counts, with the known
    limitation stated in the module docstring (a prose mention can launder
    an unassigned symbol; reviewers spot-check assignment).
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


def emitted(declared: set[str]) -> dict[str, tuple[str, str | None]]:
    """Failure symbols the crates emit, with file and namespace-or-None.

    Repair round 9: the namespace filter is gone from the census. Every
    failure-shaped literal (`"FOO_BAR"`, single-token status words like
    `"CREATED"` excluded by the underscore requirement) plus the helper
    literals is collected; the caller partitions by registration and
    family instead of the census silently dropping the family-less.
    """
    found: dict[str, tuple[str, str | None]] = {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        relative = str(path.relative_to(ROOT))
        for symbol in SYMBOL.findall(text):
            if symbol in SUCCESS_SYMBOLS:
                continue
            if symbol not in found:
                found[symbol] = (relative, longest_namespace(symbol, declared))
        for symbol in STALE_ROOT_FAILURE.findall(
            text
        ) + RESOURCE_FAILURE_LITERAL.findall(text):
            if symbol not in found:
                found[symbol] = (relative, longest_namespace(symbol, declared))
    return found


def defined_symbols() -> set[str]:
    """Symbols the crates map an error variant to: the failure-precise census.

    A `Self::Variant => "SYMBOL"` arm, a payload-carrying arm
    (`Self::Digests(_) => "SYMBOL"` — the payload changes nothing about
    the variant-to-string role), a `write_str("SYMBOL")` Display mapping
    (the same variant-to-string role in another shape), a direct
    `Failure::new(..., "SYMBOL", ...)` construction, or a
    candidate-validation helper literal is emittable by construction.
    Single-token arms (`Self::Created => "CREATED"`) are status words, not
    failure symbols, and are excluded by the underscore requirement — the
    same shape the literal scan requires. Bare literals with no mapping
    role (test fixtures, log fragments, inline words in production files)
    are out of scope: without a mapping site their failure-hood cannot be
    told from data, so censusing them would force misregistration to
    clear. In-namespace bare literals stay censused (existing behavior:
    a test inventing a symbol in a declared family is still caught).
    """
    ARM = re.compile(
        r"Self::\w+(?:\([^)]*\))?\s*=>\s*(?:Some\()?\"([A-Z][A-Z0-9]*_[A-Z0-9_]*)\"(?:\))?"
    )
    WRITE = re.compile(r"write_str\(\"([A-Z][A-Z0-9]*_[A-Z0-9_]*)\"\)")
    DIRECT = re.compile(r"Failure::new\([^)]*\"([A-Z][A-Z0-9]*_[A-Z0-9_]*)\"")
    found: set[str] = set()
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        if "/target/" in str(path):
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        found.update(ARM.findall(text))
        found.update(WRITE.findall(text))
        found.update(DIRECT.findall(text))
        found.update(
            STALE_ROOT_FAILURE.findall(text) + RESOURCE_FAILURE_LITERAL.findall(text)
        )
    return found - SUCCESS_SYMBOLS


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
        # Only complete inline test modules count, never production text
        # following a test-only helper or a finished test module.
        tests = rust_test_text(text)
        if tests:
            test_regions[relative] = tests
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
    exercise_texts = dict(test_regions)
    for path in exercise_corpus_files():
        try:
            exercise_texts[str(path.relative_to(ROOT))] = path.read_text(
                encoding="utf-8", errors="ignore"
            )
        except OSError:
            continue
    for relative, text in exercise_texts.items():
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
    mapped = defined_symbols()
    # The census is in-namespace literals (existing behavior: a test
    # inventing a symbol in a declared family is caught) plus every
    # mapped symbol in any namespace or none. Out-of-namespace bare
    # literals with no mapping role are not failures by any evidence
    # the census can see, so they stay out rather than force
    # misregistration to clear.
    census = {
        symbol: source
        for symbol, source in symbols.items()
        if source[1] is not None or symbol in mapped
    }
    unregistered = sorted(symbol for symbol in census if symbol not in assigned)
    familyless = sorted(
        (
            {"symbol": symbol, "first_seen": census[symbol][0]}
            for symbol in census
            if symbol not in unregistered and census[symbol][1] is None
        ),
        key=lambda entry: entry["symbol"],
    )
    pairs = code_symbol_pairs()
    ambiguous = sorted(number for number, names in pairs.items() if len(names) > 1)
    never_exercised = unexercised()
    failures = bool(unregistered or ambiguous or never_exercised or familyless)
    result = {
        "contract": CONTRACT,
        "declared_namespaces": len(declared),
        "emitted_symbols": len(census),
        "numeric_codes": len(pairs),
        "ambiguous_codes": [
            {"code": number, "symbols": sorted(pairs[number])} for number in ambiguous
        ],
        "unexercised": never_exercised,
        "unregistered": [
            {"symbol": s, "first_seen": census[s][0]} for s in unregistered
        ],
        "registered_without_declared_namespace": familyless,
        "scope": "REGISTRATION ONLY; MEANING, NUMBER, AND FREEZE STAY WITH THE OWNING CONTRACT",
        "result": "PASS" if not failures else "FAIL",
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
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
