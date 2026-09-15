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
is a pass. No criterion is a constant: each state is a function of the fact
its evidence string names, and a fact that is absent or contrary reads
`AWAITS_REVIEW` or `GATED`, never `EVIDENCED` (DECISION_DOSSIER_V1 section
2.1).

The predicates the finding register owns are not re-implemented here: package
completion is the register's `complete_packages`, a lane is clear only when
none of its rows is pending, unclaimed, or unclassified in the register, and a
recorded verdict evidences only when the register's token classifier reads it
`PASS` in a complete form (the bare `PASS` token or an all-zero enumerated
count form). The register digest is recorded so the dossier can bind the
dispositions this report was derived from.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from bench.accounting import report as accounting

SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
CONFORMANCE = ROOT / "evidence/conformance/independent-conformance-report.json"
THREATS = ROOT / "evidence/security/threat-coverage-report.json"
SYMBOLS = ROOT / "evidence/security/error-symbol-registration.json"
ANTI_GOALS = ROOT / "evidence/validation/anti-goal-conformance.json"
REGISTER = ROOT / "evidence/review/finding-register.json"
REPRO = ROOT / "evidence/release/reproducibility-report.json"
SECRET_SCAN = ROOT / "evidence/security/T54/secret-scan.json"
CYCLONEDX = ROOT / "evidence/release/sbom/cyclonedx-1.6.json"
SPDX = ROOT / "evidence/release/sbom/spdx-2.3.json"
PROVENANCE = ROOT / "evidence/release/provenance.json"
LICENSE_INVENTORY = ROOT / "evidence/security/T52/pre-release-inventory.json"
LEGACY_ADR = ROOT / "docs/adr/ADR-0002-clean-room-legacy-boundary.md"
# The one section 22 threshold verdict. The S20-630 accounting report of a
# real trial campaign is tracked here (`sley2.succession-accounting-report.v1`);
# the smoke report under evidence/runtime/ is untracked and never read. Until a
# campaign tracks its report, the file is absent and the thresholds do not pass.
ACCOUNTING_REPORT = ROOT / "evidence/release/succession-accounting-report.json"
ACCOUNTING_CONTRACT = accounting.REPORT_CONTRACT
BENCHMARK_PLAN = ROOT / "bench/benchmark-plan.json"
REGISTER_BUILDER = ROOT / "scripts/build_finding_register.py"
REPORT = ROOT / "evidence/release/ga-acceptance-report.json"
CONTRACT = "sley2.ga-acceptance-report.v1"
ARTIFACT_NAME = "sley-2.0.0-linux-x86_64.tar.gz"

EVIDENCED = "EVIDENCED"
AWAITS_REVIEW = "AWAITS_REVIEW"
GATED = "GATED"

HEX_40 = re.compile(r"^[0-9a-f]{40}$")
HEX_64 = re.compile(r"^[0-9a-f]{64}$")
# The only PASS forms a recorded verdict field may evidence with: the bare
# token, or an enumerated count form whose every count is zero.
ZERO_COUNT_FORM = re.compile(r"^PASS(?:_0_P[0-4])+$")
TRANSCRIPT = re.compile(r"evidence/review/verdicts/[A-Za-z0-9_./-]+\.md")


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def display(path: Path) -> str:
    return str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path)


_REGISTER_BUILDER = None


def register_builder():
    """The finding-register builder, loaded by path so its classifier is reused.

    `check_finding_register.py` loads the same module the same way: one
    classifier of dispositions, one completion predicate, no re-implementation.
    """
    global _REGISTER_BUILDER
    if _REGISTER_BUILDER is None:
        spec = importlib.util.spec_from_file_location("build_finding_register", REGISTER_BUILDER)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        _REGISTER_BUILDER = module
    return _REGISTER_BUILDER


def complete_pass_form(disposition: object) -> bool:
    """A complete PASS: the register classifier reads PASS and no finding is named.

    Accepted forms are the bare `PASS` token and an enumerated count form
    whose every count is zero (`PASS_0_P0_0_P1_0_P2_0_P3`). Everything else
    (`PASS_PENDING_CONFIRMATION_2_P0_OPEN`, `PASSED_TO_NEXT_ROUND`,
    `PASS_2_P1`, `PASS_WITH_OPEN_P1`, a PASS naming follow-ups) evidences
    nothing, whatever its prefix.
    """
    if not isinstance(disposition, str):
        return False
    if disposition != "PASS" and not ZERO_COUNT_FORM.fullmatch(disposition):
        return False
    builder = register_builder()
    return builder.classify_token(disposition) == "PASS" and not builder.severities_of(disposition)


