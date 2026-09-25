#!/usr/bin/env python3
"""Check the fail-closed local completion frontier for unfinished Sley 2 work."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any
sys.path.insert(0, str(Path(__file__).resolve().parent))
import publication_authority  # noqa: E402  (sibling module)

ROOT = Path(__file__).resolve().parents[1]
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
AUDIT = ROOT / "docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SCB1 = ROOT / "docs/spec/SCB1.md"
EPOCH_SCHEMA = ROOT / "docs/spec/SSMC1_EPOCH1_SCHEMA.txt"
SSMC = ROOT / "crates/sley-ssmc/src/lib.rs"
MAKEFILE = ROOT / "Makefile"


def fail(message: str) -> None:
    raise AssertionError(message)


def require_equal(actual: Any, expected: Any, label: str) -> None:
    if actual != expected:
        fail(f"frontier drift: {label}")


def check_fail_closed_gate(gate: str) -> None:
    completed = subprocess.run(
        [sys.executable, "scripts/gate_status.py", gate],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    require_equal(completed.returncode, 2, f"{gate} return code")
    try:
        report = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise AssertionError(f"{gate} did not emit JSON") from error
    require_equal(report.get("result"), "NOT_IMPLEMENTED", f"{gate} result")


def main() -> int:
    try:
        summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
        require_equal(summary.get("status"), "IN_PROGRESS", "project status")
        require_equal(summary.get("phase"), "M2", "project phase")
        require_equal(
            summary.get("publication_authorized"),
            publication_authority.is_authorized(summary),
            "publication authority",
        )
        require_equal(
            summary.get("artifact"),
            {"path": None, "sha256": None, "size_bytes": None, "reproducibility": None},
            "release artifact",
        )

        frontier = summary.get("local_completion_frontier", {})
        expected_frontier = {
            "status": "LOCAL_IMPLEMENTATION_CONTINUES_PENDING_COUNCIL_EPOCH_AND_OPERATOR",
            "goal_complete": False,
            "next_authority_safe_package": "S20-740-INDEPENDENT-REVIEW (Council-gated); local implementation continues where a determination unblocks it",
            "e7a_contract_assertion_landed": True,
            "blocked_lane_count": 6,
            "blocked_lanes": [
                "semantics_and_queries",
                "sessions_and_protocol",
                "repository",
                "succession_benchmark",
                "adversarial",
                "supply_chain_and_release",
            ],
            "p0_package_ready": True,
            "s20_250_schema_bodies_frozen": True,
            "s20_250_impact_semantics_frozen": False,
            "locked_option_canon_resolved": True,
            "const_value_canon_resolved": True,
            "s20_350_native_implementation_complete": True,
            "s20_350_independent_conformance_complete": True,
            "s20_350_implementation_complete": True,
            "s20_350_persistent_fuzz_complete": True,
            "s20_360_restricted_validation_complete": True,
            "s20_360_operation_analysis_complete": True,
            "s20_360_independent_conformance_complete": True,
            "s20_360_persistent_fuzz_complete": True,
            "s20_390_restricted_atomic_commit_complete": True,
            "s20_390_independent_conformance_complete": True,
            "s20_390_persistent_fuzz_complete": True,
            "s20_390_full_recovery_complete": False,
            "s20_500_native_refs_branches_implemented": True,
            "s20_500_closeout_complete": True,
            "s20_530_contract_frozen": True,
            "s20_530_implementation_complete": True,
            "s20_530_final_checker_confirmed": True,
            "s20_530_aging_rule": "ADR-0024",
            "s20_540_implementation_started": True,
            "s20_540_complete": True,
            "s20_510_blocked_by_full_s20_250": False,
            "s20_250_full_implemented": True,
            "s20_510_implemented": True,
            "s20_520_implemented": True,
            "s20_300_full_implemented": True,
            "s20_310_full_implemented": True,
            "s20_320_full_implemented": True,
            "s20_400_contract_drafted": True,
            "s20_410_implemented": True,
            "s20_440_implemented": True,
            "s20_330_implemented": True,
            "s20_420_implemented": True,
            "s20_430_implemented": True,
            "s20_620_implemented": True,
            "s20_630_implemented": True,
            "s20_410_slice_c_implemented": True,
            "s20_720_mechanics_implemented": True,
            "session_authority_available": True,
            "json_bridge_available": True,
            "cli_available": True,
            "sley2_trial_runner_available": True,
            "succession_accounting_available": True,
            "transaction_boundary_available": True,
            "fixed_accepted_head_available": True,
            "named_ref_boundary_available": True,
            "protocol_boundary_available": True,
            "merge_boundary_available": True,
            "real_benchmark_run_authorized": False,
            "root_license_text_approved": True,
            "release_artifact_available": False,
            "required_specialist_review": "PASS_S20_500_NABU_ARIADNE_VULCAN",
            "focused_semantic_security_review": "PASS_NO_OPEN_P0_P1_P2_P3_P4",
            "full_v2_eligible": False,
            "release_check_eligible": False,
            "frontier_note": (
                "the frontier is not exhausted while a contract names an owned, specified, unimplemented behaviour. Answering the S20-760 profile-versus-epoch question per agenda item (revision 2, 2026-09-03) unblocked slice E7a, and auditing what the extended profile changed found the S20-290 rejected-report profile collision. The same two techniques, reading each contract's own exclusions and re-auditing every identity whose inputs grew, are the standing local method"
            ),
        }
        require_equal(frontier, expected_frontier, "machine-summary frontier")

        session = summary.get("session_handle_profile", {})
        # Re-audited 2026-09-03 (ADR-0033): S20-330 moved from deferred to a
        # staged contract; the session module may exist only while the staged
        # checker says its implementation is in progress or later.
        session_status = session.get("status")
        session_statuses = (
            "S20_330_CONTRACT_DRAFT_REVIEW_PENDING",
            "S20_330_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS",
            "S20_330_CONTRACT_FROZEN_IMPLEMENTATION_PENDING",
            "S20_330_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS",
            "S20_330_IMPLEMENTED_REVIEW_PENDING",
            "S20_330_COMPLETE",
        )
        if session_status not in session_statuses:
            fail(f"S20-330 status drift: {session_status}")
        session_implemented = session_status in session_statuses[1:]
        require_equal(
            (ROOT / "crates/sley-protocol/src/session.rs").exists(),
            session_implemented,
            "S20-330 implementation",
        )
        require_equal(
            session.get("unblocked_by_restricted_s20_320"), False, "S20-330 authority"
        )
        mutation = summary.get("mutation_value_profile", {})
        for field in ("aggregate_codecs", "candidate_construction"):
            require_equal(mutation.get(field), True, f"S20-350 {field}")
        for field in ("candidate_authority", "runtime_mutation"):
            require_equal(mutation.get(field), False, f"S20-350 {field}")
        for field in (
            "native_s20_350_implementation_complete",
            "independent_conformance_complete",
            "persistent_candidate_fuzz_harness",
            "full_s20_350_complete",
        ):
            require_equal(mutation.get(field), True, f"S20-350 {field}")
        for field in ("generic_option_canon_resolved", "const_value_canon_resolved"):
            require_equal(mutation.get(field), True, f"S20-350 {field}")
        require_equal(
            mutation.get("nabu_review"),
            "PASS_FROZEN_ARCHITECTURE_INTEGRATED",
            "S20-350 review",
        )
        # The row was normalized to the register's closed form once the
        # c04539b9 closure review verified its carried P2/P3/P4 (the original
        # disposition string is preserved in the sibling `_original_note`).
        require_equal(
            mutation.get("epoch1_reanchor_review"),
            "PASS_P2_P3_P4_CLOSED",
            "S20-350 epoch-1 re-anchor review",
        )
        require_equal(
            mutation.get("epoch1_reanchor_review_original_note"),
            "PASS_AFTER_P2_VECTOR_REANCHOR",
            "S20-350 epoch-1 re-anchor review (original disposition)",
        )
        require_equal(
            summary.get("candidate_contract_freeze", {}).get("s20_350_unblocked"),
            True,
            "S20-345 unblocks S20-350",
        )
        require_equal(
            summary.get("fingerprint_impact_profile", {}).get("unmodeled_entity_kinds"),
            [],
            "S20-250 modeled kinds",
        )
        require_equal(
            summary.get("s20_700_remaining_surface_audit", {}).get(
                "next_dependency_complete_package"
            ),
            "S20-740-INDEPENDENT-REVIEW (Council-gated); local hardening continues discretionarily",
            "S20-700 next package",
        )
        validation = summary.get("s20_360_candidate_validation", {})
        require_equal(
            validation.get("status"),
            "COMPLETE_RESTRICTED_EXECUTABLE_PROGRAM_OPERATION_ANALYSIS_BOUNDARY",
            "S20-360 operation analysis status",
        )
        require_equal(validation.get("validation_phases"), 14, "S20-360 phases")
        require_equal(validation.get("terminal_decisions"), 16, "S20-360 decisions")
        require_equal(
            validation.get("operation_success_subset"),
            "extended-opcode-families-e1-through-e6",
            "S20-360 success subset",
        )
        require_equal(
            validation.get("excluded_operation_opcodes"),
            [144, 145, 160, 161, 162],
            "S20-360 excluded opcodes",
        )
        require_equal(
            validation.get("operation_judgment_owner"),
            "sley_vm::judge_function_operations",
            "S20-360 judgment owner",
        )
        require_equal(
            validation.get("ariadne_review"),
            "PASS_AFTER_RESOURCE_LIMIT_DECISION_FIX",
            "S20-360 Ariadne review",
        )
        require_equal(
            validation.get("vulcan_review"),
            "PASS_P3_CORPUS_BREADTH_CLOSED_NO_OPEN_P0_P1_P2_P3_P4",
            "S20-360 Vulcan review",
        )
        # The validator still holds no authority; only the operation analysis
        # of families E1 through E6 landed (ADR-0044).
        for field in (
            "accepted_state_mutation",
            "capability_ledger_mutation",
            "candidate_authority",
            "commit_authority",
            "runtime_authority",
            "full_ga_fingerprint_requirement_complete",
        ):
            require_equal(validation.get(field), False, f"S20-360 {field}")
        require_equal(
            validation.get("full_ga_operation_analysis_complete"),
            True,
            "S20-360 full_ga_operation_analysis_complete",
        )
        transaction = summary.get("s20_390_atomic_commit", {})
        require_equal(
            transaction.get("status"),
            "COMPLETE_RESTRICTED_EXECUTABLE_PROGRAM_TEST_FREE_BOUNDARY_WITH_EXTENDED_OPERATION_PROFILE",
            "S20-390 status",
        )
        for field in (
            "fresh_commit_time_revalidation",
            "fixed_accepted_head",
            "independent_conformance_complete",
            "persistent_fuzz_complete",
            "trusted_genesis",
        ):
            require_equal(transaction.get(field), True, f"S20-390 {field}")
        for field in (
            "full_crash_recovery_complete",
            "named_refs_and_branches",
            "runtime_authority",
            "selected_test_execution",
        ):
            require_equal(transaction.get(field), False, f"S20-390 {field}")
        require_equal(transaction.get("fault_boundaries"), 5, "S20-390 faults")
        require_equal(
            transaction.get("ariadne_review"),
            "PASS_FINDINGS_CLOSED_NO_OPEN_P0_P1_P2_P3_P4",
            "S20-390 Ariadne review",
        )
        require_equal(
            transaction.get("vulcan_review"),
            "PASS_MANIFEST_LENGTH_FINDINGS_CLOSED_NO_OPEN_P0_P1_P2_P3_P4",
            "S20-390 Vulcan review",
        )
        require_equal(
            summary.get("s20_710_pre_release_audit", {}).get("full_s20_710_complete"),
            False,
            "S20-710 completion",
        )

        scb1 = " ".join(SCB1.read_text(encoding="utf-8").split())
        if (
            "`Option<T>` is a union with tag 0 and zero-length payload for `None`, or tag 1"
            not in scb1
        ):
            fail("SCB1 Option<T> tag marker changed")
        epoch_schema = EPOCH_SCHEMA.read_text(encoding="utf-8")
        if "generic union Option<T>(0:None,1:Some<T>)" not in epoch_schema:
            fail("epoch Option<T> tag marker changed")
        if "generic union Option<T>(1:None,2:Some<T>)" in epoch_schema:
            fail("provisional epoch Option<T> tags remain")

        # Re-audited 2026-09-03 (ADR-0026): the six S20-250 core bodies may
        # exist only while the full profile's staged checker says the
        # implementation is in progress or later; the staged checker owns
        # the freeze-before-implementation rule.
        ssmc = SSMC.read_text(encoding="utf-8")
        full_profile = summary.get("complete_entity_impact_profile", {})
        bodies_allowed = full_profile.get("status") in (
            "S20_250_FULL_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS",
            "S20_250_FULL_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS",
            "S20_250_FULL_IMPLEMENTED_REVIEW_PENDING",
            "S20_250_FULL_COMPLETE",
        )
        for type_name in (
            "WorkspaceDefinition",
            "PackageDefinition",
            "NamespaceDefinition",
            "EntryPointDefinition",
            "PolicyBindingDefinition",
            "DependencyBindingDefinition",
        ):
            present = f"pub struct {type_name}" in ssmc
            if present and not bodies_allowed:
                fail(f"S20-250 core body appeared before its staged checker allows it: {type_name}")
            if bodies_allowed and not present:
                fail(f"S20-250 core body missing while the full profile is in progress: {type_name}")

        # Re-audited 2026-09-03 (ADR-0035): the CLI crate may exist only while
        # the S20-430 staged checker says its implementation is in progress,
        # implemented, or complete.
        cli_status = summary.get("cli", {}).get("status")
        if (ROOT / "crates/sley-cli").exists() and cli_status not in (
            "S20_430_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS",
            "S20_430_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS",
            "S20_430_IMPLEMENTED_REVIEW_PENDING",
            "S20_430_COMPLETE",
        ):
            fail("cli production boundary appeared before its staged checker allows it")
        # Re-audited 2026-09-03 (ADR-0034): the JSON bridge crate may exist only
        # while the S20-420 staged checker says its implementation is in
        # progress, implemented, or complete.
        bridge_status = summary.get("json_bridge", {}).get("status")
        if (ROOT / "crates/sley-json-bridge").exists() and bridge_status not in (
            "S20_420_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS",
            "S20_420_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS",
            "S20_420_IMPLEMENTED_REVIEW_PENDING",
            "S20_420_COMPLETE",
        ):
            fail("json bridge production boundary appeared before its staged checker allows it")
        # Re-audited 2026-09-03 (ADR-0032): the protocol crate may exist only
        # while the S20-400 staged checker says S20-410 is in progress,
        # implemented, or complete.
        protocol_status = summary.get("protocol", {}).get("status")
        if (ROOT / "crates/sley-protocol").exists() and protocol_status not in (
            "S20_400_CONTRACT_DRAFT_S20_410_IN_PROGRESS",
            "S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING",
            "S20_400_COMPLETE",
        ):
            fail("protocol production boundary appeared before its staged checker allows it")
        # Re-audited 2026-09-03 (ADR-0028): the merge module may exist only while
        # the S20-520 staged checker says its implementation is in progress or
        # later; that checker owns the freeze-before-implementation rule.
        merge_allowed = summary.get("merge", {}).get("status") in (
            "S20_520_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS",
            "S20_520_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS",
            "S20_520_IMPLEMENTED_REVIEW_PENDING",
            "S20_520_COMPLETE",
        )
        merge_present = (ROOT / "crates/sley-repo/src/merge.rs").exists()
        if merge_present and not merge_allowed:
            fail("merge production boundary appeared before its staged checker allows it")
        if merge_allowed and not merge_present:
            fail("merge module missing while S20-520 is in progress")

        packages = WORK_PACKAGES.read_text(encoding="utf-8")
        for marker in (
            "| S20-330 | 320 |",
            "| S20-390 | 150,160,360,370 |",
            "| S20-400 | 310,350,390 |",
            "| S20-500 | 390 |",
            "| S20-620 | 320,420,430 |",
            "| S20-720 | all GA code |",
        ):
            if marker not in packages:
                fail(f"work-package dependency drift: {marker}")

        audit = AUDIT.read_text(encoding="utf-8")
        for marker in (
            "S20-540 pack exchange is complete",
            "S20-250 remains incomplete",
            "candidate construction is proposal-only",
            "full S20-250 entity bodies",
            "S20-510 semantic comparison is implemented",
            "S20-520 merge is implemented",
            "the full S20-300 complete-root snapshot is implemented",
            "the full S20-310 root-backed queries are implemented",
            "the full S20-320 context capsule is implemented",
            "the S20-400 SMP1 contract is drafted",
            "S20-410 is implemented",
            "S20-440 is implemented",
            "S20-330 is implemented",
            "S20-420 is implemented",
            "S20-430 is implemented",
            "S20-620 is implemented",
            "S20-630 is implemented",
            "S20-720 mechanics are implemented",
            "the full S20-260/S20-270 opcode program is implemented",
            "S20-730 mechanics are implemented",
            "the standards SBOM and release provenance are implemented",
            "the S20-740 finding register is implemented",
            "the S20-750 decision dossier is implemented",
            "the full S20-360 operation analysis and the extended semantic profile are implemented",
            "every fixture family is independently checked",
        ):
            if marker not in audit:
                fail(f"frontier audit marker missing: {marker}")
        if (
            "python3 scripts/check_local_completion_frontier.py"
            not in MAKEFILE.read_text(encoding="utf-8")
        ):
            fail("quick gate omits local completion frontier")

        check_fail_closed_gate("v2")
        check_fail_closed_gate("release-check")
    except (AssertionError, OSError, json.JSONDecodeError) as error:
        print(json.dumps({"reason": str(error), "result": "FAIL"}, sort_keys=True))
        return 1

    print(
        json.dumps(
            {
                "blocked_lanes": 6,
                "full_gate_run": False,
                "goal_complete": False,
                "next_authority_safe_package": "S20-740-INDEPENDENT-REVIEW (Council-gated); local implementation continues where a determination unblocks it",
                "result": "PASS",
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
