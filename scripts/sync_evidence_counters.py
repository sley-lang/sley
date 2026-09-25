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

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_reproducibility_report as reproducibility
import build_standards_sbom as standards
import history_ledger

ROOT = Path(__file__).resolve().parents[1]
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
REGISTER = ROOT / "evidence/review/finding-register.json"
DOSSIER = ROOT / "evidence/release/decision-dossier.json"
GA_ACCEPTANCE = ROOT / "evidence/release/ga-acceptance-report.json"
REPRO = ROOT / "evidence/release/reproducibility-report.json"
CYCLONEDX = ROOT / "evidence/release/sbom/cyclonedx-1.6.json"
PROVENANCE = ROOT / "evidence/release/provenance.json"
THREAT_REPORT = ROOT / "evidence/security/threat-coverage-report.json"
PRE_PUBLIC_CHAIN = ROOT / "evidence/history/reproducibility-attestation-chain.json"


def attestation_chain(reports: list[dict], prefix: list[dict] | None = None) -> list[dict]:
    """The candidate chain the tracked report's history attests, oldest first.

    Consecutive versions of the same commit collapse into one entry carrying
    the widest host set that version reached; a commit that returns later
    (a re-merge after a report replacement) is a new entry. Derived, not
    prose (Nabu P4 at c04539b9/db53894e on `attestation_supersedes`).
    `prefix` is the chain already derived from earlier versions; the fold
    continues it exactly as if those versions led `reports`.
    """
    chain: list[dict] = [dict(item) for item in prefix or []]
    for report in reports:
        for commit, entry in (report.get("commits") or {}).items():
            hosts = sorted(entry.get("hosts", [])) if isinstance(entry, dict) else []
            item = {"commit": commit, "hosts": hosts}
            if chain and chain[-1]["commit"] == commit:
                if len(hosts) > len(chain[-1]["hosts"]):
                    chain[-1] = item
            else:
                chain.append(item)
    return chain


def pre_public_chain(path: Path) -> list[dict]:
    """The chain derived from the report's archived pre-public versions.

    Those versions exist only in the archive, so their derivation was frozen
    once at the public-history cut in `PRE_PUBLIC_CHAIN`, bound to the
    archive the history ledger names. It prefixes the report at `path` only;
    a cut with no matching record fails closed instead of starting the chain
    over.
    """
    if not history_ledger.archived_tip():
        return []
    record_path = PRE_PUBLIC_CHAIN
    if not record_path.exists():
        raise ValueError(f"{record_path.name} missing for the archived history")
    record = json.loads(record_path.read_text(encoding="utf-8"))
    chain = record.get("chain")
    if (record.get("contract") != "sley2.pre-public-attestation-chain.v1" or not isinstance(chain, list)
            or not all(isinstance(item, dict) and set(item) == {"commit", "hosts"} for item in chain)):
        raise ValueError(f"{record_path.name} shape")
    if (record.get("archived_tip") != history_ledger.archived_tip()
            or record.get("archive_sha256") != history_ledger.ledger().get("archive", {}).get("sha256")):
        raise ValueError(f"{record_path.name} names another archive")
    if path.resolve() != (ROOT / str(record.get("path", ""))).resolve():
        return []
    return chain


def report_history(path: Path) -> list[dict]:
    """Every tracked version of the report, oldest first, then the working-tree
    version (the sync runs before the records commit that tracks the report
    being minted, so the on-disk file is the chain's last link — Nabu/Vulcan/
    Ariadne P3 at c67b0729: the chain had been one entry short at every mint).
    Versions at archived commits are the `pre_public_chain` prefix's, not
    repeated here. Empty without git."""
    import subprocess

    try:
        revisions = subprocess.check_output(
            ["git", "log", "--format=%H", "--reverse", "--", str(path.relative_to(ROOT))],
            cwd=ROOT, text=True, stderr=subprocess.DEVNULL,
        ).split()
    except (subprocess.CalledProcessError, OSError, ValueError):
        revisions = []
    versions: list[dict] = []
    archived = history_ledger.archived_commits()
    for revision in revisions:
        if revision in archived:
            continue
        try:
            text = subprocess.check_output(
                ["git", "show", f"{revision}:{path.relative_to(ROOT).as_posix()}"], cwd=ROOT, text=True
            )
            versions.append(json.loads(text))
        except (subprocess.CalledProcessError, json.JSONDecodeError):
            continue
    if path.exists():
        try:
            versions.append(json.loads(path.read_text(encoding="utf-8")))
        except json.JSONDecodeError:
            pass
    return versions


def realized_codes_recorded(report: object) -> int:
    """Rows of the threat-coverage report that record a realized code."""
    count = 0
    stack = [report]
    while stack:
        node = stack.pop()
        if isinstance(node, dict):
            if node.get("realized_code_recorded") is True:
                count += 1
            stack.extend(node.values())
        elif isinstance(node, list):
            stack.extend(node)
    return count


def main() -> int:
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    changed: list[str] = []

    section = summary.get("reproducibility_and_independent_conformance")
    if REPRO.exists() and isinstance(section, dict):
        report = json.loads(REPRO.read_text(encoding="utf-8"))
        problems = reproducibility.verify_report(report)
        if problems:
            raise ValueError(f"invalid reproducibility report: {problems}")
        selected = reproducibility.select_attestation(report)
        updates = {
            "reproducibility_result": report["result"],
            "second_host_status": report["second_host"]["status"],
            "attested_hosts": report["distinct_hosts"],
            "required_hosts": report["required_hosts"],
            "attested_commit": selected["commit"] if selected else None,
            "blockers": report["blockers"],
        }
        for key, value in updates.items():
            if section.get(key) != value:
                section[key] = value
                changed.append(f"reproducibility_and_independent_conformance.{key}")
        history = report_history(REPRO)
        if history:
            chain = attestation_chain(history, pre_public_chain(REPRO))
            if section.get("attestation_chain") != chain:
                section["attestation_chain"] = chain
                changed.append("reproducibility_and_independent_conformance.attestation_chain")

    section = summary.get("standards_sbom_and_provenance")
    if isinstance(section, dict):
        updates = standards.recorded_summary_facts(
            json.loads(CYCLONEDX.read_text()), json.loads(PROVENANCE.read_text())
        )
        for key, value in updates.items():
            if section.get(key) != value:
                section[key] = value
                changed.append(f"standards_sbom_and_provenance.{key}")

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

    # threat_coverage.realized_codes_recorded is derived from the tracked
    # report: the count of rows recording a realized code (the 25 -> 27 step
    # at 7426bc0b was a hand edit; Vulcan P4 at 92fa6646).
    if THREAT_REPORT.exists() and isinstance(summary.get("threat_coverage"), dict):
        realized = realized_codes_recorded(json.loads(THREAT_REPORT.read_text(encoding="utf-8")))
        if summary["threat_coverage"].get("realized_codes_recorded") != realized:
            summary["threat_coverage"]["realized_codes_recorded"] = realized
            changed.append("threat_coverage.realized_codes_recorded")

    if changed:
        SUMMARY.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"changed": sorted(changed), "result": "PASS"}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
