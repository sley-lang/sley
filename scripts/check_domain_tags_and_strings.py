#!/usr/bin/env python3
"""Digest-domain-tag table and script-side domain-string audit (AT-IG-07, AT-IG-08).

Two assertions over the frozen registry in docs/spec/IDENTIFIERS_V1.md:

1. The `digest_domain_tag` table matches the implementation: every row's
   integer appears at the source it names, no two rows share an integer, and
   no crate declares a DIGEST_DOMAIN_TAG constant the table omits. Two
   conventions coexist (3, 4, 8 equal sley-id Domain ordinals; 18 to 22 are
   sequential), so a new tag assembled by analogy could collide silently
   without this table.

2. `sley2.*` strings under scripts/ and bench/ that are not registered
   domains are evidence-chain labels, never BLAKE3 identity domains: a file
   carrying an unregistered label must not also hash with blake3. The
   registry's crate-side drift check lives in
   scripts/check_required_contract_index.py; this covers the scope that check
   does not read.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IDENTIFIERS = ROOT / "docs/spec/IDENTIFIERS_V1.md"
TABLE_HEADING = "## Digest domain tags"
# Python string literals in either quote style, with or without a bytes prefix.
DOMAIN_STRING = re.compile(r"""[bB]?(["'])(sley2\.[a-z0-9.\-]+)\1""")
BLAKE3_USE = re.compile(r"^\s*(?:import\s+blake3|from\s+blake3\s+import)|\bblake3\s*\.", re.M)
RUST_TAG = re.compile(r"DIGEST_DOMAIN_TAG: u32 = (\d+)")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def registry_domains(text: str) -> set[str]:
    return set(re.findall(r"`(sley2\.[a-z0-9.\-]+)`", text.split(TABLE_HEADING)[0]))


def script_label_problems(root: Path, registered: set[str]) -> tuple[list[str], list[str]]:
    """Unregistered `sley2.*` labels under scripts/ and bench/, and the files
    that carry one while also using blake3."""
    problems: list[str] = []
    unregistered: list[str] = []
    for path in sorted(list((root / "scripts").rglob("*.py")) + list((root / "bench").rglob("*.py"))):
        relative = str(path.relative_to(root))
        if "/tests/" in relative or "/.venv/" in relative or path.resolve() == Path(__file__).resolve():
            continue
        body = read(path)
        labels = {domain for _, domain in DOMAIN_STRING.findall(body) if domain not in registered}
        unregistered.extend(f"{relative}:{domain}" for domain in sorted(labels))
        if labels and BLAKE3_USE.search(body):
            problems.append(f"domain-string:unregistered-label-in-blake3-script:{relative}:{','.join(sorted(labels))}")
    return problems, unregistered


def self_test() -> int:
    """Evasion regression: a single-quoted label in a blake3 script must fail."""
    import tempfile

    failures: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "scripts").mkdir()
        (root / "bench").mkdir()
        evasive = root / "scripts" / "evasive.py"
        evasive.write_text("import blake3\nDOMAIN = 'sley2.evasive-label.v1'\n", encoding="utf-8")
        problems, _ = script_label_problems(root, {"sley2.object.v1"})
        if not any("evasive.py" in p for p in problems):
            failures.append("single-quoted-label-in-blake3-script-not-detected")
        evasive.write_text("from blake3 import blake3\nDOMAIN = b'sley2.evasive-label.v1'\n", encoding="utf-8")
        problems, _ = script_label_problems(root, {"sley2.object.v1"})
        if not any("evasive.py" in p for p in problems):
            failures.append("bytes-label-with-from-import-not-detected")
        evasive.write_text("import hashlib\nDOMAIN = 'sley2.evidence-label.v1'\n# blake3 mentioned only in a comment\n", encoding="utf-8")
        problems, unregistered = script_label_problems(root, {"sley2.object.v1"})
        if problems or len(unregistered) != 1:
            failures.append("sha256-label-without-blake3-wrongly-flagged")
    if failures:
        print("SELF_TEST FAIL: " + ", ".join(failures))
        return 1
    print("SELF_TEST PASS: 3 cases")
    return 0


def tag_table(text: str) -> list[tuple[int, str, str]]:
    """Rows of (tag, contract, source path) from the digest-domain-tag table."""
    if TABLE_HEADING not in text:
        return []
    section = text.split(TABLE_HEADING, 1)[1].split("\n## ", 1)[0]
    rows = []
    for line in section.splitlines():
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) >= 3 and cells[0].isdigit():
            rows.append((int(cells[0]), cells[1].strip("`"), cells[2].strip("`")))
    return rows


def main() -> int:
    if "--self-test" in sys.argv[1:]:
        return self_test()
    problems: list[str] = []
    text = read(IDENTIFIERS)
    rows = tag_table(text)
    if not rows:
        problems.append("tag-table:missing")
    seen: dict[int, str] = {}
    for tag, contract, source in rows:
        if tag in seen:
            problems.append(f"tag-table:duplicate-tag:{tag}:{seen[tag]}:{contract}")
        seen[tag] = contract
        path = ROOT / source
        if not path.is_file():
            problems.append(f"tag-table:source-missing:{source}")
            continue
        body = read(path)
        if source.endswith(".rs"):
            found = RUST_TAG.search(body)
            if found is None or int(found.group(1)) != tag:
                problems.append(f"tag-table:source-value-drift:{source}:{tag}")
        elif not re.search(rf"digest_domain_tag\D{{0,12}}{tag}\b", body):
            problems.append(f"tag-table:source-value-drift:{source}:{tag}")
    declared = {
        str(path.relative_to(ROOT))
        for path in sorted((ROOT / "crates").rglob("*.rs"))
        if "/target/" not in str(path) and RUST_TAG.search(read(path))
    }
    for source in sorted(declared - {source for _, _, source in rows}):
        problems.append(f"tag-table:crate-constant-not-tabled:{source}")

    # Script- and bench-side `sley2.*` strings that are not registered domains
    # are evidence-chain labels (SHA-256 framing, JSON contract names). They may
    # never be fed to the BLAKE3 identity hasher without registration.
    registered = registry_domains(text)
    label_problems, unregistered = script_label_problems(ROOT, registered)
    problems.extend(label_problems)

    print(
        json.dumps(
            {
                "problems": problems,
                "registered_domains": len(registered),
                "result": "PASS" if not problems else "FAIL",
                "tag_rows": len(rows),
                "unregistered_script_labels": len(unregistered),
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main())
