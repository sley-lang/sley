#!/usr/bin/env python3
"""S20-630 accounting over a live S20-640 run (contract section 11).

The live campaign records every attempt of one model tier in one
``attempts.jsonl`` under a ``sley2.live-campaign-manifest.v1`` run
manifest. This entry point verifies each required arm's attempts through
that arm's live verifier (``bench.live.live_claims.LIVE_ARM_VERIFIERS``,
the legacy arm included), then derives the report with the same
``arm_accounting``, threshold evaluation, and digest as the offline path:
every attempt, including timeouts, harness failures, and attempts retained
over budget, stays in every denominator and every median. The three
section 22 conditions the plan does not encode are evaluated from the
claims (``evaluate_section_22_rows``). A run frozen with label PILOT is
derived like any other run and is marked as never counting toward
succession.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from bench.accounting.report import (
    LEGACY_ARM,
    NO_CHAIN,
    REPORT_CONTRACT,
    SLEY2_ARM,
    STATUS_FIELDS,
    AccountingError,
    AccountingErrorCode,
    _complete,
    _fail,
    arm_accounting,
    derive_evidence_status,
    evaluate_section_22_rows,
    evaluate_thresholds,
    report_digest,
    verify_report,
    write_report,
)
from bench.raw.runner import _plan_and_corpus

INPUT_MODE = "LIVE_ATTEMPTS"
COUNTING_LABEL = "CAMPAIGN"


def derive_live_report(run_directory: Path, *, require_complete: bool = False,
                       require_campaign: bool = False) -> dict[str, Any]:
    from bench.live.live_claims import LIVE_ARM_VERIFIERS, LiveClaimError
    from bench.live.manifest import ManifestError, manifest_digest, read_manifest

    run = Path(run_directory)
    try:
        manifest = read_manifest(run / "run_manifest.json")
    except (ManifestError, OSError) as error:
        raise AccountingError(AccountingErrorCode.RUN_INVALID, str(error)) from error
    label = manifest["environment_manifest"].get("label")
    if label not in ("CAMPAIGN", "PILOT"):
        _fail(AccountingErrorCode.RUN_INVALID, "label")
    if require_campaign and label != COUNTING_LABEL:
        _fail(AccountingErrorCode.RUN_INVALID, f"label {label} never counts toward succession")
    plan, corpus = _plan_and_corpus()
    arm_info = {arm["id"]: arm for arm in plan["arms"]}
    required = [arm["id"] for arm in plan["arms"] if arm.get("required")]
    arms: dict[str, Any] = {}
    claims_by_arm: dict[str, list[dict[str, Any]]] = {}
    observed: dict[str, set[str]] = {name: set() for name in STATUS_FIELDS}
    for arm in required:
        verifier = LIVE_ARM_VERIFIERS.get(arm)
        if verifier is None:
            _fail(AccountingErrorCode.ARM_UNKNOWN, arm)
        try:
            claims = verifier(run)
        except (LiveClaimError, ValueError, OSError) as error:
            raise AccountingError(AccountingErrorCode.CHAIN_INVALID, f"{arm}:{error}") from error
        for claim in claims:
            if claim.get("arm_id") != arm:
                _fail(AccountingErrorCode.ARM_UNKNOWN, str(claim.get("arm_id")))
        claims_by_arm[arm] = claims
        if not claims:
            arms[arm] = NO_CHAIN
            continue
        accounting = arm_accounting(claims, manifest, corpus)
        accounting["chain_verifier"] = verifier.__name__
        accounting["fixture_status"] = arm_info.get(arm, {}).get("fixture_status")
        arms[arm] = accounting
        for name in STATUS_FIELDS:
            observed[name].update(accounting["claim_statuses"][name])
    statuses = {name: sorted(observed[name]) for name in STATUS_FIELDS}
    evidence_status = derive_evidence_status(statuses)
    if all(value == NO_CHAIN for value in arms.values()):
        status = "NO_TRIALS"
    elif all(_complete(value) for value in arms.values()):
        status = "COMPLETE"
    else:
        status = "PARTIAL"
    if require_complete and status != "COMPLETE":
        _fail(AccountingErrorCode.INCOMPLETE, status)
    section_22 = evaluate_section_22_rows(arms, claims_by_arm.get(SLEY2_ARM, []), evidence_status)
    report = {
        "arm_fixture_status": {arm: arm_info.get(arm, {}).get("fixture_status") for arm in required},
        "arms": arms,
        "benchmark_plan_digest": manifest["benchmark_plan_digest"],
        "claim_statuses": statuses,
        "contract": REPORT_CONTRACT,
        "corpus_digest": manifest["corpus_digest"],
        "corpus_version": plan["corpus_version"],
        "counts_toward_succession": label == COUNTING_LABEL,
        "evidence_status": evidence_status,
        "input": {
            "label": label,
            "mode": INPUT_MODE,
            "model_exact_version": manifest["model_exact_version"],
            "model_tier": manifest["model_tier"],
            "repo_commit": manifest["repo_commit"],
            "scheduled_attempts": manifest["scheduled_attempts"],
        },
        "required_arms": required,
        "run_id": manifest["run_id"],
        "run_manifest_digest": manifest_digest(manifest),
        "status": status,
        "thresholds": evaluate_thresholds(arms, plan, manifest, evidence_status, section_22),
    }
    report["report_digest"] = report_digest(report)
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--require-complete", action="store_true")
    parser.add_argument("--require-campaign", action="store_true")
    arguments = parser.parse_args(argv)
    try:
        report = derive_live_report(arguments.run, require_complete=arguments.require_complete,
                                    require_campaign=arguments.require_campaign)
        verify_report(report)
        if arguments.output:
            write_report(report, arguments.output)
        print(json.dumps({"counts_toward_succession": report["counts_toward_succession"],
                          "evidence_status": report["evidence_status"],
                          "report_digest": report["report_digest"], "result": "DERIVED",
                          "status": report["status"]}, sort_keys=True))
        return 0
    except AccountingError as error:
        print(json.dumps({"problem": str(error), "result": "FAIL"}, sort_keys=True))
        return 1


if __name__ == "__main__":
    sys.exit(main())
