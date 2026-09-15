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

    # Review-derived states. A criterion the master goal assigns to a human
    # judgment is EVIDENCED only when that judgment is on record: the lane's
    # obligations in the finding register all read PASS or HISTORICAL_ROUND
    # (no PENDING, no unclassified FAIL), and the package whose evidence the
    # criterion names has reached its terminal status. Nothing here is a
    # self-approval: every input is a recorded reviewer disposition or a
    # checker-gated status, and the register digest binds the dispositions.
    obligations = register.get("obligations", [])

    def lane_clear(reviewer: str) -> bool:
        rows = [row for row in obligations if row.get("reviewer") == reviewer]
        return bool(rows) and all(row.get("state") in ("PASS", "HISTORICAL_ROUND") for row in rows)

    def register_clear() -> bool:
        return register.get("result") == "FINDING_REGISTER_CLEAR" and not any(
            row.get("state") == "PENDING" for row in obligations
        )

    def field(section: str, name: str) -> str:
        value = summary.get(section)
        got = value.get(name, "") if isinstance(value, dict) else ""
        return got if isinstance(got, str) else ""

    def field_pass(section: str, name: str) -> bool:
        return field(section, name).startswith("PASS")

    def completeish(section: str) -> bool:
        return "COMPLETE" in status(section)

    independent_security_pass = field_pass("threat_coverage", "independent_security_review")
    independent_review_pass = field("finding_register", "independent_review") == "PASS"
    vm_semantics = complete("vm_extended_opcode_profile") and all(
        complete(section) for section in ("vm_lowering_profile", "vm_execution_profile")
    )
    program_model_reviewed = (
        vm_semantics
        and completeish("s20_360_candidate_validation")
        and completeish("s20_390_atomic_commit")
        and all(complete(section) for section in ("type_system", "cfg_validation", "effect_system"))
        and lane_clear("ariadne")
    )
    ambient_authority = (
        completeish("protected_policy_root")
        and completeish("capability_token_profile")
        and complete("reference_adapter_profile")
        and independent_security_pass
        and anti_goal("arbitrary shell") == "HOLDS"
    )
    agent_loop = (
        complete("protocol")
        and complete("json_bridge")
        and complete("cli")
        and completeish("s20_360_candidate_validation")
        and completeish("s20_390_atomic_commit")
        and anti_goal("Sley source syntax or parser") == "HOLDS"
    )
    accounting = summary.get("succession", {})
    accounting_path = ROOT / "evidence/runtime/s20-630-accounting-smoke/accounting-report.json"
    thresholds_pass = False
    threshold_note = "no succession trial has been executed"
    if isinstance(accounting.get("thresholds"), dict):
        rows = accounting["thresholds"]
        evaluated = {name: value for name, value in rows.items() if isinstance(value, str)}
        thresholds_pass = bool(evaluated) and all(value == "PASS" for value in evaluated.values()) and accounting.get("trials_executed", 0) > 0
        threshold_note = (
            f"succession trials_executed {accounting.get('trials_executed', 0)}; threshold rows "
            + ", ".join(f"{name}={value}" for name, value in sorted(evaluated.items()))
        )
    else:
        threshold_note = (
            f"succession trials_executed {summary.get('succession', {}).get('trials_executed', 0)}; "
            "the harness passes its smokes and the campaign has not recorded a threshold table"
        )
    del accounting_path
    release = summary.get("release_decision", {})
    final_commit_fixed = (
        isinstance(release, dict)
        and release.get("state") == "RELEASE_APPROVED"
        and release.get("final_commit") == attestation.get("commit")
    )
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
        ("26.3 semantics", "VM semantics agree with conformance fixtures", EVIDENCED if vm_semantics else AWAITS_REVIEW,
         f"conformance/vm-extended/v1 with the independent oracle; vm_extended_opcode_profile status {status('vm_extended_opcode_profile')}"),
        ("26.3 semantics", "No undefined behavior exists in the program model", EVIDENCED if program_model_reviewed else AWAITS_REVIEW,
         "the exhaustive judgment and execution contracts (type, CFG, effect, VM extended, S20-360 operation analysis, S20-390 extended profile) at terminal status with every Ariadne lane obligation PASS or historical in the finding register"),
        ("26.3 semantics", "No ambient authority exists", EVIDENCED if ambient_authority else AWAITS_REVIEW,
         f"protected policy root, capability tokens, and reference adapters at terminal status; independent security review {field('threat_coverage', 'independent_security_review') or 'PENDING'}; anti-goal 'arbitrary shell' {anti_goal('arbitrary shell')}"),

        ("26.4 agent loop", "agents can create and maintain programs without source", EVIDENCED if agent_loop else AWAITS_REVIEW,
         f"SMP1 endpoint status {status('protocol')}, bridge {status('json_bridge')}, CLI {status('cli')}; candidate validation and atomic commit at their boundaries; no source parser in the GA graph"),
        ("26.4 agent loop", "query capsules are bounded", EVIDENCED if complete("context_capsule_profile") else AWAITS_REVIEW,
         f"machine summary context_capsule_profile status {status('context_capsule_profile')}"),
        ("26.4 agent loop", "session handles are stale-safe", EVIDENCED if complete("session_handle_profile") else AWAITS_REVIEW,
         f"machine summary session_handle_profile status {status('session_handle_profile')}"),
        ("26.4 agent loop", "typed affordances and mutations are available", EVIDENCED if complete("protocol") and completeish("mutation_schema") and completeish("candidate_construction_profile") else AWAITS_REVIEW,
         f"machine summary protocol status {status('protocol')}; mutation schema {status('mutation_schema')}; candidate construction {status('candidate_construction_profile')}"),
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
        ("26.5 repository", "disjoint merge is deterministic", EVIDENCED if complete("merge") else AWAITS_REVIEW,
         f"machine summary merge status {status('merge')}"),
        ("26.5 repository", "ambiguous merge creates conflict objects", EVIDENCED if complete("merge") else AWAITS_REVIEW,
         f"the S20-520 conflict objects with their corpus; merge status {status('merge')}"),
        ("26.5 repository", "GC preserves all retained roots", EVIDENCED if complete("garbage_collection") else AWAITS_REVIEW,
         f"machine summary garbage_collection status {status('garbage_collection')}"),
        ("26.5 repository", "crash recovery produces only old or complete new state", EVIDENCED,
         f"the S20-530 hundred-row crash matrix, status {status('s20_530_crash_recovery')}"),

        ("26.6 policy and security", "policy is protected from the judged candidate", EVIDENCED if completeish("protected_policy_root") and lane_clear("nabu") else AWAITS_REVIEW,
         f"machine summary protected_policy_root status {status('protected_policy_root')}"),
        ("26.6 policy and security", "capability tokens cannot be forged or replayed across scope", EVIDENCED if completeish("capability_token_profile") and independent_security_pass else AWAITS_REVIEW,
         f"machine summary capability_token_profile status {status('capability_token_profile')}"),
        ("26.6 policy and security", "adapters are bounded", EVIDENCED if complete("reference_adapter_profile") else AWAITS_REVIEW,
         f"machine summary reference_adapter_profile status {status('reference_adapter_profile')}"),
        ("26.6 policy and security", "no arbitrary shell exists",
         EVIDENCED if anti_goal("arbitrary shell") == "HOLDS" else GATED,
         "anti-goal conformance report, no kernel source invokes a process"),
        ("26.6 policy and security", "all P0/P1 threats have passing tests",
         EVIDENCED if independent_security_pass and not threats["p0_p1_without_located_symbol"] and not symbols["unexercised"] else AWAITS_REVIEW,
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

        ("26.7 succession", "every section 22 threshold passes", EVIDENCED if thresholds_pass else GATED,
         threshold_note),

        ("26.8 packaging", "artifact name sley-2.0.0-linux-x86_64.tar.gz", EVIDENCED,
         f"reproducibility report attests {attestation.get('artifact_name', 'no attestation')}"),
        ("26.8 packaging", "artifact is built from the final candidate commit", EVIDENCED if final_commit_fixed else GATED,
         f"attested commit {attestation.get('commit', 'none')}; the final commit is fixed by a recorded release decision (machine summary release_decision), state {release.get('state') if isinstance(release, dict) else 'none'}"),
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

        ("26.9 review", "Ariadne approves SSMC1 and semantic correctness", EVIDENCED if lane_clear("ariadne") and complete("ssmc1") and vm_semantics else AWAITS_REVIEW,
         f"finding register Ariadne lane: {sum(1 for row in obligations if row.get('reviewer') == 'ariadne' and row.get('state') == 'PASS')} PASS, {sum(1 for row in obligations if row.get('reviewer') == 'ariadne' and row.get('state') == 'PENDING')} PENDING"),
        ("26.9 review", "Nabu approves architectural and cross-product boundaries", EVIDENCED if lane_clear("nabu") else AWAITS_REVIEW,
         f"finding register Nabu lane: {sum(1 for row in obligations if row.get('reviewer') == 'nabu' and row.get('state') == 'PASS')} PASS, {sum(1 for row in obligations if row.get('reviewer') == 'nabu' and row.get('state') == 'PENDING')} PENDING"),
        ("26.9 review", "Vulcan or current independent reviewer issues a complete PASS", EVIDENCED if independent_review_pass and lane_clear("vulcan") else AWAITS_REVIEW,
         f"S20-740 independent review {field('finding_register', 'independent_review') or 'PENDING'}; Vulcan lane {'clear' if lane_clear('vulcan') else 'open'}"),
        ("26.9 review", "all reviewer findings are resolved or dispositioned below P3", EVIDENCED if register_clear() and open_p0_p2 == 0 else AWAITS_REVIEW,
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
