#!/usr/bin/env python3
"""Render the six checker-owned S20-530 grouped multifault test bodies."""

from __future__ import annotations

import argparse
import re
import runpy
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "scripts/check_s20_530_crash_recovery.py"
GROUPED_M2_CASES = (
    ("COR-06", "head_checksum", "head_shape_before_receipt_missing"),
    ("COR-06", "head_checksum", "head_checksum_before_receipt_corrupt"),
    ("COR-07", "origin_format", "branch_record_format_version"),
    ("COR-07", "origin_digest", "branch_record_digest_mismatch"),
    ("COR-07", "ref_format", "ref_format_version"),
    ("COR-07", "ref_digest", "ref_digest_mismatch"),
)


def test_name(key: tuple[str, str, str]) -> str:
    row_id, subcase_id, leaf_id = key
    separator = "__" if row_id == "COR-06" else "_"
    return f"{row_id.lower().replace('-', '')}_{subcase_id}{separator}{leaf_id}"


def test_body_range(
    checker: dict[str, object],
    source: str,
    name: str,
    *,
    mask: str | None = None,
) -> tuple[int, int, str]:
    rust_code_mask = checker["rust_code_mask"]
    matching_delimiter = checker["matching_delimiter"]
    if not callable(rust_code_mask) or not callable(matching_delimiter):
        raise RuntimeError("checker parser functions are unavailable")
    if mask is None:
        mask = rust_code_mask(source)
    pattern = re.compile(
        rf"(?m)^(?P<indent>[ \t]*)#\[test\][ \t]*\n"
        rf"(?P=indent)fn[ \t]+{re.escape(name)}[ \t]*\([ \t]*\)[ \t]*\{{"
    )
    matches = tuple(pattern.finditer(source))
    if len(matches) != 1:
        raise RuntimeError(f"cannot isolate one exact #[test] fn {name}")
    match = matches[0]
    opening = match.end() - 1
    closing = matching_delimiter(source, mask, opening, "{", "}")
    if not isinstance(closing, int):
        raise RuntimeError(f"cannot isolate the body closing brace for {name}")
    return opening + 1, closing, match["indent"]


def rendered_body(statements: tuple[str, ...], indent: str) -> str:
    body_indent = indent + "    "
    rendered = "\n".join(
        "\n".join(f"{body_indent}{line}" for line in statement.splitlines())
        for statement in statements
    )
    return f"\n{rendered}\n{indent}"


def validate_case(
    checker: dict[str, object],
    source: str,
    key: tuple[str, str, str],
    *,
    mask: str | None = None,
) -> None:
    overlays = checker["MULTIFAULT_OVERLAYS"]
    multifault_operation_bindings = checker["multifault_operation_bindings"]
    multifault_operation_binding_problem = checker[
        "multifault_operation_binding_problem"
    ]
    if not isinstance(overlays, dict):
        raise RuntimeError("checker multifault registry is unavailable")
    spec = overlays[key]
    start, end, _indent = test_body_range(checker, source, test_name(key), mask=mask)
    entry = {"m2_operation_bindings": multifault_operation_bindings(key, spec)}
    problem = multifault_operation_binding_problem(
        source[start:end], key[0], key[1], entry, key[2]
    )
    if problem is not None:
        raise RuntimeError(f"{key!r} exact M2 body differs: {problem}")


