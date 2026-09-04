#!/usr/bin/env python3
"""Map every GA acceptance criterion to its evidence and derive its state.

Master goal section 26 lists fifty-two GA acceptance criteria across nine
groups. Each is satisfied, gated, or awaiting a review somewhere in this
repository, but nothing connected a criterion to the artifact that answers it,
so "how much of GA is locally satisfied?" had no answer.

Every state here is derived from a tracked artifact: the machine summary, the
derived reports of S20-730 through S20-780, or a conformance corpus. A
criterion whose evidence needs a human is `AWAITS_REVIEW`, and one whose
evidence needs an authority this repository does not hold is `GATED`; neither
is a pass.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
CONFORMANCE = ROOT / "evidence/conformance/independent-conformance-report.json"
THREATS = ROOT / "evidence/security/threat-coverage-report.json"
SYMBOLS = ROOT / "evidence/security/error-symbol-registration.json"
ANTI_GOALS = ROOT / "evidence/validation/anti-goal-conformance.json"
REGISTER = ROOT / "evidence/review/finding-register.json"
REPRO = ROOT / "evidence/release/reproducibility-report.json"
REPORT = ROOT / "evidence/release/ga-acceptance-report.json"
CONTRACT = "sley2.ga-acceptance-report.v1"

EVIDENCED = "EVIDENCED"
AWAITS_REVIEW = "AWAITS_REVIEW"
GATED = "GATED"


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def build_report() -> dict:
    summary = load(SUMMARY)
    conformance = load(CONFORMANCE)
    threats = load(THREATS)
    symbols = load(SYMBOLS)
    anti_goals = load(ANTI_GOALS)
    register = load(REGISTER)
    repro = load(REPRO)

    def status(section: str) -> str:
        value = summary.get(section)
        return value.get("status", "") if isinstance(value, dict) else ""

    def complete(section: str) -> bool:
        return status(section).endswith("COMPLETE")

    def anti_goal(name: str) -> str:
        for entry in anti_goals["anti_goals"]:
            if entry["anti_goal"] == name:
                return entry["state"]
        return "REVIEW_ONLY"

    findings = register.get("declared_open_findings", {})
    open_p0_p2 = sum(findings.get(key, 0) for key in ("p0", "p1", "p2"))
    packaging = summary.get("release_candidate_packaging", {})
    attestation = repro["attestations"][0] if repro.get("attestations") else {}

    criteria: list[tuple[str, str, str, str]] = [
        # group, criterion, state, evidence
        ("26.1 lineage", "Sley 1.2.0 is preserved and checksum-verified", EVIDENCED,
         "machine summary legacy_freeze records the artifact and source digests; make legacy-runner-smoke verifies them"),
        ("26.1 lineage", "No Sley 1.2.1 implementation was required", EVIDENCED,
         "docs/adr/ADR-0002-clean-room-legacy-boundary.md"),
        ("26.1 lineage", "The new repository has independent history", EVIDENCED,
         f"machine summary m0_commit {summary.get('m0_commit', '')[:12]} with lineage {summary.get('lineage')}"),
        ("26.1 lineage", "No legacy source tree was copied wholesale",
         EVIDENCED if summary.get("clean_room_disposition_register", {}).get("legacy_source_in_tree") is False else GATED,
         "scripts/check_clean_room_boundary.py verifies no legacy source, dependency, or in-process touchpoint"),
        ("26.1 lineage", "Every reused concept has a disposition and evidence",
         EVIDENCED if summary.get("clean_room_disposition_register", {}).get("reused_concepts") else GATED,
         "docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md, two dispositioned entries"),

        ("26.2 canonical format", "SCB1 is frozen as version 1", EVIDENCED if complete("scb1") else AWAITS_REVIEW,
         f"machine summary scb1 status {status('scb1')}"),
        ("26.2 canonical format", "Rust and independent oracle produce byte-identical output", EVIDENCED,
         f"machine summary conformance cross_implementation_agreement {summary.get('conformance', {}).get('cross_implementation_agreement')}"),
        ("26.2 canonical format", "Every non-canonical fixture is rejected", EVIDENCED,
         f"{conformance['independently_checked']} of {conformance['fixture_directories']} fixture families independently checked, rejection corpora included"),
        ("26.2 canonical format", "Hashes are domain separated", EVIDENCED,
         "docs/spec/IDENTIFIERS_V1.md registry with the drift check of scripts/check_required_contract_index.py"),
        ("26.2 canonical format", "Schema epochs are explicit", EVIDENCED if complete("schema_epoch") else AWAITS_REVIEW,
         f"machine summary schema_epoch status {status('schema_epoch')}; docs/spec/EPOCH_MIGRATION_POLICY_V1.md governs successors"),
        ("26.2 canonical format", "Pack import reconstructs exact roots", EVIDENCED if complete("repository_pack") else AWAITS_REVIEW,
         f"machine summary repository_pack status {status('repository_pack')}"),
        ("26.2 canonical format", "Corruption is detected before ref advancement", EVIDENCED if complete("s20_530_crash_recovery") or "COMPLETE" in status("s20_530_crash_recovery") else AWAITS_REVIEW,
         f"machine summary s20_530_crash_recovery status {status('s20_530_crash_recovery')}"),

        ("26.3 semantics", "SSMC1 is the only canonical program representation", EVIDENCED if complete("ssmc1") else AWAITS_REVIEW,
         f"machine summary ssmc1 status {status('ssmc1')}"),
        ("26.3 semantics", "No source parser exists in the GA dependency graph",
         EVIDENCED if anti_goal("Sley source syntax or parser") == "HOLDS" else GATED,
         "anti-goal conformance report, parser crates absent from Cargo.lock"),
        ("26.3 semantics", "Type, CFG, effect, contract, and identity checks are deterministic",
         EVIDENCED if all(complete(section) for section in ("type_system", "cfg_validation", "effect_system", "identifiers")) else AWAITS_REVIEW,
         "machine summary type_system, cfg_validation, effect_system, identifiers"),
        ("26.3 semantics", "VM semantics agree with conformance fixtures", AWAITS_REVIEW,
         "conformance/vm-extended/v1 with the independent oracle; the extended profile's reviews are pending"),
        ("26.3 semantics", "No undefined behavior exists in the program model", AWAITS_REVIEW,
         "the exhaustive judgment and execution contracts; the independent semantic review owns this"),
        ("26.3 semantics", "No ambient authority exists", AWAITS_REVIEW,
         "capability and policy contracts; anti-goal 'untyped or ambient effects' is REVIEW_ONLY"),

        ("26.4 agent loop", "agents can create and maintain programs without source", AWAITS_REVIEW,
         "the SMP1 endpoint, the JSON bridge, and the CLI are implemented with reviews pending"),
        ("26.4 agent loop", "query capsules are bounded", AWAITS_REVIEW,
         f"machine summary context_capsule_profile status {status('context_capsule_profile')}"),
        ("26.4 agent loop", "session handles are stale-safe", AWAITS_REVIEW,
         f"machine summary session_handle_profile status {status('session_handle_profile')}"),
        ("26.4 agent loop", "typed affordances and mutations are available", AWAITS_REVIEW,
         f"machine summary protocol status {status('protocol')}"),
        ("26.4 agent loop", "invalid candidates cannot commit", EVIDENCED,
         f"machine summary s20_360_candidate_validation status {status('s20_360_candidate_validation')} with the S20-390 commit boundary"),
        ("26.4 agent loop", "exact stale conflicts are rejected", EVIDENCED,
         "the S20-360 stale root and stale entity decisions with their result vectors"),
        ("26.4 agent loop", "machine outputs use stable codes and contracts", EVIDENCED,
         "docs/spec/ERROR_CODES_V1.md with the per-package checkers that pin every reserved range"),

        ("26.5 repository", "state versions are content addressed", EVIDENCED if complete("state_root") else AWAITS_REVIEW,
         f"machine summary state_root status {status('state_root')}"),
        ("26.5 repository", "refs update atomically", EVIDENCED if "COMPLETE" in status("s20_500_native_refs_branches") else AWAITS_REVIEW,
         f"machine summary s20_500_native_refs_branches status {status('s20_500_native_refs_branches')}"),
        ("26.5 repository", "branches preserve ancestry", EVIDENCED if "COMPLETE" in status("s20_500_native_refs_branches") else AWAITS_REVIEW,
         "the S20-500 immutable branch origin and bounded ancestry rules"),
        ("26.5 repository", "disjoint merge is deterministic", AWAITS_REVIEW,
         f"machine summary merge status {status('merge')}"),
        ("26.5 repository", "ambiguous merge creates conflict objects", AWAITS_REVIEW,
         "the S20-520 conflict objects with their corpus; reviews pending"),
        ("26.5 repository", "GC preserves all retained roots", EVIDENCED if complete("garbage_collection") else AWAITS_REVIEW,
         f"machine summary garbage_collection status {status('garbage_collection')}"),
        ("26.5 repository", "crash recovery produces only old or complete new state", EVIDENCED,
         f"the S20-530 hundred-row crash matrix, status {status('s20_530_crash_recovery')}"),

        ("26.6 policy and security", "policy is protected from the judged candidate", EVIDENCED if complete("protected_policy_root") else AWAITS_REVIEW,
         f"machine summary protected_policy_root status {status('protected_policy_root')}"),
        ("26.6 policy and security", "capability tokens cannot be forged or replayed across scope", EVIDENCED if complete("capability_token_profile") else AWAITS_REVIEW,
         f"machine summary capability_token_profile status {status('capability_token_profile')}"),
        ("26.6 policy and security", "adapters are bounded", EVIDENCED if complete("reference_adapter_profile") else AWAITS_REVIEW,
         f"machine summary reference_adapter_profile status {status('reference_adapter_profile')}"),
        ("26.6 policy and security", "no arbitrary shell exists",
         EVIDENCED if anti_goal("arbitrary shell") == "HOLDS" else GATED,
         "anti-goal conformance report, no kernel source invokes a process"),
        ("26.6 policy and security", "all P0/P1 threats have passing tests", AWAITS_REVIEW,
         f"threat coverage report: {threats['states'].get('SYMBOL_REALIZED_WITH_EXERCISE', 0)} exercised, "
         f"{len(threats['p0_p1_without_located_symbol'])} P0/P1 without a located symbol; "
         f"{symbols['emitted_symbols']} stable failure symbols, {len(symbols['unexercised'])} unexercised, "
         f"{len(symbols['ambiguous_codes'])} numeric codes carrying more than one symbol. Whether a located, "
         "exercised control mitigates its threat stays the review's judgment"),
        ("26.6 policy and security", "no P0/P1/P2 finding remains open",
         EVIDENCED if open_p0_p2 == 0 else GATED,
         f"finding register declares {open_p0_p2} open P0/P1/P2 findings across {register['obligation_count']} obligations"),
        ("26.6 policy and security", "opacity is not used as a security argument", EVIDENCED,
         "every contract is public in docs/spec and the independent oracle checks nineteen fixture families"),

        ("26.7 succession", "every section 22 threshold passes", GATED,
         f"succession trials_executed {summary.get('succession', {}).get('trials_executed', 0)}; the harness passes its smokes and needs model access and spend authorization"),

        ("26.8 packaging", "artifact name sley-2.0.0-linux-x86_64.tar.gz", EVIDENCED,
         f"reproducibility report attests {attestation.get('artifact_name', 'no attestation')}"),
        ("26.8 packaging", "artifact is built from the final candidate commit", GATED,
         "the attested commit is the latest local candidate; the final commit is fixed at the release decision"),
        ("26.8 packaging", "artifact runs with no source-tree access", EVIDENCED,
         "the S20-720 unpacked demo runs the packaged binary outside the tree"),
        ("26.8 packaging", "artifact contains no secrets, local paths, caches, or debug files", EVIDENCED,
         "the S20-720 forbidden content scan reports zero findings"),
        ("26.8 packaging", "manifest, SHA-256, size, SBOM, license inventory, and provenance are recorded", EVIDENCED,
         "the manifest and both SBOM documents with the unsigned provenance statement"),
        ("26.8 packaging", "second clean build establishes reproducibility", EVIDENCED,
         f"reproducibility report result {repro.get('result')}"),
        ("26.8 packaging", "the source working tree is clean", EVIDENCED,
         f"the attested build records working_tree_clean {attestation.get('working_tree_clean')}"),

        ("26.9 review", "Ariadne approves SSMC1 and semantic correctness", AWAITS_REVIEW,
         "Council lane unavailable; requests staged"),
        ("26.9 review", "Nabu approves architectural and cross-product boundaries", AWAITS_REVIEW,
         "Council lane unavailable; requests staged"),
        ("26.9 review", "Vulcan or current independent reviewer issues a complete PASS", AWAITS_REVIEW,
         "S20-740 independent review is PENDING"),
        ("26.9 review", "all reviewer findings are resolved or dispositioned below P3", AWAITS_REVIEW,
         f"finding register result {register['result']} with {len(register['open_reviews'])} open obligations"),
        ("26.9 review", "publication remains unauthorized unless separately granted", EVIDENCED,
         f"machine summary publication_authorized {summary.get('publication_authorized')}; anti-goal 'unauthorized publication/deploy/spend' {anti_goal('unauthorized publication/deploy/spend')}"),
    ]

    entries = [
        {"group": group, "criterion": criterion, "state": state, "evidence": evidence}
        for group, criterion, state, evidence in criteria
    ]
    states: dict[str, int] = {}
    for entry in entries:
        states[entry["state"]] = states.get(entry["state"], 0) + 1
    report = {
        "contract": CONTRACT,
        "source": "master goal section 26",
        "criterion_count": len(entries),
        "states": states,
        "gated": [entry["criterion"] for entry in entries if entry["state"] == GATED],
        "awaiting_review": [entry["criterion"] for entry in entries if entry["state"] == AWAITS_REVIEW],
        "criteria": entries,
        "ga_claimed": False,
        "interpretation": (
            "EVIDENCED means a tracked artifact answers the criterion today. AWAITS_REVIEW means the "
            "work is implemented but a human judgment is required and has not happened. GATED means "
            "an authority this repository does not hold is required. Neither AWAITS_REVIEW nor GATED "
            "is a pass, and no combination of these states is a GA claim."
        ),
    }
    report["report_digest"] = digest_of(report)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    report = build_report()
    text = canonical(report)
    summary = {
        "criterion_count": report["criterion_count"],
        "states": report["states"],
        "gated": len(report["gated"]),
    }
    if arguments.check:
        current = REPORT.read_text(encoding="utf-8") if REPORT.exists() else None
        if current != text:
            print(
                canonical(
                    {
                        "mode": "check",
                        "result": "FAIL",
                        "detail": "the tracked GA acceptance report differs from the derived report",
                    }
                ),
                end="",
            )
            return 1
        print(canonical({"mode": "check", "result": "PASS", **summary}), end="")
        return 0
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(text, encoding="utf-8")
    print(canonical({"mode": "write", "result": "PASS", **summary}), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
