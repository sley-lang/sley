#!/usr/bin/env python3
"""Check the S20-770 required contract index against the repository."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
INDEX = ROOT / "docs/spec/REQUIRED_CONTRACT_INDEX_V1.md"
ADR = ROOT / "docs/adr/ADR-0047-required-contract-index.md"
IDENTIFIERS = ROOT / "docs/spec/IDENTIFIERS_V1.md"
MAKEFILE = ROOT / "Makefile"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"

DRAFT_STATUS = "S20_770_CONTRACT_DRAFT_REVIEW_PENDING"
ACCEPTED_STATUS = "S20_770_INDEX_ACCEPTED"
REQUIRED_ROWS = 12
CODES = ((77000, "CONTRACT_INDEX_UNSATISFIED"), (77001, "CONTRACT_INDEX_DRIFT"))
BACKTICKED = re.compile(r"`([^`]+)`")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def table_rows(text: str) -> list[list[str]]:
    start = text.index("## 1. The twelve required contracts")
    end = text.index("## 2. Rules", start)
    rows = []
    for line in text[start:end].split("\n"):
        line = line.strip()
        if not line.startswith("| 17."):
            continue
        rows.append([cell.strip() for cell in line.strip("|").split("|")])
    return rows


def check_current_delta_review(
    section: dict, expected_revision: int, status: object, problems: list[str]
) -> None:
    """The revision-bound current review record for this contract delta.

    Historical review fields keep their own revisions and never satisfy the
    current delta: only this object, bound to the anchored Status revision,
    admits acceptance, while draft states stay valid with PENDING.
    """
    review = section.get("current_delta_review")
    if not isinstance(review, dict) or set(review) != {
        "contract_revision",
        "ariadne",
        "nabu",
        "vulcan",
    }:
        problems.append("review:current-delta-shape")
        return
    revision = review.get("contract_revision")
    if type(revision) is not int or revision != expected_revision:
        problems.append(f"review:current-delta-revision:{revision!r}")
    for lane in ("ariadne", "nabu", "vulcan"):
        if review.get(lane) not in (
            "PENDING",
            "PASS",
            "NEEDS_WORK",
            "FAIL",
            "INCOMPLETE",
        ):
            problems.append(f"review:current-delta-judgment:{lane}")
    if status == ACCEPTED_STATUS or (
        isinstance(status, str) and "CONTRACT_FROZEN" in status
    ):
        if not all(review.get(lane) == "PASS" for lane in ("ariadne", "nabu", "vulcan")):
            problems.append("review:current-delta-frozen-requires-pass")


def main() -> int:
    problems: list[str] = []
    for path in (INDEX, ADR, IDENTIFIERS, MAKEFILE, SUMMARY, WORK_PACKAGES):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2, sort_keys=True))
        return 1

    index = read(INDEX)
    identifiers = read(IDENTIFIERS)
    makefile = read(MAKEFILE)
    rows = table_rows(index)
    if len(rows) != REQUIRED_ROWS:
        problems.append(f"index-rows:{len(rows)}")

    documents = 0
    checkers = 0
    domains = 0
    for row in rows:
        number, name, document_cell, domain_cell, checker_cell, corpus_cell = row[:6]
        if not name.startswith("`sley-") or not name.endswith("-v1`"):
            problems.append(f"{number}: required name is not a versioned contract name")
        for document in BACKTICKED.findall(document_cell):
            if not document.endswith(".md"):
                continue
            documents += 1
            if not (ROOT / "docs/spec" / document).exists():
                problems.append(f"{number}: missing document {document}")
        for domain in BACKTICKED.findall(domain_cell):
            domains += 1
            if domain not in identifiers:
                problems.append(f"{number}: domain {domain} is not frozen in IDENTIFIERS_V1.md")
        for checker in BACKTICKED.findall(checker_cell):
            if not checker.endswith(".py"):
                continue
            checkers += 1
            if not (ROOT / "scripts" / checker).exists():
                problems.append(f"{number}: missing checker {checker}")
            elif f"scripts/{checker}" not in makefile:
                problems.append(f"{number}: checker {checker} is not run by the Makefile")
        for corpus in BACKTICKED.findall(corpus_cell):
            if corpus.startswith("conformance/") and not (ROOT / corpus).is_dir():
                problems.append(f"{number}: missing corpus {corpus}")
            if corpus.startswith("crates/") and not (ROOT / corpus).is_dir():
                problems.append(f"{number}: missing crate {corpus}")

    # Registry drift validation, which IDENTIFIERS_V1.md requires of every
    # added domain: the implementation's derived domains and the frozen
    # registry must be the same set.
    # Every crate that hashes may define a domain, not only `sley-id`.
    derived = sorted(
        {
            domain
            for path in sorted((ROOT / "crates").rglob("*.rs"))
            if "/target/" not in str(path)
            for domain in re.findall(r'"(sley2\.[a-z0-9.\-]+)"', read(path))
        }
    )
    unregistered = [domain for domain in derived if f"`{domain}`" not in identifiers]
    for domain in unregistered:
        problems.append(f"identifier-registry-drift:{domain}")

    summary = json.loads(read(SUMMARY))
    section = summary.get("required_contract_index")
    if not isinstance(section, dict):
        print(
            json.dumps(
                {"problems": ["machine-summary:section"], "result": "FAIL"},
                indent=2,
                sort_keys=True,
            )
        )
        return 1
    status = section.get("status")
    if status not in (DRAFT_STATUS, ACCEPTED_STATUS):
        problems.append("machine-summary:status")
    own_revision = re.search(r"^Status:.*revision (\d+)", index, flags=re.M)
    if own_revision is None:
        problems.append("index-revision:status-line")
    else:
        check_current_delta_review(section, int(own_revision.group(1)), status, problems)
    for key, value in (
        ("contract", "docs/spec/REQUIRED_CONTRACT_INDEX_V1.md"),
        ("adr", "docs/adr/ADR-0047-required-contract-index.md"),
        ("checker", "scripts/check_required_contract_index.py"),
        ("required_contracts", REQUIRED_ROWS),
        ("new_error_code_range", "77000 through 77001"),
        ("defines_no_contract", True),
    ):
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status == ACCEPTED_STATUS:
        for review in (
            "ariadne_contract_review",
            "nabu_architecture_review",
            "vulcan_surface_review",
        ):
            if section.get(review) != "PASS":
                problems.append(f"machine-summary:{review}")
    codes = read(ROOT / "docs/spec/ERROR_CODES_V1.md")
    for number, symbol in CODES:
        if symbol not in codes:
            problems.append(f"error-codes-missing:{number}:{symbol}")
    if "`docs/spec/REQUIRED_CONTRACT_INDEX_V1.md`" not in read(WORK_PACKAGES):
        problems.append("work-package-missing:index")

    result = {
        "checkers_named": checkers,
        "contract": "s20-770-required-contract-index-v1",
        "derived_identifier_domains": len(derived),
        "documents_named": documents,
        "domains_named": domains,
        "problems": problems,
        "required_contracts": len(rows),
        "result": "FAIL" if problems else "PASS",
        "status": status,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