def flagged_rows(register: dict) -> set[tuple[str, str]]:
    """Register rows that block clearance beyond their state: unclaimed or unclassified."""
    flagged: set[tuple[str, str]] = set()
    for key in ("unclaimed_carried_findings", "unclassified"):
        for row in register.get(key, []) or []:
            if isinstance(row, dict):
                flagged.add((str(row.get("section")), str(row.get("field"))))
    return flagged


def register_row(register: dict, section: str, field: str) -> dict | None:
    for row in register.get("obligations", []) or []:
        if isinstance(row, dict) and row.get("section") == section and row.get("field") == field:
            return row
    return None


def verdict_complete_pass(register: dict, section: str, field: str) -> tuple[bool, str]:
    """Whether a recorded verdict field evidences, with the exact reason.

    The register row must exist in state `PASS`, the disposition must be a
    complete PASS form, and the row must appear in neither
    `unclaimed_carried_findings` nor `unclassified`.
    """
    row = register_row(register, section, field)
    name = f"{section}.{field}"
    if row is None:
        return False, f"{name} is not a register obligation"
    disposition = str(row.get("disposition", ""))
    if row.get("state") != "PASS":
        return False, f"{name} reads {disposition} (register state {row.get('state')})"
    if (section, field) in flagged_rows(register):
        unclaimed = [
            entry
            for entry in register.get("unclaimed_carried_findings", []) or []
            if entry.get("section") == section and entry.get("field") == field
        ]
        named = ", ".join(unclaimed[0].get("unclaimed_severities", [])) if unclaimed else "an unclassified form"
        return False, f"{name} reads {disposition}, a PASS naming unclaimed findings ({named})"
    if not complete_pass_form(disposition):
        return False, f"{name} reads {disposition}, which is not a complete PASS form"
    return True, f"{name} reads {disposition} (register state PASS, complete form)"


def succession_thresholds() -> dict:
    """The one section 22 threshold verdict the GA report and the dossier both read.

    Derived from the tracked S20-630 accounting report only: every threshold
    row must read `PASS`, the report must be `COMPLETE` over verified claims.
    An absent, foreign, partial, or undetermined report does not pass. No
    machine-summary hand key is read.
    """
    source = display(ACCOUNTING_REPORT)
    if not ACCOUNTING_REPORT.exists():
        return {
            "source": source,
            "present": False,
            "pass": False,
            "rows": {},
            "note": f"no tracked S20-630 accounting report at {source}; no trial campaign has recorded a threshold table",
        }
    try:
        report = load(ACCOUNTING_REPORT)
    except (OSError, json.JSONDecodeError) as error:
        return {"source": source, "present": True, "pass": False, "rows": {}, "note": f"{source} is unreadable: {error}"}
    rows = report.get("thresholds") if isinstance(report, dict) else None
    if not isinstance(rows, dict) or report.get("contract") != ACCOUNTING_CONTRACT:
        return {
            "source": source,
            "present": True,
            "pass": False,
            "rows": {},
            "note": f"{source} does not carry the {ACCOUNTING_CONTRACT} contract with a threshold table",
        }
    try:
        accounting.verify_report(report)
    except accounting.AccountingError as error:
        return {
            "source": source, "present": True, "pass": False, "rows": {},
            "note": f"{source} failed S20-630 report verification: {error}",
        }
    expected = set(load(BENCHMARK_PLAN)["thresholds"]) | accounting.NOT_EVALUATED_NAMES
    results = {name: str(row.get("result")) if isinstance(row, dict) else "MALFORMED" for name, row in sorted(rows.items())}
    status = report.get("status")
    evidence_status = report.get("evidence_status")
    passed = (
        set(results) == expected
        and all(result == "PASS" for result in results.values())
        and status == "COMPLETE"
        and evidence_status == accounting.EVIDENCE_VERIFIED
    )
    note = (
        f"{source} status {status}, evidence status {evidence_status}; "
        f"threshold coverage {len(set(results) & expected)}/{len(expected)}, "
        f"unexpected rows {sorted(set(results) - expected)}; threshold rows "
        + ", ".join(f"{name}={result}" for name, result in results.items())
    )
    return {"source": source, "present": True, "pass": passed, "rows": results, "note": note}


