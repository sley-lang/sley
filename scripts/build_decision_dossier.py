#!/usr/bin/env python3
"""S20-750 decision dossier: every master-goal section 30 item and the state.

Derives `evidence/release/decision-dossier.json` from the machine summary and
the tracked S20-710 full, S20-720, S20-730, and S20-740 reports. It reports;
it never decides, approves, publishes, or opens a product gate.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sys
from enum import IntEnum
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GA_BUILDER = ROOT / "scripts/build_ga_acceptance_report.py"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
REPRO = ROOT / "evidence/release/reproducibility-report.json"
CYCLONEDX = ROOT / "evidence/release/sbom/cyclonedx-1.6.json"
SPDX = ROOT / "evidence/release/sbom/spdx-2.3.json"
INVENTORY = ROOT / "evidence/security/T52/pre-release-inventory.json"
PROVENANCE = ROOT / "evidence/release/provenance.json"
CONFORMANCE = ROOT / "evidence/conformance/independent-conformance-report.json"
REGISTER = ROOT / "evidence/review/finding-register.json"
TEST_INVENTORY = ROOT / "evidence/validation/test-inventory.json"
THREAT_COVERAGE = ROOT / "evidence/security/threat-coverage-report.json"
GA_ACCEPTANCE = ROOT / "evidence/release/ga-acceptance-report.json"
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


_GA_BUILDER = None


def ga_builder():
    """The GA report builder, loaded by path so its predicates are shared.

    The complete-PASS form, the register-row verdict test, and the section
    22 threshold derivation are defined once there; the dossier reads them
    rather than re-implementing them (contract section 2.1).
    """
    global _GA_BUILDER
    if _GA_BUILDER is None:
        spec = importlib.util.spec_from_file_location("build_ga_acceptance_report", GA_BUILDER)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        _GA_BUILDER = module
    return _GA_BUILDER


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


def required(mapping: object, key: str, what: str) -> object:
    """A structural key the dossier cannot mean anything without.

    A missing or renamed key in a present source fails loudly with
    `DOSSIER_SOURCE_INVALID` so a regrade from `EVIDENCED` to `GATED`
    can never read as resolved.
    """
    if not isinstance(mapping, dict) or key not in mapping:
        raise DossierError(
            DossierErrorCode.SOURCE_INVALID, f"{what}: missing required key {key!r}"
        )
    return mapping[key]


def entry(item: str, *, value=None, evidence: list[Path] | None = None, note: str) -> dict:
    """One section 30 entry; a value makes it EVIDENCED, its absence GATED.

    `EVIDENCED` means every fact of the item is present: a null value, or
    an object whose fields are all null, carries no facts. An all-null
    object reads `GATED` with no value (a gated item is never given one);
    the note records which fields were null. Every cited evidence path
    must exist; a cited-but-absent file is `DOSSIER_SOURCE_MISSING`,
    never a quiet note.
    """
    if isinstance(value, dict) and value and all(field is None for field in value.values()):
        note = f"{note} (all {len(value)} fields null: {', '.join(sorted(value))})"
        value = None
    state = "EVIDENCED" if value is not None else "GATED"
    for path in evidence or []:
        if not path.exists():
            raise DossierError(
                DossierErrorCode.SOURCE_MISSING, f"{item}: cited evidence absent {display(path)}"
            )
    return {
        "item": item,
        "state": state,
        "value": value,
        "evidence": sorted(display(path) for path in (evidence or [])),
        "note": note,
    }


def build_entries(sources: dict) -> list[dict]:
    summary = sources["summary"]
    repro = sources["repro"]
    register = sources["register"]
    conformance = sources["conformance"]
    license_inventory = sources["inventory"]
    bom = sources["cyclonedx"]
    provenance = sources["provenance"]
    test_inventory = sources["inventory_of_tests"]
    threats = sources["threat_coverage"]
    acceptance = sources["ga_acceptance"]
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

    shared = ga_builder()
    # Item 15: the recorded security verdict evidences only through the
    # register's classifier (row state PASS, complete PASS form, neither
    # unclaimed nor unclassified). The transcript the summary note names is
    # cited so its absence fails the build, never a quiet note.
    security_field = summary.get("threat_coverage", {}).get("independent_security_review")
    security_pass, security_reason = shared.verdict_complete_pass(
        register, "threat_coverage", "independent_security_review"
    )
    security_note = str(summary.get("threat_coverage", {}).get("independent_security_review_note", ""))
    transcripts = [ROOT / match for match in shared.TRANSCRIPT.findall(security_note)]
    security_evidence = [SUMMARY, THREAT_COVERAGE, REGISTER, *transcripts]
    # Item 32: a complete PASS from the independent reviewer over a register
    # whose result is CLEAR, the same definition GA criterion 26.9.3 reads.
    independent_review = summary.get("finding_register", {}).get("independent_review")
    register_result = register.get("result")
    independent_pass = (
        shared.complete_pass_form(independent_review) and register_result == "FINDING_REGISTER_CLEAR"
    )

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
            value={
                "property_tests": test_inventory["property_tests"],
                "property_test_detail": test_inventory["property_test_detail"],
                "rust_unit_tests": test_inventory["rust_unit_tests"],
                "rust_ignored_emitters": test_inventory["rust_ignored_emitters"],
                "python_tests": test_inventory["python_tests"],
                "persistent_fuzz_targets": test_inventory["persistent_fuzz_target_count"],
                "conformance_vectors": test_inventory["conformance_vectors"],
                "conformance_rejections": test_inventory["conformance_rejections"],
            },
            evidence=[TEST_INVENTORY],
            note="the property-test count is a counted zero: the inventory scanned "
            "the workspace and crate manifests, the lockfile, and every Rust and "
            "Python test source and found no proptest, quickcheck, or hypothesis "
            "harness. The unit-test, fuzz-target, and vector counts are adjacent "
            "corpus facts, not property tests. The inventory describes the corpus "
            "and runs nothing, so a passing run is separate evidence",
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
            value=security_field if security_pass else None,
            evidence=security_evidence if security_pass else (transcripts if transcripts else []),
            note=f"the independent security review is Vulcan's; the machine summary records it as "
            f"{security_field or 'PENDING'} and the finding register classifies it: {security_reason}. "
            "The item is evidenced only by a complete PASS (the bare PASS token or an all-zero enumerated "
            "count form the register classifies PASS, with no unclaimed finding); the transcript the "
            f"summary note names ({', '.join(display(path) for path in transcripts) or 'none named'}) is "
            "cited as evidence and must exist. Its input is measured: of the "
            f"{threats['threat_count']} registered threats, "
            f"{threats['states'].get('SYMBOL_REALIZED_WITH_EXERCISE', 0)} have a located "
            f"control whose symbol a test region, test file, corpus, fuzz target, or oracle names, "
            f"{threats['states'].get('PLANNED_EVIDENCE_PRESENT', 0)} carry "
            f"their planned evidence directory, and "
            f"{len(threats['p0_p1_without_located_symbol'])} P0 or P1 threats have no located "
            "symbol and form the review's work list "
            "(evidence/security/threat-coverage-report.json; a located symbol proves the "
            "named control exists, not that the threat is mitigated)",
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
            note="the attested artifact digest; the provenance subject must "
            "name it (checker: provenance:subject-attestation-mismatch), it "
            "is not assumed to repeat it",
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
            # Derived from the live report, never asserted: a fresh
            # candidate is single-host until a second attestation earns
            # MULTI_HOST_REPRODUCIBLE.
            note="multi-host reproducibility across distinct attested hosts"
            if repro.get("result") == "MULTI_HOST_REPRODUCIBLE"
            else "single-host reproducibility; the second host is an operator-gated lane",
        ),
        entry(
            "SBOM and license inventory",
            value={
                "cyclonedx_spec_version": bom.get("specVersion"),
                "spdx_version": sources["spdx"].get("spdxVersion"),
                "components": len(bom.get("components", [])),
                "license_disposition_blocked": sum(
                    1
                    for package in license_inventory.get("packages", [])
                    if str(package.get("license_disposition", "")).startswith("BLOCKED")
                ),
                "root_license_text_approved": audit.get("root_license_text_approved"),
            },
            evidence=[CYCLONEDX, SPDX, INVENTORY],
            note="both standards documents exist as drafts; the root license text is "
            "operator-approved, which the documents state in band"
            if audit.get("root_license_text_approved") is True
            else "both standards documents exist as drafts; the root license text is unapproved, "
            "which the documents state in band",
        ),
        entry(
            "findings by severity and disposition",
            value={
                "obligations": register.get("obligation_count"),
                "states": register.get("states"),
                "severity_mentions": register.get("severity_mentions"),
                "declared_open_findings": register.get("declared_open_findings"),
                "open_reviews": len(register.get("open_reviews", [])),
                "deferred_reviews": len(register.get("deferred_reviews", [])),
                "unclassified": len(register.get("unclassified", [])),
                "unclaimed_carried_findings": len(register.get("unclaimed_carried_findings", [])),
                "result": register.get("result"),
            },
            evidence=[REGISTER],
            note="the S20-740 register of every recorded review obligation: pending, deferred, "
            "unclassified (OTHER), and unclaimed-carried rows are counted, and the register's own "
            "result is carried so the decision rule reads it",
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
            value=(independent_review if independent_pass else None),
            evidence=[SUMMARY, REGISTER] if independent_pass else [],
            note=f"S20-740 records the independent review as {independent_review or 'PENDING'} over a "
            f"register whose result is {register_result}; the item is evidenced only by a recorded "
            "complete PASS (the bare PASS token or an all-zero enumerated count form the register's "
            "classifier reads PASS) from the independent reviewer while the register result is "
            "FINDING_REGISTER_CLEAR, the definition GA criterion 26.9.3 shares",
        ),
        entry(
            "release decision state",
            value=None,
            note="derived below from the entries and sources; the decision itself is the "
            "operator's and has not been made. The master goal's section 26 criteria stand at "
            f"{acceptance['states'].get('EVIDENCED', 0)} evidenced, "
            f"{acceptance['states'].get('AWAITS_REVIEW', 0)} awaiting review, and "
            f"{acceptance['states'].get('GATED', 0)} gated of {acceptance['criterion_count']} "
            "(evidence/release/ga-acceptance-report.json)",
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


ARM_ENTRIES = (
    "strict correctness by arm",
    "ACT by arm",
    "context bytes and model tokens by arm",
    "repair loops by arm",
    "invalid committed states",
    "stale candidates incorrectly accepted",
)


GATE_NAMES = ("release-check", "v2")


def gate_results(sources: dict) -> dict[str, str]:
    """The product-gate states the builder may rely on.

    Injected `sources["gates"]` (unit tests, production `build_dossier`)
    is dual-sourced against the summary hand field by the caller; a sources
    mapping with no `gates` key predates the wiring and falls back to the
    summary field alone, documented here and never in production.
    """
    injected = sources.get("gates")
    if injected is None:
        return {}
    if not isinstance(injected, dict):
        raise DossierError(DossierErrorCode.SOURCE_INVALID, "decision sources: gates is not an object")
    return {name: str(injected.get(name, "UNKNOWN")) for name in GATE_NAMES}


def read_gate_status(name: str) -> str:
    """One gate's state from the fail-closed stub; `OPEN` opens, anything else closes."""
    import subprocess

    try:
        completed = subprocess.run(
            [sys.executable, "scripts/gate_status.py", name],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        return str(json.loads(completed.stdout).get("result", "UNKNOWN"))
    except (OSError, ValueError):
        return "UNKNOWN"


def open_p2_rows(register: object, approved: set[str]) -> list[str]:
    """Open P2 obligation rows no approval covers, as `section:field` names.

    The register carries review rows, not per-finding identities, so an
    approval names the `section:field` row whose P2s it covers. A row is
    open for P2 purposes while it mentions P2, is neither a passing row nor
    a historical round (a FAIL/REVISE the register folds under a same-lane
    superseding PASS), and does not itself declare no open findings.
    """
    if not isinstance(register, dict):
        return ["the finding register is unavailable, so P2 approval is unverifiable"]
    rows = register.get("obligations")
    if not isinstance(rows, list):
        return ["the finding register has no obligation list, so P2 approval is unverifiable"]
    uncovered = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        if "P2" not in row.get("severities", []):
            continue
        if row.get("state") in ("PASS", "HISTORICAL_ROUND") or row.get("declares_no_open_p0_p1_p2") is True:
            continue
        name = f"{row.get('section')}:{row.get('field')}"
        if name not in approved:
            uncovered.append(name)
    return sorted(uncovered)


def enforce_pass_guard(state: str, gates_closed: bool) -> None:
    """A `PASS` may never stand while a product gate is fail-closed.

    Unreachable by construction (a closed gate always blocks first), kept
    so a future narrowing of the `BLOCKED` rules cannot silently admit
    `PASS` behind closed gates. Raises `DOSSIER_DECISION_INVALID`.
    """
    if state == "PASS" and gates_closed:
        raise DossierError(
            DossierErrorCode.DECISION_INVALID,
            "a PASS decision may not stand while a product gate is fail-closed",
        )


def derive_decision(sources: dict, entries: list[dict]) -> tuple[str, list[str]]:
    """The contract section 3 decision state and its exact reasons.

    Every BLOCKED and FAIL reason is read off the entries named in the
    contract's section 3 mapping, not re-derived from the sources behind the
    entries' backs: a missing decision-input entry fails closed, and a gated
    entry contributes its gated fact. Four inputs have no section 30 item that
    carries them (the release-check and v2 gate states, the succession
    thresholds derived from the tracked S20-630 accounting report, the GA
    acceptance states, and the approved conditional items), so those four
    rules read the tracked sources the contract names; everything else comes
    from the entries.
    """
    summary = sources["summary"]
    if not isinstance(summary, dict):
        raise DossierError(DossierErrorCode.SOURCE_INVALID, "decision sources: summary is not an object")
    by_item = {item["item"]: item for item in entries}
    blocked: list[str] = []

    def missing(item: str) -> bool:
        if item not in by_item:
            blocked.append(f"no {item} entry to derive from")
            return True
        return False

    def evidenced_value(item: str) -> dict | None:
        if missing(item):
            return None
        entry = by_item[item]
        if entry["state"] != "EVIDENCED":
            return None
        value = entry["value"]
        return value if isinstance(value, dict) else None

    findings = evidenced_value("findings by severity and disposition")
    if findings is not None:
        # Rule 1 "any open review obligation": PENDING rows, OTHER
        # (unclassified) rows, and the register's own result are all read
        # off the entry; a register that is not CLEAR blocks by itself.
        if findings.get("open_reviews"):
            blocked.append(f"{findings['open_reviews']} review obligations are open")
        if findings.get("deferred_reviews"):
            blocked.append(
                f"{findings['deferred_reviews']} review lanes are deferred and unavailable"
            )
        if findings.get("unclassified"):
            blocked.append(
                f"{findings['unclassified']} review dispositions are unclassified"
            )
        if findings.get("result") != "FINDING_REGISTER_CLEAR":
            blocked.append(
                f"the finding register result is {findings.get('result')}, not FINDING_REGISTER_CLEAR"
            )
    elif "findings by severity and disposition" in by_item:
        blocked.append("the findings entry is gated, so openness is unknown")

    license_entry = evidenced_value("SBOM and license inventory")
    if license_entry is not None:
        if license_entry.get("root_license_text_approved") is not True:
            blocked.append("the root license text is not operator-approved")
    elif "SBOM and license inventory" in by_item:
        blocked.append("the license entry is gated, so approval is unknown")

    arm_states = []
    for item in ARM_ENTRIES:
        if missing(item):
            continue
        arm_states.append(by_item[item]["state"])
    if arm_states and all(state == "GATED" for state in arm_states):
        blocked.append("no succession trial has been executed")

    gates = gate_results(sources)
    gate_failed = [name for name, result in gates.items() if result == "FAILED"]
    gate_unimplemented = [name for name, result in gates.items() if result not in ("OPEN", "FAILED")]
    gates_closed = bool(gate_unimplemented)
    summary_gate = summary.get("release_candidate_packaging", {})
    summary_gate_state = summary_gate.get("release_check_gate") if isinstance(summary_gate, dict) else None
    if summary_gate_state is not None and not isinstance(summary_gate_state, str):
        raise DossierError(DossierErrorCode.SOURCE_INVALID, "release_check_gate is not a string")
    if summary_gate_state != "OPEN":
        gates_closed = True
    if gates_closed:
        blocked.append("the release-check and v2 product gates are fail-closed")

    repro_entry = evidenced_value("reproducibility result")
    if repro_entry is not None:
        if repro_entry.get("result") != "MULTI_HOST_REPRODUCIBLE":
            blocked.append("only one host has attested the candidate")
    elif "reproducibility result" in by_item:
        blocked.append("the reproducibility entry is gated, so attestation is unknown")

    acceptance = sources.get("ga_acceptance")
    if isinstance(acceptance, dict):
        states = acceptance.get("states", {})
        if isinstance(states, dict):
            pending = sum(
                states.get(name, 0) for name in ("AWAITS_REVIEW", "GATED")
            )
            if pending:
                blocked.append(f"{pending} GA acceptance criteria are not evidenced")

    if blocked:
        return "BLOCKED", sorted(blocked)
    declared = (findings or {}).get("declared_open_findings", {})
    if any(declared.get(severity, 0) for severity in ("p0", "p1")):
        return "FAIL", ["an open P0 or P1 finding is recorded"]
    if declared.get("p2", 0):
        approved = summary.get("approved_conditional_items")
        if approved is not None and not isinstance(approved, list):
            raise DossierError(
                DossierErrorCode.SOURCE_INVALID, "approved_conditional_items is not a list"
            )
        uncovered = open_p2_rows(sources.get("register"), set(approved or []))
        if uncovered:
            return "FAIL", [f"an unapproved P2 finding is recorded: {name}" for name in uncovered]
    if gate_failed:
        return "FAIL", [f"a required gate failed: {name}" for name in sorted(gate_failed)]
    # The one section 22 threshold verdict, derived by the GA builder from
    # the tracked S20-630 accounting report; no summary hand key is read, and
    # an absent verdict is a non-passing one.
    thresholds = sources.get("thresholds")
    if thresholds is not None and not isinstance(thresholds, dict):
        raise DossierError(DossierErrorCode.SOURCE_INVALID, "decision sources: thresholds is not an object")
    passed = thresholds.get("pass") if isinstance(thresholds, dict) else None
    if passed is not None and not isinstance(passed, bool):
        raise DossierError(DossierErrorCode.SOURCE_INVALID, "thresholds pass is not a boolean")
    if not passed:
        why = thresholds.get("note") if isinstance(thresholds, dict) else "no threshold verdict was derived"
        return "ALPHA_COMPLETE", [f"the succession thresholds do not pass: {why}"]
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
        "inventory_of_tests": load(TEST_INVENTORY, "sley2.test-inventory.v1"),
        "threat_coverage": load(THREAT_COVERAGE, "sley2.threat-coverage-report.v1"),
        "ga_acceptance": load(GA_ACCEPTANCE, "sley2.ga-acceptance-report.v1"),
    }
    # Structural keys the dossier cannot mean anything without: absence is
    # malformation, never silent gating.
    summary = sources["summary"]
    project = required(summary, "project", "machine-summary")
    target_version = required(summary, "target_version", "machine-summary")
    phase = required(summary, "phase", "machine-summary")
    # The SBOM documents carry no contract tag, so their shapes are checked
    # here: a malformed SBOM is SOURCE_INVALID, not a silent zero.
    bom = sources["cyclonedx"]
    if bom.get("bomFormat") != "CycloneDX" or not isinstance(bom.get("components"), list):
        raise DossierError(DossierErrorCode.SOURCE_INVALID, "cyclonedx: unexpected shape")
    spdx = sources["spdx"]
    if not isinstance(spdx.get("spdxVersion"), str) or not isinstance(spdx.get("packages"), list):
        raise DossierError(DossierErrorCode.SOURCE_INVALID, "spdx: unexpected shape")
    # The GA report is a decision input (rule 1 count, rule 5 gate): its
    # digest must verify, and it must be bound to the register this dossier
    # reads, or a hand-edited or stale report could remove a BLOCKED reason.
    shared = ga_builder()
    acceptance = sources["ga_acceptance"]
    if not shared.verify_report_digest(acceptance):
        raise DossierError(
            DossierErrorCode.SOURCE_INVALID,
            "ga-acceptance-report.json: report_digest does not verify against the report body",
        )
    if acceptance.get("register_digest") != sources["register"].get("register_digest"):
        raise DossierError(
            DossierErrorCode.SOURCE_INVALID,
            "ga-acceptance-report.json: register_digest differs from the finding register read "
            "(rebuild the GA report after the register)",
        )
    sources["thresholds"] = shared.succession_thresholds()
    # Gate authority is dual-sourced: the live stub runs plus the summary
    # hand field. A hand edit clearing the field cannot clear a gate the
    # stub still reports closed.
    sources["gates"] = {name: read_gate_status(name) for name in GATE_NAMES}
    entries = build_entries(sources)
    state, reasons = derive_decision(sources, entries)
    gates_closed = any(result != "OPEN" for result in sources["gates"].values())
    packaging = summary.get("release_candidate_packaging", {})
    if not isinstance(packaging, dict) or packaging.get("release_check_gate") != "OPEN":
        gates_closed = True
    enforce_pass_guard(state, gates_closed)
    dossier = {
        "contract": CONTRACT,
        "work_package": "S20-750",
        "project": project,
        "target_version": target_version,
        "phase": phase,
        "gates": dict(sources["gates"]),
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
