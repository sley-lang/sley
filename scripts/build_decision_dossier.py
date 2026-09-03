#!/usr/bin/env python3
"""S20-750 decision dossier: every master-goal section 30 item and the state.

Derives `evidence/release/decision-dossier.json` from the machine summary and
the tracked S20-710 full, S20-720, S20-730, and S20-740 reports. It reports;
it never decides, approves, publishes, or opens a product gate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from enum import IntEnum
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
REPRO = ROOT / "evidence/release/reproducibility-report.json"
CYCLONEDX = ROOT / "evidence/release/sbom/cyclonedx-1.6.json"
SPDX = ROOT / "evidence/release/sbom/spdx-2.3.json"
INVENTORY = ROOT / "evidence/security/T52/pre-release-inventory.json"
PROVENANCE = ROOT / "evidence/release/provenance.json"
CONFORMANCE = ROOT / "evidence/conformance/independent-conformance-report.json"
REGISTER = ROOT / "evidence/review/finding-register.json"
STATE_ROOT_FIXTURE = ROOT / "conformance/state-root/v1/accepted.json"
DOSSIER = ROOT / "evidence/release/decision-dossier.json"
CONTRACT = "sley2.decision-dossier.v1"
DECISION_AUTHORITY = "OPERATOR_DECISION_NOT_DELEGATED"


class DossierErrorCode(IntEnum):
    """S20-750 dossier failures (contract section 5)."""

    SOURCE_MISSING = 76000
    SOURCE_INVALID = 76001
    DECISION_INVALID = 76002
    DRIFT = 76003


class DossierError(Exception):
    """One exact S20-750 failure."""

    def __init__(self, code: DossierErrorCode, detail: str) -> None:
        super().__init__(f"{code.name}: {detail}")
        self.code = code
        self.detail = detail


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def display(path: Path) -> str:
    """The repository-relative path when the path is inside the tree."""
    return str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path)


def load(path: Path, contract: str | None = None) -> dict:
    if not path.exists():
        raise DossierError(DossierErrorCode.SOURCE_MISSING, f"{display(path)} does not exist")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise DossierError(DossierErrorCode.SOURCE_INVALID, f"{path.name}: {error}") from error
    if not isinstance(value, dict):
        raise DossierError(DossierErrorCode.SOURCE_INVALID, f"{path.name} is not an object")
    if contract is not None and value.get("contract") != contract:
        raise DossierError(DossierErrorCode.SOURCE_INVALID, f"{path.name} has the wrong contract")
    return value


def relative(path: Path) -> str:
    return display(path)


def entry(item: str, *, value=None, evidence: list[Path] | None = None, note: str) -> dict:
    """One section 30 entry; a value makes it EVIDENCED, its absence GATED."""
    return {
        "item": item,
        "state": "GATED" if value is None else "EVIDENCED",
        "value": value,
        "evidence": sorted(relative(path) for path in (evidence or [])),
        "note": note,
    }


def build_entries(sources: dict) -> list[dict]:
    summary = sources["summary"]
    repro = sources["repro"]
    register = sources["register"]
    conformance = sources["conformance"]
    inventory = sources["inventory"]
    bom = sources["cyclonedx"]
    provenance = sources["provenance"]
    attestation = repro["attestations"][0] if repro.get("attestations") else None
    succession = summary.get("succession", {})
    audit = summary.get("s20_710_pre_release_audit", {})
    trials = succession.get("trials_executed", 0)
    benchmark_note = (
        "no succession trial has been executed: the legacy and Sley 2 arms are recorded as "
        "runners only, so every per-arm metric is unresolved"
    )

    def package_state(name: str) -> str | None:
        section = summary.get(name)
        return section.get("status") if isinstance(section, dict) else None

    entries = [
        entry(
            "final repository",
            value="sley2",
            evidence=[SUMMARY],
            note="the master goal names the repository; the final identity is fixed at the decision",
        ),
        entry(
            "final branch",
            note="the release branch is fixed at the release decision, which has not been made",
        ),
        entry(
            "final commit",
            value=attestation["commit"] if attestation else None,
            evidence=[REPRO] if attestation else [],
            note="the commit the local candidate was built and attested from"
            if attestation
            else "no attested candidate build",
        ),
        entry(
            "clean-working-tree status",
            value=attestation["working_tree_clean"] if attestation else None,
            evidence=[REPRO] if attestation else [],
            note="the attested build refuses a dirty tree (S20-730)"
            if attestation
            else "no attestation",
        ),
        entry(
            "frozen legacy artifact verification",
            value=summary.get("legacy_freeze"),
            evidence=[SUMMARY],
            note="the frozen Sley 1.2.0 commit, artifact digest, size, and source snapshot digest",
        ),
        entry(
            "final schema epoch",
            value=summary.get("schema_epoch", {}).get("bootstrap_schema_epoch_id"),
            evidence=[SUMMARY],
            note="the S20-140 bootstrap schema epoch identity",
        ),
        entry(
            "final state-root demo identifiers",
            value={
                "conformance_state_root": summary.get("state_root", {}).get(
                    "accepted_state_root"
                ),
                "conformance_epoch_id": summary.get("state_root", {}).get(
                    "conformance_epoch_id"
                ),
            },
            evidence=[SUMMARY, STATE_ROOT_FIXTURE],
            note="the accepted state-root vector identities; the release demo exercises them "
            "through the packaged binary",
        ),
        entry(
            "SCB1 conformance result",
            value={
                "status": package_state("conformance"),
                "cross_implementation_agreement": summary.get("conformance", {}).get(
                    "cross_implementation_agreement"
                ),
                "independently_checked_families": conformance["independently_checked"],
                "native_only_families": conformance["native_only"],
            },
            evidence=[SUMMARY, CONFORMANCE],
            note="S20-130 agreement plus the S20-730 family coverage",
        ),
        entry(
            "independent encoder result",
            value={
                "oracle": summary.get("conformance", {}).get("implementation"),
                "language": summary.get("conformance", {}).get("language"),
                "rust_dependency_check": summary.get("conformance", {}).get(
                    "rust_dependency_check"
                ),
                "python_sources": conformance["oracle_independence"]["python_sources"],
            },
            evidence=[SUMMARY, CONFORMANCE],
            note="the independent Python oracle and its S20-130 independence scan",
        ),
        entry(
            "SSMC1 corpus counts",
            value={
                "entity_kinds": summary.get("ssmc1", {}).get("entity_kinds"),
                "type_tags": summary.get("ssmc1", {}).get("type_tags"),
                "opcodes": summary.get("ssmc1", {}).get("opcodes"),
                "terminators": summary.get("ssmc1", {}).get("terminators"),
                "succession_corpus_tasks": succession.get("tasks"),
            },
            evidence=[SUMMARY],
            note="the frozen SSMC1 epoch-1 counts and the frozen succession corpus size",
        ),
        entry(
            "property-test counts",
            note="the aggregate property-test count is not recorded as a tracked number; the "
            "per-package test counts live in package sections only",
        ),
        entry(
            "fuzz duration and findings",
            value={
                "persistent_target_count": summary.get("adversarial", {}).get(
                    "persistent_target_count"
                ),
                "persistent_landed_surface_count": summary.get("adversarial", {}).get(
                    "persistent_landed_surface_count"
                ),
                "bounded_inputs": summary.get("adversarial", {}).get("bounded_inputs"),
                "campaign_duration": None,
            },
            evidence=[SUMMARY],
            note="bounded smoke coverage is recorded; no timed release fuzz campaign has been run, "
            "so the duration stays null inside an otherwise evidenced item",
        ),
        entry(
            "adversarial result",
            value={"status": package_state("adversarial")},
            evidence=[SUMMARY],
            note="the S20-700 bounded slice state",
        ),
        entry(
            "crash-injection result",
            value={
                "status": package_state("s20_530_crash_recovery"),
                "matrix_rows": summary.get("s20_530_crash_recovery", {}).get("matrix_rows"),
            },
            evidence=[SUMMARY],
            note="the S20-530 crash recovery matrix and its frozen contract",
        ),
        entry(
            "security review result",
            note="the independent security review is Vulcan's and the lane is unavailable "
            "(the finding register records the deferred dispositions)",
        ),
        entry(
            "succession benchmark methodology",
            value={
                "corpus_version": succession.get("corpus_version"),
                "corpus_sha256": succession.get("corpus_sha256"),
                "design_status": succession.get("design_status"),
                "tasks": succession.get("tasks"),
                "run_freeze_controls": succession.get("run_freeze_controls"),
                "metrics": succession.get("metrics"),
                "accounting_contract": summary.get("succession_accounting", {}).get(
                    "report_contract"
                ),
            },
            evidence=[SUMMARY],
            note="the frozen methodology, corpus, and the S20-630 accounting contract",
        ),
        entry("strict correctness by arm", note=benchmark_note),
        entry("ACT by arm", note=benchmark_note),
        entry("context bytes and model tokens by arm", note=benchmark_note),
        entry("repair loops by arm", note=benchmark_note),
        entry("invalid committed states", note=benchmark_note),
        entry("stale candidates incorrectly accepted", note=benchmark_note),
        entry(
            "optional Zerolang comparison",
            value={"status": package_state("s20_650_external_comparison")},
            evidence=[SUMMARY],
            note="the optional external arm is explicitly unavailable, which is a recorded state "
            "rather than a gap",
        ),
        entry(
            "artifact path",
            value=attestation["artifact_name"] if attestation else None,
            evidence=[REPRO] if attestation else [],
            note="the local candidate artifact name; no published path exists",
        ),
        entry(
            "SHA-256",
            value=attestation["artifact_sha256"] if attestation else None,
            evidence=[REPRO, PROVENANCE] if attestation else [],
            note="the attested artifact digest, which the provenance subject repeats",
        ),
        entry(
            "byte size",
            value=attestation["artifact_size_bytes"] if attestation else None,
            evidence=[REPRO] if attestation else [],
            note="the attested artifact size",
        ),
        entry(
            "reproducibility result",
            value={
                "result": repro.get("result"),
                "distinct_hosts": repro.get("distinct_hosts"),
                "second_host": repro.get("second_host", {}).get("status"),
            },
            evidence=[REPRO],
            note="single-host reproducibility; the second host is an operator-gated lane",
        ),
        entry(
            "SBOM and license inventory",
            value={
                "cyclonedx_spec_version": bom.get("specVersion"),
                "spdx_version": sources["spdx"].get("spdxVersion"),
                "components": len(bom.get("components", [])),
                "license_disposition_blocked": sum(
                    1
                    for package in inventory.get("packages", [])
                    if str(package.get("license_disposition", "")).startswith("BLOCKED")
                ),
                "root_license_text_approved": audit.get("root_license_text_approved"),
            },
            evidence=[CYCLONEDX, SPDX, INVENTORY],
            note="both standards documents exist as drafts; the root license text is unapproved, "
            "which the documents state in band",
        ),
        entry(
            "findings by severity and disposition",
            value={
                "obligations": register.get("obligation_count"),
                "states": register.get("states"),
                "severity_mentions": register.get("severity_mentions"),
                "declared_open_findings": register.get("declared_open_findings"),
                "result": register.get("result"),
            },
            evidence=[REGISTER],
            note="the S20-740 register of every recorded review obligation",
        ),
        entry(
            "residual risks",
            value=summary.get("open_risks"),
            evidence=[SUMMARY],
            note="the recorded open risks",
        ),
        entry(
            "evidence gaps",
            value={
                "recorded": summary.get("evidence_gaps"),
                "gated_dossier_items": None,
            },
            evidence=[SUMMARY],
            note="the recorded gaps; this dossier's own gated items enumerate the rest",
        ),
        entry(
            "independent review result",
            note="S20-740 records the independent review as PENDING; no reviewer has issued a "
            "complete PASS",
        ),
        entry(
            "release decision state",
            value=None,
            note="derived below from the entries and sources; the decision itself is the "
            "operator's and has not been made",
        ),
        entry(
            "confirmation that no push, tag, upload, deployment, publication, or public "
            "announcement occurred without separate authorization",
            value={
                "publication_authorized": summary.get("publication_authorized"),
                "artifact_published": summary.get("artifact"),
                "provenance_publication_authorized": provenance.get("attestation", {}).get(
                    "publication_authorized"
                ),
            },
            evidence=[SUMMARY, PROVENANCE],
            note="every tracked publication flag is false and the dossier records no published "
            "artifact",
        ),
    ]
    gated = [item["item"] for item in entries if item["state"] == "GATED"]
    for item in entries:
        if item["item"] == "evidence gaps":
            item["value"]["gated_dossier_items"] = sorted(gated)
    return entries


def derive_decision(sources: dict, entries: list[dict]) -> tuple[str, list[str]]:
    """The contract section 3 decision state and its exact reasons."""
    summary = sources["summary"]
    register = sources["register"]
    repro = sources["repro"]
    audit = summary.get("s20_710_pre_release_audit", {})
    succession = summary.get("succession", {})
    blocked: list[str] = []
    if register.get("open_reviews"):
        blocked.append(f"{len(register['open_reviews'])} review obligations are open")
    if register.get("deferred_reviews"):
        blocked.append(
            f"{len(register['deferred_reviews'])} review lanes are deferred and unavailable"
        )
    if audit.get("root_license_text_approved") is not True:
        blocked.append("the root license text is not operator-approved")
    if not succession.get("trials_executed"):
        blocked.append("no succession trial has been executed")
    if summary.get("release_candidate_packaging", {}).get("release_check_gate") != "OPEN":
        blocked.append("the release-check and v2 product gates are fail-closed")
    if repro.get("result") != "MULTI_HOST_REPRODUCIBLE":
        blocked.append("only one host has attested the candidate")
    if blocked:
        return "BLOCKED", sorted(blocked)
    declared = register.get("declared_open_findings", {})
    if any(declared.get(severity, 0) for severity in ("p0", "p1", "p2")):
        return "FAIL", ["an open P0, P1, or P2 finding is recorded"]
    if not succession.get("thresholds_pass"):
        return "ALPHA_COMPLETE", ["the succession thresholds do not pass"]
    if summary.get("approved_conditional_items"):
        return "CONDITIONAL_PASS", ["an approved non-correctness item remains"]
    return "PASS", []


def build_dossier() -> dict:
    sources = {
        "summary": load(SUMMARY),
        "repro": load(REPRO, "sley2.reproducibility-report.v1"),
        "cyclonedx": load(CYCLONEDX),
        "spdx": load(SPDX),
        "inventory": load(INVENTORY, "s20-710-pre-release-inventory-v1"),
        "provenance": load(PROVENANCE, "sley2.release-provenance.v1"),
        "conformance": load(CONFORMANCE, "sley2.independent-conformance-report.v1"),
        "register": load(REGISTER, "sley2.finding-register.v1"),
    }
    entries = build_entries(sources)
    state, reasons = derive_decision(sources, entries)
    gates_closed = (
        sources["summary"].get("release_candidate_packaging", {}).get("release_check_gate")
        != "OPEN"
    )
    if state == "PASS" and gates_closed:
        raise DossierError(
            DossierErrorCode.DECISION_INVALID,
            "a PASS decision may not stand while a product gate is fail-closed",
        )
    dossier = {
        "contract": CONTRACT,
        "work_package": "S20-750",
        "project": sources["summary"].get("project"),
        "target_version": sources["summary"].get("target_version"),
        "phase": sources["summary"].get("phase"),
        "entries": entries,
        "evidenced": sum(1 for item in entries if item["state"] == "EVIDENCED"),
        "gated": sum(1 for item in entries if item["state"] == "GATED"),
        "decision_state": state,
        "decision_reasons": reasons,
        "decision_authority": DECISION_AUTHORITY,
        "publication": {
            "authorized": False,
            "push": False,
            "tag": False,
            "upload": False,
            "deployment": False,
            "announcement": False,
        },
        # The digest covers the derived entries, not the source bytes: the
        # machine summary also records this dossier's own counters.
        "entries_digest": digest_of(entries),
    }
    dossier["dossier_digest"] = digest_of(dossier)
    return dossier


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        dossier = build_dossier()
        text = canonical(dossier)
        summary = {
            "entries": len(dossier["entries"]),
            "evidenced": dossier["evidenced"],
            "gated": dossier["gated"],
            "decision_state": dossier["decision_state"],
        }
        if args.check:
            current = DOSSIER.read_text(encoding="utf-8") if DOSSIER.exists() else None
            if current != text:
                print(
                    canonical(
                        {
                            "mode": "check",
                            "result": "FAIL",
                            "code": int(DossierErrorCode.DRIFT),
                            "name": DossierErrorCode.DRIFT.name,
                            "detail": "the tracked dossier differs from the derived dossier",
                        }
                    ),
                    end="",
                )
                return 1
            print(canonical({"mode": "check", "result": "PASS", **summary}), end="")
            return 0
        DOSSIER.parent.mkdir(parents=True, exist_ok=True)
        DOSSIER.write_text(text, encoding="utf-8")
        print(canonical({"mode": "write", "result": "PASS", **summary}), end="")
        return 0
    except DossierError as error:
        print(
            canonical(
                {
                    "result": "FAIL",
                    "code": int(error.code),
                    "name": error.code.name,
                    "detail": error.detail,
                }
            ),
            end="",
        )
        return 1


if __name__ == "__main__":
    sys.exit(main())
