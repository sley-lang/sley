#!/usr/bin/env python3
"""Copies the derived register, GA report, and dossier counters into the summary.

The summary records how many review obligations are open, how the GA
acceptance criteria stand, and how many dossier items are evidenced, and the
staged checkers cross-check those numbers against the derived documents. The
derivations are stable under counter updates (they digest their derived
entries, not the summary bytes, and none reads its own mirror), so this sync
converges in one pass.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
REGISTER = ROOT / "evidence/review/finding-register.json"
DOSSIER = ROOT / "evidence/release/decision-dossier.json"
GA_ACCEPTANCE = ROOT / "evidence/release/ga-acceptance-report.json"


def main() -> int:
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    changed: list[str] = []

    if REGISTER.exists() and isinstance(summary.get("finding_register"), dict):
        register = json.loads(REGISTER.read_text(encoding="utf-8"))
        updates = {
            "obligations": register["obligation_count"],
            "open_reviews": len(register["open_reviews"]),
            "deferred_reviews": len(register["deferred_reviews"]),
            "unclassified_dispositions": len(register["unclassified"]),
            "complete_packages": len(register["complete_packages"]),
            "register_result": register["result"],
        }
        for key, value in updates.items():
            if summary["finding_register"].get(key) != value:
                summary["finding_register"][key] = value
                changed.append(f"finding_register.{key}")

    if DOSSIER.exists() and isinstance(summary.get("decision_dossier"), dict):
        dossier = json.loads(DOSSIER.read_text(encoding="utf-8"))
        updates = {
            "evidenced_items": dossier["evidenced"],
            "gated_items": dossier["gated"],
            "decision_state": dossier["decision_state"],
            "decision_reasons": dossier["decision_reasons"],
        }
        sbom = next(
            (entry for entry in dossier.get("entries", []) if entry.get("item") == "SBOM and license inventory"),
            {},
        )
        if isinstance(sbom.get("value"), dict) and "license_disposition_blocked" in sbom["value"]:
            updates["license_disposition_blocked"] = sbom["value"]["license_disposition_blocked"]
        for key, value in updates.items():
            if summary["decision_dossier"].get(key) != value:
                summary["decision_dossier"][key] = value
                changed.append(f"decision_dossier.{key}")

    if GA_ACCEPTANCE.exists() and isinstance(summary.get("ga_acceptance"), dict):
        acceptance = json.loads(GA_ACCEPTANCE.read_text(encoding="utf-8"))
        updates = {
            "criterion_count": acceptance["criterion_count"],
            "states": acceptance["states"],
            "gated": acceptance["gated"],
            "awaiting_review": acceptance["awaiting_review"],
        }
        for key, value in updates.items():
            if summary["ga_acceptance"].get(key) != value:
                summary["ga_acceptance"][key] = value
                changed.append(f"ga_acceptance.{key}")

    if changed:
        SUMMARY.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"changed": sorted(changed), "result": "PASS"}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
