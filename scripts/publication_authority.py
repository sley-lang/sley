#!/usr/bin/env python3
"""The operator's publication decision, as recorded in the machine summary.

Publication was unauthorized by default, and every release record carried a
literal ``publication_authorized: false``. The operator authorized
publication of Sley 2.0.0 on 2026-09-24. The flag is now derived from one
recorded decision rather than restated in each builder and checker:

- ``publication_authorized: false`` with no decision is the default state;
- ``publication_authorized: true`` is valid only with a complete
  ``publication_decision`` record whose authority is the operator.

Anything else, such as a true flag without a decision or a decision with the
flag still false, is an inconsistent record. The strict readers refuse it,
and the tolerant ones report it as not authorized.
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"

DECISION_STATE = "PUBLICATION_AUTHORIZED"
DECISION_FIELDS = ("state", "authority", "decided", "scope", "authorized_tags", "public_repository")
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def load_summary() -> dict[str, Any]:
    return json.loads(SUMMARY.read_text(encoding="utf-8"))


def decision_problems(summary: dict[str, Any]) -> list[str]:
    """Every way the summary's publication record is inconsistent (empty = consistent)."""
    flag = summary.get("publication_authorized")
    decision = summary.get("publication_decision")
    if flag is False:
        return [] if decision is None else ["publication_decision present while publication_authorized is false"]
    if flag is not True:
        return ["publication_authorized is not a boolean"]
    if not isinstance(decision, dict):
        return ["publication_authorized is true without a publication_decision"]
    problems = [f"publication_decision missing {name}" for name in DECISION_FIELDS if name not in decision]
    if decision.get("state") != DECISION_STATE:
        problems.append(f"publication_decision state is not {DECISION_STATE}")
    if decision.get("authority") != "operator":
        problems.append("publication_decision authority is not the operator")
    if not isinstance(decision.get("decided"), str) or not DATE.match(decision["decided"]):
        problems.append("publication_decision decided is not an ISO date")
    for name in ("scope", "authorized_tags"):
        value = decision.get(name)
        if not isinstance(value, list) or not value or not all(isinstance(item, str) and item for item in value):
            problems.append(f"publication_decision {name} is not a non-empty list of strings")
    if not str(decision.get("public_repository", "")).startswith("https://"):
        problems.append("publication_decision public_repository is not an https URL")
    return problems


def is_authorized(summary: dict[str, Any] | None = None) -> bool:
    """Tolerant: True only for a consistent operator decision."""
    summary = load_summary() if summary is None else summary
    return summary.get("publication_authorized") is True and not decision_problems(summary)


def authorized(summary: dict[str, Any] | None = None) -> bool:
    """Strict: the recorded publication state, refusing an inconsistent record."""
    summary = load_summary() if summary is None else summary
    problems = decision_problems(summary)
    if problems:
        raise ValueError("inconsistent publication record: " + "; ".join(problems))
    return summary["publication_authorized"] is True


def authorized_tags(summary: dict[str, Any] | None = None) -> list[str]:
    """The git tags the operator decision covers (none unless authorized)."""
    summary = load_summary() if summary is None else summary
    if not is_authorized(summary):
        return []
    return list(summary["publication_decision"]["authorized_tags"])


if __name__ == "__main__":
    current = load_summary()
    problems = decision_problems(current)
    print(json.dumps({
        "publication_authorized": current.get("publication_authorized"),
        "authorized_tags": authorized_tags(current),
        "problems": problems,
        "result": "FAIL" if problems else "PASS",
    }, sort_keys=True))
    raise SystemExit(1 if problems else 0)