def load_sources() -> dict:
    """Every tracked input of the report, in one mapping the derivation reads."""

    def optional(path: Path) -> dict | None:
        if not path.exists():
            return None
        try:
            value = load(path)
        except (OSError, json.JSONDecodeError):
            return None
        return value if isinstance(value, dict) else None

    return {
        "summary": load(SUMMARY),
        "conformance": load(CONFORMANCE),
        "threats": load(THREATS),
        "symbols": load(SYMBOLS),
        "anti_goals": load(ANTI_GOALS),
        "register": load(REGISTER),
        "repro": load(REPRO),
        "secret_scan": optional(SECRET_SCAN),
        "cyclonedx": optional(CYCLONEDX),
        "spdx": optional(SPDX),
        "provenance": optional(PROVENANCE),
        "license_inventory": optional(LICENSE_INVENTORY),
        "legacy_adr_present": LEGACY_ADR.exists(),
        "thresholds": succession_thresholds(),
    }


def derive_criteria(sources: dict) -> list[dict]:
    summary = sources["summary"]
    conformance = sources["conformance"]
    threats = sources["threats"]
    symbols = sources["symbols"]
    anti_goals = sources["anti_goals"]
    register = sources["register"]
    repro = sources["repro"]
    thresholds = sources["thresholds"]
    builder = register_builder()

    def section(name: str) -> dict:
        value = summary.get(name)
        return value if isinstance(value, dict) else {}

    def status(name: str) -> str:
        got = section(name).get("status", "")
        return got if isinstance(got, str) else ""

    # Package completion is the register's predicate (FINDING_REGISTER_V1
    # section 7): a status ending COMPLETE, never a mid-string COMPLETE that
    # names a restricted boundary.
    complete_packages = set(register.get("complete_packages", []) or [])

    def complete(name: str) -> bool:
        return name in complete_packages and builder.is_complete_status(status(name))

    def all_complete(*names: str) -> bool:
        return all(complete(name) for name in names)

    def incomplete(*names: str) -> str:
        missing = [f"{name} {status(name) or 'absent'}" for name in names if not complete(name)]
        return "; ".join(missing) if missing else "all at terminal status"

    def anti_goal(name: str) -> str:
        for entry in anti_goals.get("anti_goals", []):
            if entry.get("anti_goal") == name:
                return str(entry.get("state"))
        return "REVIEW_ONLY"

    def field(name: str, key: str) -> str:
        got = section(name).get(key, "")
        return got if isinstance(got, str) else ""

    declared = register.get("declared_open_findings", {}) or {}
    open_p0_p2 = sum(int(declared.get(key, 0) or 0) for key in ("p0", "p1", "p2"))
    obligations = [row for row in register.get("obligations", []) or [] if isinstance(row, dict)]
    flagged = flagged_rows(register)

    def lane_rows(reviewer: str) -> list[dict]:
        return [row for row in obligations if row.get("reviewer") == reviewer]

    def lane_clear(reviewer: str) -> bool:
        rows = lane_rows(reviewer)
        return (
            bool(rows)
            and all(row.get("state") in ("PASS", "HISTORICAL_ROUND") for row in rows)
            and not any((row.get("section"), row.get("field")) in flagged for row in rows)
        )

    def lane_note(reviewer: str) -> str:
        rows = lane_rows(reviewer)
        counts = {
            state: sum(1 for row in rows if row.get("state") == state)
            for state in ("PASS", "PENDING", "HISTORICAL_ROUND", "OTHER")
        }
        blocked = sum(1 for row in rows if (row.get("section"), row.get("field")) in flagged)
        return (
            f"finding register {reviewer} lane: {counts['PASS']} PASS, {counts['PENDING']} PENDING, "
            f"{counts['OTHER']} OTHER, {counts['HISTORICAL_ROUND']} historical, "
            f"{blocked} unclaimed or unclassified; lane {'clear' if lane_clear(reviewer) else 'open'}"
        )

    register_result = str(register.get("result"))
    register_clear = register_result == "FINDING_REGISTER_CLEAR"

    security_pass, security_note = verdict_complete_pass(register, "threat_coverage", "independent_security_review")
    independent_review = field("finding_register", "independent_review")
    independent_review_pass = complete_pass_form(independent_review) and register_clear
    independent_review_note = (
        f"S20-740 independent review {independent_review or 'PENDING'} "
        f"({'a complete PASS form' if complete_pass_form(independent_review) else 'not a complete PASS form'}); "
        f"finding register result {register_result}"
    )

    vm_sections = ("vm_extended_opcode_profile", "vm_lowering_profile", "vm_execution_profile")
    vm_semantics = all_complete(*vm_sections)
    program_sections = (
        *vm_sections,
        "s20_360_candidate_validation",
        "s20_390_atomic_commit",
        "type_system",
        "cfg_validation",
        "effect_system",
    )
    program_model_reviewed = all_complete(*program_sections) and lane_clear("ariadne")
    authority_sections = ("protected_policy_root", "capability_token_profile", "reference_adapter_profile")
    ambient_authority = all_complete(*authority_sections) and security_pass and anti_goal("arbitrary shell") == "HOLDS"
    loop_sections = ("protocol", "json_bridge", "cli", "s20_360_candidate_validation", "s20_390_atomic_commit")
    agent_loop = all_complete(*loop_sections) and anti_goal("Sley source syntax or parser") == "HOLDS"
    affordance_sections = ("protocol", "mutation_schema", "candidate_construction_profile")

    legacy = section("legacy_freeze")
    legacy_frozen = (
        legacy.get("version") == "1.2.0"
        and HEX_40.fullmatch(str(legacy.get("commit", ""))) is not None
        and HEX_64.fullmatch(str(legacy.get("artifact_sha256", ""))) is not None
        and HEX_64.fullmatch(str(legacy.get("source_snapshot_sha256", ""))) is not None
        and isinstance(legacy.get("artifact_size_bytes"), int)
    )
    clean_room = section("clean_room_disposition_register")
    reused = clean_room.get("reused_concepts")
    reused_entries = clean_room.get("reused_concept_entries")
    reused_dispositioned = (
        isinstance(reused, int) and reused > 0 and isinstance(reused_entries, list) and len(reused_entries) == reused
    )
    m0_commit = str(summary.get("m0_commit", ""))
    lineage = summary.get("lineage")
    independent_history = HEX_40.fullmatch(m0_commit) is not None and isinstance(lineage, str) and bool(lineage)

    agreement = section("conformance").get("cross_implementation_agreement")
    checked = conformance.get("independently_checked")
    families = conformance.get("fixture_directories")
    native_only = conformance.get("native_only", []) or []
    every_family_checked = (
        conformance.get("result") == "INDEPENDENT_CONFORMANCE_COMPLETE"
        and isinstance(checked, int)
        and isinstance(families, int)
        and families > 0
        and checked == families
        and not native_only
    )
    identifiers = section("identifiers")
    domains_registered = (
        complete("identifiers")
        and isinstance(identifiers.get("registry_domains"), int)
        and isinstance(identifiers.get("frozen_domains"), int)
        and identifiers["registry_domains"] > 0
    )
    stable_codes = (
        symbols.get("result") == "PASS"
        and not symbols.get("unregistered")
        and not symbols.get("ambiguous_codes")
        and not symbols.get("registered_without_declared_namespace")
    )
    crash = section("s20_530_crash_recovery")
    matrix_rows = crash.get("matrix_rows")
    crash_matrix = complete("s20_530_crash_recovery") and isinstance(matrix_rows, int) and matrix_rows > 0

    attestations = [row for row in repro.get("attestations", []) or [] if isinstance(row, dict)]
    attestation = attestations[0] if attestations else {}
    release = summary.get("release_decision")
    release = release if isinstance(release, dict) else {}
    final_commit_fixed = (
        release.get("state") == "RELEASE_APPROVED"
        and bool(attestation.get("commit"))
        and release.get("final_commit") == attestation.get("commit")
    )
    artifact_named = (
        attestation.get("artifact_name") == ARTIFACT_NAME
        and section("release_candidate_packaging").get("artifact_name") == ARTIFACT_NAME
    )
    packaging = section("release_candidate_packaging")
    demo = str(packaging.get("candidate_demo", ""))
    demo_passed = complete("release_candidate_packaging") and re.fullmatch(r"PASS_\d+_STEPS", demo) is not None
    scan = sources.get("secret_scan") or {}
    scan_clean = (
        scan.get("contract") == "s20-710-secret-scan-v1"
        and scan.get("result") == "PASS_NO_HIGH_CONFIDENCE_FINDINGS"
        and scan.get("findings") == []
        and scan.get("matched_secret_values_emitted") is False
    )
    bom = sources.get("cyclonedx") or {}
    spdx = sources.get("spdx") or {}
    provenance = sources.get("provenance") or {}
    inventory = sources.get("license_inventory") or {}
    records_recorded = (
        HEX_64.fullmatch(str(attestation.get("manifest_digest", ""))) is not None
        and HEX_64.fullmatch(str(attestation.get("artifact_sha256", ""))) is not None
        and isinstance(attestation.get("artifact_size_bytes"), int)
        and bom.get("bomFormat") == "CycloneDX"
        and isinstance(spdx.get("spdxVersion"), str)
        and inventory.get("contract") == "s20-710-pre-release-inventory-v1"
        and provenance.get("contract") == "sley2.release-provenance.v1"
    )
    reproducible = (
        repro.get("result") == "MULTI_HOST_REPRODUCIBLE"
        and bool(attestations)
        and all(row.get("reproducibility") == "REPRODUCIBLE" for row in attestations)
        and isinstance(repro.get("distinct_hosts"), int)
        and isinstance(repro.get("required_hosts"), int)
        and repro["distinct_hosts"] >= repro["required_hosts"]
    )
    tree_clean = attestation.get("working_tree_clean") is True
    unpublished = (
        summary.get("publication_authorized") is False
        and repro.get("publication_authorized") is False
        and anti_goal("unauthorized publication/deploy/spend") == "HOLDS"
    )

    def state(passed: bool, otherwise: str = AWAITS_REVIEW) -> str:
        return EVIDENCED if passed else otherwise

    criteria: list[tuple[str, str, str, str]] = [
        # group, criterion, state, evidence
        ("26.1 lineage", "Sley 1.2.0 is preserved and checksum-verified", state(legacy_frozen, GATED),
         f"machine summary legacy_freeze records version {legacy.get('version')}, commit {str(legacy.get('commit', ''))[:12] or 'none'}, "
         f"artifact digest {str(legacy.get('artifact_sha256', ''))[:12] or 'none'} and source snapshot digest "
         f"{str(legacy.get('source_snapshot_sha256', ''))[:12] or 'none'}; make legacy-runner-smoke verifies them"),
        ("26.1 lineage", "No Sley 1.2.1 implementation was required",
         state(sources.get("legacy_adr_present") is True and clean_room.get("legacy_source_in_tree") is False
               and anti_goal("Sley 1.x compatibility") == "HOLDS", GATED),
         f"docs/adr/ADR-0002-clean-room-legacy-boundary.md {'present' if sources.get('legacy_adr_present') else 'absent'}; "
         f"clean-room register legacy_source_in_tree {clean_room.get('legacy_source_in_tree')}; "
         f"anti-goal 'Sley 1.x compatibility' {anti_goal('Sley 1.x compatibility')}"),
        ("26.1 lineage", "The new repository has independent history", state(independent_history, GATED),
         f"machine summary m0_commit {m0_commit[:12] or 'none'} with lineage {lineage}"),
        ("26.1 lineage", "No legacy source tree was copied wholesale",
         state(clean_room.get("legacy_source_in_tree") is False, GATED),
         f"scripts/check_clean_room_boundary.py verifies no legacy source, dependency, or in-process touchpoint; "
         f"register legacy_source_in_tree {clean_room.get('legacy_source_in_tree')}"),
        ("26.1 lineage", "Every reused concept has a disposition and evidence", state(reused_dispositioned, GATED),
         f"docs/spec/CLEAN_ROOM_DISPOSITION_REGISTER_V1.md, {reused if isinstance(reused, int) else 0} dispositioned entries "
         f"({len(reused_entries) if isinstance(reused_entries, list) else 0} named)"),

        ("26.2 canonical format", "SCB1 is frozen as version 1", state(complete("scb1")),
         f"machine summary scb1 status {status('scb1')}"),
        ("26.2 canonical format", "Rust and independent oracle produce byte-identical output",
         state(agreement == "PASS" and complete("conformance")),
         f"machine summary conformance cross_implementation_agreement {agreement}, status {status('conformance')}"),
        ("26.2 canonical format", "Every non-canonical fixture is rejected", state(every_family_checked),
         f"independent conformance report {conformance.get('result')}: {checked} of {families} fixture families "
         f"independently checked, {len(native_only)} native-only, rejection corpora included"),
        ("26.2 canonical format", "Hashes are domain separated", state(domains_registered),
         f"docs/spec/IDENTIFIERS_V1.md registry with the drift check of scripts/check_required_contract_index.py; "
         f"identifiers status {status('identifiers')}, {identifiers.get('registry_domains')} registry domains, "
         f"{identifiers.get('frozen_domains')} frozen"),
        ("26.2 canonical format", "Schema epochs are explicit", state(complete("schema_epoch")),
         f"machine summary schema_epoch status {status('schema_epoch')}; docs/spec/EPOCH_MIGRATION_POLICY_V1.md governs successors"),
        ("26.2 canonical format", "Pack import reconstructs exact roots", state(complete("repository_pack")),
         f"machine summary repository_pack status {status('repository_pack')}"),
        ("26.2 canonical format", "Corruption is detected before ref advancement", state(complete("s20_530_crash_recovery")),
         f"machine summary s20_530_crash_recovery status {status('s20_530_crash_recovery')}"),

        ("26.3 semantics", "SSMC1 is the only canonical program representation", state(complete("ssmc1")),
         f"machine summary ssmc1 status {status('ssmc1')}"),
        ("26.3 semantics", "No source parser exists in the GA dependency graph",
         state(anti_goal("Sley source syntax or parser") == "HOLDS", GATED),
         f"anti-goal conformance report 'Sley source syntax or parser' {anti_goal('Sley source syntax or parser')}, parser crates absent from Cargo.lock"),
        ("26.3 semantics", "Type, CFG, effect, contract, and identity checks are deterministic",
         state(all_complete("type_system", "cfg_validation", "effect_system", "identifiers")),
         f"machine summary type_system, cfg_validation, effect_system, identifiers: {incomplete('type_system', 'cfg_validation', 'effect_system', 'identifiers')}"),
        ("26.3 semantics", "VM semantics agree with conformance fixtures", state(vm_semantics),
         f"conformance/vm-extended/v1 with the independent oracle; VM profiles: {incomplete(*vm_sections)}"),
        ("26.3 semantics", "No undefined behavior exists in the program model", state(program_model_reviewed),
         f"the exhaustive judgment and execution contracts at terminal status ({incomplete(*program_sections)}) "
         f"with the Ariadne lane clear in the finding register ({lane_note('ariadne')})"),
        ("26.3 semantics", "No ambient authority exists", state(ambient_authority),
         f"protected policy root, capability tokens, and reference adapters at terminal status ({incomplete(*authority_sections)}); "
         f"independent security review: {security_note}; anti-goal 'arbitrary shell' {anti_goal('arbitrary shell')}"),

        ("26.4 agent loop", "agents can create and maintain programs without source", state(agent_loop),
         f"SMP1 endpoint, bridge, CLI, candidate validation, and atomic commit at terminal status ({incomplete(*loop_sections)}); "
         f"anti-goal 'Sley source syntax or parser' {anti_goal('Sley source syntax or parser')}"),
        ("26.4 agent loop", "query capsules are bounded", state(complete("context_capsule_profile")),
         f"machine summary context_capsule_profile status {status('context_capsule_profile')}"),
        ("26.4 agent loop", "session handles are stale-safe", state(complete("session_handle_profile")),
         f"machine summary session_handle_profile status {status('session_handle_profile')}"),
        ("26.4 agent loop", "typed affordances and mutations are available", state(all_complete(*affordance_sections)),
         f"protocol, mutation schema, and candidate construction at terminal status ({incomplete(*affordance_sections)})"),
        ("26.4 agent loop", "invalid candidates cannot commit",
         state(all_complete("s20_360_candidate_validation", "s20_390_atomic_commit")),
         f"S20-360 candidate validation and the S20-390 commit boundary at terminal status ({incomplete('s20_360_candidate_validation', 's20_390_atomic_commit')})"),
        ("26.4 agent loop", "exact stale conflicts are rejected", state(complete("s20_360_candidate_validation")),
         f"the S20-360 stale root and stale entity decisions with their result vectors; status {status('s20_360_candidate_validation')}"),
        ("26.4 agent loop", "machine outputs use stable codes and contracts", state(stable_codes),
         f"docs/spec/ERROR_CODES_V1.md with the per-package checkers that pin every reserved range; error-symbol registration "
         f"{symbols.get('result')}: {symbols.get('emitted_symbols')} symbols, {len(symbols.get('unregistered', []) or [])} unregistered, "
         f"{len(symbols.get('ambiguous_codes', []) or [])} ambiguous codes"),

        ("26.5 repository", "state versions are content addressed", state(complete("state_root")),
         f"machine summary state_root status {status('state_root')}"),
        ("26.5 repository", "refs update atomically", state(complete("s20_500_native_refs_branches")),
         f"machine summary s20_500_native_refs_branches status {status('s20_500_native_refs_branches')}"),
        ("26.5 repository", "branches preserve ancestry", state(complete("s20_500_native_refs_branches")),
         f"the S20-500 immutable branch origin and bounded ancestry rules; status {status('s20_500_native_refs_branches')}"),
        ("26.5 repository", "disjoint merge is deterministic", state(complete("merge")),
         f"machine summary merge status {status('merge')}"),
        ("26.5 repository", "ambiguous merge creates conflict objects", state(complete("merge")),
         f"the S20-520 conflict objects with their corpus; merge status {status('merge')}"),
        ("26.5 repository", "GC preserves all retained roots", state(complete("garbage_collection")),
         f"machine summary garbage_collection status {status('garbage_collection')}"),
        ("26.5 repository", "crash recovery produces only old or complete new state", state(crash_matrix),
         f"the S20-530 crash matrix of {matrix_rows} rows, status {status('s20_530_crash_recovery')}"),

        ("26.6 policy and security", "policy is protected from the judged candidate",
         state(complete("protected_policy_root") and lane_clear("nabu")),
         f"machine summary protected_policy_root status {status('protected_policy_root')}; {lane_note('nabu')}"),
        ("26.6 policy and security", "capability tokens cannot be forged or replayed across scope",
         state(complete("capability_token_profile") and security_pass),
         f"machine summary capability_token_profile status {status('capability_token_profile')}; independent security review: {security_note}"),
        ("26.6 policy and security", "adapters are bounded", state(complete("reference_adapter_profile")),
         f"machine summary reference_adapter_profile status {status('reference_adapter_profile')}"),
        ("26.6 policy and security", "no arbitrary shell exists",
         state(anti_goal("arbitrary shell") == "HOLDS", GATED),
         f"anti-goal conformance report 'arbitrary shell' {anti_goal('arbitrary shell')}, no kernel source invokes a process"),
        ("26.6 policy and security", "all P0/P1 threats have passing tests",
         state(security_pass and not threats.get("p0_p1_without_located_symbol") and not symbols.get("unexercised")),
         f"threat coverage report: {threats.get('states', {}).get('SYMBOL_REALIZED_WITH_EXERCISE', 0)} exercised, "
         f"{len(threats.get('p0_p1_without_located_symbol', []) or [])} P0/P1 without a located symbol; "
         f"{symbols.get('emitted_symbols')} stable failure symbols, {len(symbols.get('unexercised', []) or [])} unexercised, "
         f"{len(symbols.get('ambiguous_codes', []) or [])} numeric codes carrying more than one symbol; "
         f"independent security review: {security_note}. Whether a located, exercised control mitigates its threat stays the review's judgment"),
        ("26.6 policy and security", "no P0/P1/P2 finding remains open", state(open_p0_p2 == 0, GATED),
         f"finding register declares {open_p0_p2} open P0/P1/P2 findings across {register.get('obligation_count')} obligations"),
        ("26.6 policy and security", "opacity is not used as a security argument",
         state(anti_goal("opacity as security") == "HOLDS"),
         f"anti-goal conformance report 'opacity as security' {anti_goal('opacity as security')}; every contract is public in docs/spec "
         f"and the independent oracle checks {checked} fixture families, which is the review's input"),

        ("26.7 succession", "every section 22 threshold passes", state(thresholds.get("pass") is True, GATED),
         str(thresholds.get("note"))),

        ("26.8 packaging", f"artifact name {ARTIFACT_NAME}", state(artifact_named, GATED),
         f"reproducibility report attests {attestation.get('artifact_name', 'no attestation')}; packaging records "
         f"{packaging.get('artifact_name', 'no artifact name')}"),
        ("26.8 packaging", "artifact is built from the final candidate commit", state(final_commit_fixed, GATED),
         f"attested commit {attestation.get('commit', 'none')}; the final commit is fixed by a recorded release decision "
         f"(machine summary release_decision), state {release.get('state', 'none')}"),
        ("26.8 packaging", "artifact runs with no source-tree access", state(demo_passed),
         f"the S20-720 unpacked demo runs the packaged binary outside the tree: candidate_demo {demo or 'absent'}, "
         f"packaging status {status('release_candidate_packaging')}"),
        ("26.8 packaging", "artifact contains no secrets, local paths, caches, or debug files", state(scan_clean),
         f"the S20-720 forbidden content scan (evidence/security/T54/secret-scan.json) reports {scan.get('result', 'no scan')} with "
         f"{len(scan.get('findings', []) or []) if scan else 'no'} findings"),
        ("26.8 packaging", "manifest, SHA-256, size, SBOM, license inventory, and provenance are recorded", state(records_recorded, GATED),
         f"attested manifest digest {str(attestation.get('manifest_digest', ''))[:12] or 'none'}, artifact digest "
         f"{str(attestation.get('artifact_sha256', ''))[:12] or 'none'}, {attestation.get('artifact_size_bytes', 'no')} bytes; "
         f"CycloneDX {bom.get('specVersion', 'absent')}, {spdx.get('spdxVersion', 'SPDX absent')}, license inventory "
         f"{inventory.get('contract', 'absent')}, provenance {provenance.get('contract', 'absent')} (unsigned)"),
        ("26.8 packaging", "second clean build establishes reproducibility", state(reproducible, GATED),
         f"reproducibility report result {repro.get('result')} across {repro.get('distinct_hosts')} of "
         f"{repro.get('required_hosts')} required hosts, {len(attestations)} attestations"),
        ("26.8 packaging", "the source working tree is clean", state(tree_clean, GATED),
         f"the attested build records working_tree_clean {attestation.get('working_tree_clean', 'no attestation')}"),

        ("26.9 review", "Ariadne approves SSMC1 and semantic correctness",
         state(lane_clear("ariadne") and complete("ssmc1") and vm_semantics),
         f"{lane_note('ariadne')}; ssmc1 {status('ssmc1')}; VM profiles: {incomplete(*vm_sections)}"),
        ("26.9 review", "Nabu approves architectural and cross-product boundaries", state(lane_clear("nabu")),
         lane_note("nabu")),
        ("26.9 review", "Vulcan or current independent reviewer issues a complete PASS", state(independent_review_pass),
         f"{independent_review_note}; a complete PASS is the bare PASS token or an all-zero enumerated count form over a CLEAR register"),
        ("26.9 review", "all reviewer findings are resolved or dispositioned below P3",
         state(register_clear and open_p0_p2 == 0),
         f"finding register result {register_result} with {len(register.get('open_reviews', []) or [])} open, "
         f"{len(register.get('unclassified', []) or [])} unclassified, and "
         f"{len(register.get('unclaimed_carried_findings', []) or [])} unclaimed-carried obligations"),
        ("26.9 review", "publication remains unauthorized unless separately granted", state(unpublished, GATED),
         f"machine summary publication_authorized {summary.get('publication_authorized')}; reproducibility report "
         f"publication_authorized {repro.get('publication_authorized')}; anti-goal 'unauthorized publication/deploy/spend' "
         f"{anti_goal('unauthorized publication/deploy/spend')}"),
    ]
    return [
        {"group": group, "criterion": criterion, "state": state_, "evidence": evidence}
        for group, criterion, state_, evidence in criteria
    ]


def build_report(sources: dict | None = None) -> dict:
    if sources is None:
        sources = load_sources()
    entries = derive_criteria(sources)
    states: dict[str, int] = {}
    for entry in entries:
        states[entry["state"]] = states.get(entry["state"], 0) + 1
    register = sources["register"]
    report = {
        "contract": CONTRACT,
        "source": "master goal section 26",
        "criterion_count": len(entries),
        "states": states,
        "gated": [entry["criterion"] for entry in entries if entry["state"] == GATED],
        "awaiting_review": [entry["criterion"] for entry in entries if entry["state"] == AWAITS_REVIEW],
        "criteria": entries,
        # The register whose dispositions the review criteria were derived
        # from; the dossier refuses a report bound to a different register.
        "register_digest": register.get("register_digest"),
        "obligations_digest": register.get("obligations_digest"),
        "thresholds_source": sources["thresholds"].get("source"),
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


def verify_report_digest(report: dict) -> bool:
    """Whether a report's `report_digest` is the digest of the rest of it."""
    if not isinstance(report, dict) or not isinstance(report.get("report_digest"), str):
        return False
    body = {key: value for key, value in report.items() if key != "report_digest"}
    return digest_of(body) == report["report_digest"]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args(argv)
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