def render_sources(checker: dict[str, object], *, write: bool) -> int:
    overlays = checker["MULTIFAULT_OVERLAYS"]
    multifault_statement_plan = checker["multifault_statement_plan"]
    if not isinstance(overlays, dict):
        raise RuntimeError("checker multifault registry is unavailable")
    by_source: dict[str, list[tuple[str, str, str]]] = {}
    for key in GROUPED_M2_CASES:
        spec = overlays[key]
        by_source.setdefault(spec.owner_source, []).append(key)

    changed = 0
    for relative, keys in by_source.items():
        path = ROOT / relative
        source = path.read_text(encoding="utf-8")
        source_mask = checker["rust_code_mask"](source)
        marker = "#[cfg(test)]\nmod tests"
        marker_at = source.find(marker)
        if marker_at < 0:
            raise RuntimeError(f"{relative} lacks its exact private test module marker")
        production_prefix = source[:marker_at]
        replacements: list[tuple[int, int, str]] = []
        for key in keys:
            start, end, indent = test_body_range(
                checker, source, test_name(key), mask=source_mask
            )
            statements = multifault_statement_plan(key, overlays[key])
            replacement = rendered_body(statements, indent)
            if source[start:end] != replacement:
                replacements.append((start, end, replacement))
        for start, end, replacement in sorted(replacements, reverse=True):
            source = source[:start] + replacement + source[end:]
        if source[:marker_at] != production_prefix:
            raise RuntimeError(f"{relative} production prefix changed during rendering")
        if replacements:
            changed += len(replacements)
            if write:
                path.write_text(source, encoding="utf-8")
        effective = (
            source if write or not replacements else path.read_text(encoding="utf-8")
        )
        if write or not replacements:
            effective_mask = checker["rust_code_mask"](effective)
            for key in keys:
                validate_case(checker, effective, key, mask=effective_mask)

    if write:
        print(f"rendered and validated {changed} grouped M2 test bodies")
        return 0
    if changed:
        print(f"{changed} grouped M2 test bodies differ; rerun with --write")
        return 1
    print(f"validated {len(GROUPED_M2_CASES)} exact grouped M2 test bodies")
    return 0


def restore_sources(checker: dict[str, object], revision: str) -> int:
    overlays = checker["MULTIFAULT_OVERLAYS"]
    if not isinstance(overlays, dict):
        raise RuntimeError("checker multifault registry is unavailable")
    by_source: dict[str, list[tuple[str, str, str]]] = {}
    for key in GROUPED_M2_CASES:
        spec = overlays[key]
        by_source.setdefault(spec.owner_source, []).append(key)

    changed = 0
    for relative, keys in by_source.items():
        path = ROOT / relative
        source = path.read_text(encoding="utf-8")
        baseline = subprocess.run(
            ["git", "show", f"{revision}:{relative}"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        source_mask = checker["rust_code_mask"](source)
        baseline_mask = checker["rust_code_mask"](baseline)
        marker = "#[cfg(test)]\nmod tests"
        marker_at = source.find(marker)
        if marker_at < 0 or baseline.find(marker) != marker_at:
            raise RuntimeError(f"{relative} production test boundary changed")
        production_prefix = source[:marker_at]
        replacements: list[tuple[int, int, str]] = []
        for key in keys:
            name = test_name(key)
            start, end, _ = test_body_range(checker, source, name, mask=source_mask)
            baseline_start, baseline_end, _ = test_body_range(
                checker, baseline, name, mask=baseline_mask
            )
            replacement = baseline[baseline_start:baseline_end]
            if source[start:end] != replacement:
                replacements.append((start, end, replacement))
        for start, end, replacement in sorted(replacements, reverse=True):
            source = source[:start] + replacement + source[end:]
        if source[:marker_at] != production_prefix:
            raise RuntimeError(
                f"{relative} production prefix changed during restoration"
            )
        if replacements:
            path.write_text(source, encoding="utf-8")
            changed += len(replacements)

    print(f"restored {changed} grouped M2 test bodies from {revision}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_mutually_exclusive_group()
    actions.add_argument(
        "--write",
        action="store_true",
        help="replace differing grouped M2 test bodies before validating them",
    )
    actions.add_argument(
        "--restore-revision",
        metavar="REVISION",
        help="restore only the six grouped M2 bodies from a Git revision",
    )
    args = parser.parse_args()
    checker = runpy.run_path(str(CHECKER), run_name="s20_530_grouped_m2_renderer")
    if args.restore_revision is not None:
        return restore_sources(checker, args.restore_revision)
    return render_sources(checker, write=args.write)


if __name__ == "__main__":
    raise SystemExit(main())
